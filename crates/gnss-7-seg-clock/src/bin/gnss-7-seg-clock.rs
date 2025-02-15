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

use chrono::{FixedOffset, NaiveTime, Timelike};

use misc::crlf_stream::CrlfStream;

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
        ck_a = ck_a.overflowing_add(*c).0;
        ck_b = ck_b.overflowing_add(ck_a).0;
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

fn time_to_display_payload(time: NaiveTime) -> [u8; 6] {
    [
        TABLE[time.second() as usize % 10] | MASK_DP,
        TABLE[time.second() as usize / 10 % 10],
        TABLE[time.minute() as usize % 10] | MASK_DP,
        TABLE[time.minute() as usize / 10 % 10],
        TABLE[time.hour() as usize % 10] | MASK_DP,
        TABLE[time.hour() as usize / 10 % 10],
    ]
}

const TIME_ZOME: FixedOffset = FixedOffset::east_opt(9 * 60 * 60).unwrap();

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
        c.baudrate = 115200;
        c.parity = uart::Parity::ParityNone;
        c.stop_bits = uart::StopBits::STOP1;
        c
    };
    let mut gnss_uart = {
        static UART1_BUF_RX: StaticCell<[u8; 1024]> = StaticCell::new();
        uart::BufferedUart::new(
            p.UART1,
            Irqs,
            p.PIN_20,
            p.PIN_21,
            [].as_mut_slice(),
            UART1_BUF_RX.init([0; 1024]).as_mut_slice(),
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
                0xd9, 0x00, 0x91, 0x20, 0x01, // CFG-MSGOUT-NMEA_ID_ZDA_UART1 (=1)
                // CFG-UART1-BAUDRATE (=115200)
                0x01, 0x00, 0x52, 0x40, 0x00, 0xc2, 0x01, 0x00,
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

    let mut buf = CrlfStream::<512>::new();

    let mut fix_time = None;

    loop {
        let len = defmt::unwrap!(gnss_uart.read(buf.buf_unused_mut()).await);
        buf.commit(len);

        while let Some(line) = buf.pop() {
            match nmea::parse_bytes(line) {
                Ok(msg) => {
                    defmt::debug!("{}", msg);
                    if let nmea::ParseResult::RMC(data) = msg {
                        fix_time = data.fix_time;
                    }
                }
                // adding `Debug2Format` for now due to the error like:
                // the trait `Format` is not implemented for `nom::internal::Err<nom::error::Error<&str>>`
                Err(err) => defmt::warn!("{:a}: {}", line, defmt::Debug2Format(&err)),
            }
        }

        if let Some(time) = fix_time.take() {
            let local = time + TIME_ZOME;
            defmt::debug!("{}", defmt::Debug2Format(&local));
            display_spi
                .write(&time_to_display_payload(local))
                .await
                .unwrap();

            display_le.set_high();
            Timer::after_nanos(15).await;
            display_le.set_low();
        }
    }
}
