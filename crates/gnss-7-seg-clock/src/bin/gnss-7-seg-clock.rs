#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_rp::gpio;
use embassy_rp::i2c;
use embassy_rp::peripherals::{I2C1, UART1};
use embassy_rp::spi;
use embassy_rp::uart;
use embassy_time::Timer;
use embedded_io_async::Read;
use static_cell::StaticCell;

use {defmt_rtt as _, panic_probe as _};

embassy_rp::bind_interrupts!(struct Irqs {
    I2C1_IRQ => i2c::InterruptHandler<I2C1>;
    UART1_IRQ => uart::BufferedInterruptHandler<UART1>;
});

fn ubx_fill_ck(buf: &mut [u8]) {
    if buf.len() < 4 {
        return;
    }

    let mut ck_a = 0_u8;
    let mut ck_b = 0_u8;
    for c in &buf[2..buf.len() - 2] {
        ck_a += c;
        ck_b += ck_a;
    }
    buf[buf.len() - 2] = ck_a;
    buf[buf.len() - 1] = ck_b;
}

//
//     +- A -+
//     F     B
//     +- G -+
//     E     C
//     +- D -+
//
//    GFpABEDC
const TABLE: [u8; 10] = [
    0b01011111_u8, // '0'
    0b00001001_u8, // '1'
    0b10011110_u8, // '2'
    0b10011011_u8, // '3'
    0b11001001_u8, // '4'
    0b11010011_u8, // '5'
    0b11010111_u8, // '6'
    0b00011001_u8, // '7'
    0b11011111_u8, // '8'
    0b11011011_u8, // '9'
];
#[allow(dead_code)]
const MASK_DP: u8 = 0b00100000;

fn to_digit(b: u8) -> u8 {
    match b {
        b'0' => TABLE[0],
        b'1' => TABLE[1],
        b'2' => TABLE[2],
        b'3' => TABLE[3],
        b'4' => TABLE[4],
        b'5' => TABLE[5],
        b'6' => TABLE[6],
        b'7' => TABLE[7],
        b'8' => TABLE[8],
        b'9' => TABLE[9],
        _ => 0b10000000_u8,
    }
}

#[derive(defmt::Format)]
enum State<'a, const N: usize> {
    NmeaNotFound,
    Doller,
    Address1(u8),
    Address2(u8, u8),
    Address3(u8, u8, u8),
    Address4(u8, u8, u8, u8),
    IncompletePayload {
        address: (u8, u8, u8, u8, u8),
        buf: &'a mut [u8; N],
        len: usize,
    },
    Payload {
        address: (u8, u8, u8, u8, u8),
        buf: &'a mut [u8; N],
        len: usize,
    },
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    defmt::info!("Hello World!");

    let mut display_noe = gpio::Output::new(p.PIN_11, gpio::Level::High);
    let mut display_le = gpio::Output::new(p.PIN_13, gpio::Level::Low);
    let mut gnss_nreset = gpio::Output::new(p.PIN_16, gpio::Level::Low);
    let mut _gnss_extint = gpio::Input::new(p.PIN_18, gpio::Pull::Up);
    let mut _gnss_pps = gpio::Input::new(p.PIN_19, gpio::Pull::Down);

    defmt::info!("configure 7-seg display");

    let display_spi_config = {
        let mut c = spi::Config::default();
        c.frequency = 30_000_000;
        c.phase = spi::Phase::CaptureOnFirstTransition;
        c.polarity = spi::Polarity::IdleLow;
        c
    };
    let mut display_spi =
        spi::Spi::new_txonly(p.SPI1, p.PIN_14, p.PIN_15, p.DMA_CH0, display_spi_config);

    {
        display_spi.write(&[0b10000000_u8; 6]).await.unwrap(); // "------"

        display_le.set_high();
        Timer::after_nanos(15).await;
        display_le.set_low();

        display_noe.set_low();
    }

    defmt::info!("configure MAX-M10S");

    let mut gnss_i2c =
        i2c::I2c::new_async(p.I2C1, p.PIN_23, p.PIN_22, Irqs, i2c::Config::default());

    let gnss_uart_config = {
        let mut c = uart::Config::default();
        c.baudrate = 9600;
        c.parity = uart::Parity::ParityNone;
        c.stop_bits = uart::StopBits::STOP1;
        c
    };
    let mut gnss_uart = {
        static UART1_BUF_RX: StaticCell<[u8; 64]> = StaticCell::new();
        uart::BufferedUart::new(
            p.UART1,
            Irqs,
            p.PIN_20,
            p.PIN_21,
            [].as_mut_slice(),
            UART1_BUF_RX.init([0; 64]).as_mut_slice(),
            gnss_uart_config,
        )
    };

    gnss_nreset.set_high();

    {
        let ubx_cfg_valset_frame = {
            let mut frame = [
                0xb5, 0x62, // header
                0x06, 0x8a, // id/class (=UBX-CFG-VALSET)
                0x00, 0x00, // length
                // payload begin
                0x00, // version
                0x01, // layers (=ram)
                0x00, 0x00, // reserved
                // CFG-TXREADY-ENABLED (=true)
                0x02, 0x00, 0xa2, 0x10, 0x01, // CFG-TXREADY-POLARIT (=true=low-active)
                0x03, 0x00, 0xa2, 0x20, 0x05, // CFG-TXREADY-PIN (=5=EXTINT)
                0x04, 0x00, 0xa2, 0x30, 0x01, 0x00, // CFG-TXREADY-THRESHOLD (=8/8)
                0x05, 0x00, 0xa2, 0x20, 0x00, // CFG-TXREADY-INTERFACE (=0=I2C)
                0x01, 0x00, 0x71, 0x10, 0x01, // CFG-I2CINPROT-UBX (=1)
                0x02, 0x00, 0x71, 0x10, 0x00, // CFG-I2CINPROT-NMEA (=0)
                0x01, 0x00, 0x72, 0x10, 0x01, // CFG-I2COUTPROT-UBX (=1)
                0x02, 0x00, 0x72, 0x10, 0x00, // CFG-I2COUTPROT-NMEA (=0)
                0x01, 0x00, 0x73, 0x10, 0x00, // CFG-UART1INPROT-UBX (=0)
                0x02, 0x00, 0x73, 0x10, 0x01, // CFG-UART1INPROT-NMEA (=1)
                0x01, 0x00, 0x74, 0x10, 0x00, // CFG-UART1OUTPROT-UBX (=0)
                0x02, 0x00, 0x74, 0x10, 0x01, // CFG-UART1OUTPROT-NMEA (=1)
                // CFG-UART1-BAUDRATE (=115200)
                // 0x01, 0x00, 0x52, 0x40, 0x00, 0xc2, 0x01, 0x00,
                // payload end
                0x00, // ck_a
                0x00, // ck_b
            ];
            let len = frame.len() as u16 - 8;
            frame[4..6].copy_from_slice(&len.to_le_bytes());
            ubx_fill_ck(&mut frame);
            frame
        };

        let ubx_ack_ack_frame = {
            let mut frame = [
                0xb5, 0x62, // header
                0x05, 0x01, // id/class (=UBX-ACK-ACK)
                0x02, 0x00, // length
                // payload begin
                0x06, 0x8a, // id/class (=UBX-CFG-VALSET)
                // payload end
                0x00, // ck_a
                0x00, // ck_b
            ];
            ubx_fill_ck(&mut frame);
            frame
        };

        while gnss_i2c
            .write_async(0x42_u16, ubx_cfg_valset_frame)
            .await
            .is_err()
        {
            Timer::after_millis(100).await; // wait for the bus to become ready
        }

        loop {
            let mut len = [0; 2];
            defmt::unwrap!(
                gnss_i2c
                    .write_read_async(0x42_u16, [0xfd_u8], &mut len)
                    .await
            );
            if u16::from_be_bytes(len) >= 10 {
                break;
            }
        }

        let mut buf = [0; 10];
        defmt::unwrap!(gnss_i2c.read_async(0x42_u16, &mut buf).await);
        defmt::assert_eq!(buf, ubx_ack_ack_frame);
    }

    let mut nmea_buf = [0; 512];
    let mut state: State<'_, 512> = State::NmeaNotFound;
    loop {
        let mut buf = [0; 64];
        let len = defmt::unwrap!(gnss_uart.read(&mut buf).await);
        for c in &buf[..len] {
            state = match (state, c) {
                (State::NmeaNotFound | State::Payload { .. }, b'$') => State::Doller,
                (State::NmeaNotFound | State::Payload { .. }, _) => State::Doller,
                (State::Doller, _) => State::Address1(*c),
                (State::Address1(c0), _) => State::Address2(c0, *c),
                (State::Address2(c0, c1), _) => State::Address3(c0, c1, *c),
                (State::Address3(c0, c1, c2), _) => State::Address4(c0, c1, c2, *c),
                (State::Address4(c0, c1, c2, c3), _) => State::IncompletePayload {
                    address: (c0, c1, c2, c3, *c),
                    buf: &mut nmea_buf,
                    len: 0,
                },
                (State::IncompletePayload { address, buf, len }, b'\n') => {
                    buf[len] = *c;
                    State::Payload {
                        address,
                        buf,
                        len: len + 1,
                    }
                }
                (State::IncompletePayload { address, buf, len }, _) => {
                    buf[len] = *c;
                    State::IncompletePayload {
                        address,
                        buf,
                        len: len + 1,
                    }
                }
            };

            if let State::Payload {
                address: address @ (_, _, b'R', b'M', b'C'),
                ref buf,
                len,
            } = state
            {
                if len > 7 && buf[1..7].iter().all(|&c| c.is_ascii_digit()) {
                    defmt::info!("{:?}, {:?}", address, &buf[..len]);

                    let tx_buf = [
                        to_digit(buf[6]),
                        to_digit(buf[5]),
                        to_digit(buf[4]) | MASK_DP,
                        to_digit(buf[3]),
                        to_digit(buf[2]) | MASK_DP,
                        to_digit(buf[1]),
                    ];

                    display_spi.write(&tx_buf).await.unwrap();

                    display_le.set_high();
                    Timer::after_nanos(15).await;
                    display_le.set_low();
                }
            }
        }
    }
}
