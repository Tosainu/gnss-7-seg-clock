#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_rp::gpio;
use embassy_rp::i2c;
use embassy_rp::peripherals::{I2C1, UART1};
use embassy_rp::uart;
use embassy_time::Timer;
use embedded_io_async::Read;
use static_cell::StaticCell;

use {defmt_rtt as _, panic_probe as _};

use ubx::{UbxFrame, UbxStream};

embassy_rp::bind_interrupts!(struct Irqs {
    I2C1_IRQ => i2c::InterruptHandler<I2C1>;
    UART1_IRQ => uart::BufferedInterruptHandler<UART1>;
});

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let mut gnss_nreset = gpio::Output::new(p.PIN_16, gpio::Level::Low);

    let mut i2c = i2c::I2c::new_async(p.I2C1, p.PIN_23, p.PIN_22, Irqs, i2c::Config::default());

    let uart_config = {
        let mut c = uart::Config::default();
        c.baudrate = 115200;
        c.parity = uart::Parity::ParityNone;
        c.stop_bits = uart::StopBits::STOP1;
        c
    };
    let mut uart = {
        static UART1_BUF_TX: StaticCell<[u8; 64]> = StaticCell::new();
        static UART1_BUF_RX: StaticCell<[u8; 2048]> = StaticCell::new();
        uart::BufferedUart::new(
            p.UART1,
            p.PIN_20,
            p.PIN_21,
            Irqs,
            UART1_BUF_TX.init([0; 64]).as_mut_slice(),
            UART1_BUF_RX.init([0; 2048]).as_mut_slice(),
            uart_config,
        )
    };
    uart.set_baudrate(115200);

    gnss_nreset.set_low();
    Timer::after_millis(500).await;
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
                0x01, 0x00, 0x21, 0x30, 0xc8, 0x00, // CFG-RATE-MEAS (=200 ms/5 Hz)
                0x1b, 0x00, 0x91, 0x20, 0x01, // CFG-MSGOUT-UBX_NAV_STATUS_UART1 (=1)
                0x43, 0x00, 0x91, 0x20, 0x01, // CFG-MSGOUT-UBX_NAV_VELNED_UART1 (=1)
                0x01, 0x00, 0x71, 0x10, 0x01, // CFG-I2CINPROT-UBX (=1)
                0x02, 0x00, 0x71, 0x10, 0x00, // CFG-I2CINPROT-NMEA (=0)
                0x01, 0x00, 0x72, 0x10, 0x01, // CFG-I2COUTPROT-UBX (=1)
                0x02, 0x00, 0x72, 0x10, 0x00, // CFG-I2COUTPROT-NMEA (=0)
                0x01, 0x00, 0x73, 0x10, 0x01, // CFG-UART1INPROT-UBX (=1)
                0x02, 0x00, 0x73, 0x10, 0x00, // CFG-UART1INPROT-NMEA (=0)
                0x01, 0x00, 0x74, 0x10, 0x01, // CFG-UART1OUTPROT-UBX (=1)
                0x02, 0x00, 0x74, 0x10, 0x00, // CFG-UART1OUTPROT-NMEA (=0)
                // CFG-UART1-BAUDRATE (=115200)
                0x01, 0x00, 0x52, 0x40, 0x00, 0xc2, 0x01, 0x00, // payload end
                0x00, // ck_a
                0x00, // ck_b
            ];
            let len = frame.len() as u16 - 8;
            frame[4..6].copy_from_slice(&len.to_le_bytes());
            (frame[frame.len() - 2], frame[frame.len() - 1]) =
                ubx::checksum(&frame[2..frame.len() - 2]);
            frame
        };

        while let Err(e) = i2c.write_async(0x42_u16, ubx_cfg_valset_frame).await {
            defmt::debug!("{}", e);
            Timer::after_millis(100).await;
        }
    }

    let mut buf = UbxStream::<2048>::new();

    loop {
        match uart.read(buf.buf_unused_mut()).await {
            Ok(len) => buf.commit(len),
            Err(e) => {
                defmt::warn!("error while reading UART: {}", e);
                continue;
            }
        }

        while let Some(frame) = buf.pop() {
            defmt::debug!("class = {:02x}, id = {:02x}", frame.class, frame.id,);

            match frame {
                // UBX-ACK-ACK for UBX-CFG-VALSET
                UbxFrame {
                    class: 0x05,
                    id: 0x01,
                    payload: &[0x06, 0x8a],
                } => defmt::info!("UBX-CFG-VALSET ack"),

                // UBX-ACK-NAK for UBX-CFG-VALSET
                UbxFrame {
                    class: 0x05,
                    id: 0x00,
                    payload: &[0x06, 0x8a],
                } => defmt::info!("UBX-CFG-VALSET nak"),

                _ => (),
            }
        }

        if buf.buf_filled().len() == 2048 {
            defmt::warn!("UbxStream full");
            buf.consume(2048);
        }
    }
}
