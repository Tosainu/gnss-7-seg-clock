#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_rp::gpio;
use embassy_rp::i2c;
use embassy_rp::peripherals::{I2C1, USB};
use embassy_rp::usb;
use embassy_time::Timer;
use embassy_usb::class::cdc_acm::{self, CdcAcmClass};
use static_cell::StaticCell;

use {defmt_rtt as _, panic_probe as _};

embassy_rp::bind_interrupts!(struct Irqs {
    I2C1_IRQ => i2c::InterruptHandler<I2C1>;
    USBCTRL_IRQ => usb::InterruptHandler<USB>;
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

async fn ubx_read_data_stream<T: i2c::Instance>(
    i2c: &mut i2c::I2c<'_, T, i2c::Async>,
    buf: &mut [u8],
) -> Result<usize, i2c::Error> {
    if buf.len() < 2 {
        return Ok(0);
    }

    let mut len = [0; 2];
    i2c.write_read_async(0x42_u16, [0xfd_u8], &mut len).await?;

    let len = u16::from_be_bytes(len) as usize;
    if len < 2 {
        return Ok(0);
    }

    let len = buf.len().min(len);
    i2c.read_async(0x42_u16, buf).await?;

    Ok(len)
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let mut sw3 = gpio::Input::new(p.PIN_0, gpio::Pull::None);
    let mut gnss_nreset = gpio::Output::new(p.PIN_16, gpio::Level::Low);
    let mut gnss_extint = gpio::Input::new(p.PIN_18, gpio::Pull::Up);

    let mut i2c = i2c::I2c::new_async(p.I2C1, p.PIN_23, p.PIN_22, Irqs, i2c::Config::default());

    let usb_config = {
        let mut c = embassy_usb::Config::new(0x2e8a, 0x75e9);
        c.manufacturer = Some("myon.info");
        c.product = Some("GNSS 7-seg Clock");
        c.serial_number = Some("12345678");
        c.max_power = 100;
        c.max_packet_size_0 = 64;
        c
    };
    let mut usb_builder = {
        static CONFIG_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
        static BOS_DESCRIPTOR: StaticCell<[u8; 256]> = StaticCell::new();
        static CONTROL_BUF: StaticCell<[u8; 64]> = StaticCell::new();
        embassy_usb::Builder::new(
            usb::Driver::new(p.USB, Irqs),
            usb_config,
            CONFIG_DESCRIPTOR.init([0; 256]),
            BOS_DESCRIPTOR.init([0; 256]),
            &mut [],
            CONTROL_BUF.init([0; 64]),
        )
    };

    let mut usb_cdc_acm = {
        static STATE: StaticCell<cdc_acm::State> = StaticCell::new();
        let state = STATE.init(cdc_acm::State::new());
        CdcAcmClass::new(&mut usb_builder, state, 64)
    };

    defmt::unwrap!(spawner.spawn(usb(usb_builder.build())));

    usb_cdc_acm.wait_connection().await;
    defmt::info!("usb ready");

    sw3.wait_for_falling_edge().await;
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
                0x01, 0x00, 0xa2, 0x10, 0x01, // CFG-TXREADY-ENABLED (=true)
                0x02, 0x00, 0xa2, 0x10, 0x01, // CFG-TXREADY-POLARIT (=true=low-active)
                0x03, 0x00, 0xa2, 0x20, 0x05, // CFG-TXREADY-PIN (=5=EXTINT)
                0x04, 0x00, 0xa2, 0x30, 0x10, 0x00, // CFG-TXREADY-THRESHOLD (=128/8)
                0x05, 0x00, 0xa2, 0x20, 0x00, // CFG-TXREADY-INTERFACE (=0=I2C)
                0xd8, 0x00, 0x91, 0x20, 0x01, // CFG-MSGOUT-NMEA_ID_ZDA_I2C (=1)
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

        while i2c
            .write_async(0x42_u16, ubx_cfg_valset_frame)
            .await
            .is_err()
        {
            Timer::after_millis(100).await; // wait for the bus to become ready
        }

        loop {
            let mut len = [0; 2];
            defmt::unwrap!(i2c.write_read_async(0x42_u16, [0xfd_u8], &mut len).await);
            if u16::from_be_bytes(len) >= 10 {
                break;
            }
        }

        let mut buf = [0; 10];
        defmt::unwrap!(i2c.read_async(0x42_u16, &mut buf).await);
        defmt::assert_eq!(buf, ubx_ack_ack_frame);
    }

    defmt::info!("i2c config done");

    loop {
        gnss_extint.wait_for_low().await;

        let mut buf = [0; 2048];
        let len = defmt::unwrap!(ubx_read_data_stream(&mut i2c, &mut buf).await);
        defmt::info!("len = {}", len);

        if len == 0 {
            continue;
        }

        for chunk in buf.chunks(64) {
            defmt::unwrap!(usb_cdc_acm.write_packet(chunk).await);
        }

        if len % 64 == 0 {
            defmt::unwrap!(usb_cdc_acm.write_packet(&[]).await);
        }
    }
}

#[embassy_executor::task]
async fn usb(mut usb: embassy_usb::UsbDevice<'static, usb::Driver<'static, USB>>) -> ! {
    usb.run().await
}
