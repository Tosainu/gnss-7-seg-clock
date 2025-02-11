#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_rp::gpio;
use embassy_rp::spi;
use embassy_time::Timer;

use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let display_spi_config = {
        let mut c = spi::Config::default();
        c.frequency = 30_000_000;
        c.phase = spi::Phase::CaptureOnFirstTransition;
        c.polarity = spi::Polarity::IdleLow;
        c
    };
    let mut display_spi = spi::Spi::new(
        p.SPI1,
        p.PIN_14,
        p.PIN_15,
        p.PIN_12,
        p.DMA_CH0,
        p.DMA_CH1,
        display_spi_config,
    );

    let mut display_noe = gpio::Output::new(p.PIN_11, gpio::Level::High);
    let mut display_le = gpio::Output::new(p.PIN_13, gpio::Level::Low);

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

    display_noe.set_low();

    loop {
        for i in 0..TABLE.len() {
            #[allow(clippy::identity_op)]
            let tx_buf = [
                TABLE[(i + 5) % TABLE.len()] | if i & 1 == 0 { MASK_DP } else { 0 },
                TABLE[(i + 4) % TABLE.len()] | if i & 1 == 1 { MASK_DP } else { 0 },
                TABLE[(i + 3) % TABLE.len()] | if i & 1 == 0 { MASK_DP } else { 0 },
                TABLE[(i + 2) % TABLE.len()] | if i & 1 == 1 { MASK_DP } else { 0 },
                TABLE[(i + 1) % TABLE.len()] | if i & 1 == 0 { MASK_DP } else { 0 },
                TABLE[(i + 0) % TABLE.len()] | if i & 1 == 1 { MASK_DP } else { 0 },
            ];
            display_spi.write(&tx_buf).await.unwrap();

            display_le.set_high();
            Timer::after_nanos(15).await;
            display_le.set_low();

            Timer::after_millis(500).await;
        }
    }
}
