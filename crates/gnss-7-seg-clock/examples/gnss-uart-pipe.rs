#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_rp::gpio;
use embassy_rp::peripherals::{UART1, USB};
use embassy_rp::uart;
use embassy_rp::usb;
use embassy_usb::class::cdc_acm::{self, CdcAcmClass};
use embedded_io_async::{Read, Write};
use static_cell::StaticCell;

use {defmt_rtt as _, panic_probe as _};

embassy_rp::bind_interrupts!(struct Irqs {
    UART1_IRQ => uart::BufferedInterruptHandler<UART1>;
    USBCTRL_IRQ => usb::InterruptHandler<USB>;
});

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let mut sw3 = gpio::Input::new(p.PIN_0, gpio::Pull::None);
    let mut gnss_nreset = gpio::Output::new(p.PIN_16, gpio::Level::Low);

    let uart1_config = {
        let mut c = uart::Config::default();
        c.baudrate = 9600;
        c.parity = uart::Parity::ParityNone;
        c.stop_bits = uart::StopBits::STOP1;
        c
    };
    let (uart1_tx, uart1_rx) = {
        static UART1_BUF_TX: StaticCell<[u8; 64]> = StaticCell::new();
        static UART1_BUF_RX: StaticCell<[u8; 64]> = StaticCell::new();
        uart::BufferedUart::new(
            p.UART1,
            p.PIN_20,
            p.PIN_21,
            Irqs,
            UART1_BUF_TX.init([0; 64]).as_mut_slice(),
            UART1_BUF_RX.init([0; 64]).as_mut_slice(),
            uart1_config,
        )
    }
    .split();

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

    let (usb_cdc_acm_tx, usb_cdc_acm_rx) = {
        static STATE: StaticCell<cdc_acm::State> = StaticCell::new();
        let state = STATE.init(cdc_acm::State::new());
        CdcAcmClass::new(&mut usb_builder, state, 64)
    }
    .split();

    defmt::unwrap!(spawner.spawn(usb(usb_builder.build())));
    defmt::unwrap!(spawner.spawn(uart1_to_usb(uart1_rx, usb_cdc_acm_tx)));
    defmt::unwrap!(spawner.spawn(usb_to_uart1(usb_cdc_acm_rx, uart1_tx)));

    loop {
        sw3.wait_for_falling_edge().await;
        gnss_nreset.toggle();
        match gnss_nreset.get_output_level() {
            gpio::Level::High => defmt::info!("GNSS ON"),
            gpio::Level::Low => defmt::info!("GNSS OFF"),
        }
    }
}

#[embassy_executor::task]
async fn usb(mut usb: embassy_usb::UsbDevice<'static, usb::Driver<'static, USB>>) -> ! {
    usb.run().await
}

#[embassy_executor::task]
async fn uart1_to_usb(
    mut uart1: uart::BufferedUartRx,
    mut usb: cdc_acm::Sender<'static, usb::Driver<'static, USB>>,
) {
    loop {
        usb.wait_connection().await;
        defmt::info!("connected");
        let mut buf = [0; 64];
        while let Ok(n) = uart1.read(&mut buf).await {
            if n == 0 {
                continue;
            }
            if usb.write_packet(&buf[..n]).await.is_err() {
                break;
            }
            if n == 64 && usb.write_packet(&[]).await.is_err() {
                break;
            }
        }
        defmt::info!("disconnected");
    }
}

#[embassy_executor::task]
async fn usb_to_uart1(
    mut usb: cdc_acm::Receiver<'static, usb::Driver<'static, USB>>,
    mut uart1: uart::BufferedUartTx,
) {
    loop {
        usb.wait_connection().await;
        defmt::info!("connected");
        let mut buf = [0; 64];
        while let Ok(n) = usb.read_packet(&mut buf).await {
            if n > 0 {
                uart1.write_all(&buf[..n]).await.unwrap();
            }
        }
        defmt::info!("disconnected");
    }
}
