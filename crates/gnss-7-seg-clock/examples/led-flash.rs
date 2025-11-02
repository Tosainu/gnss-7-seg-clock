#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_rp::gpio;
use embassy_time::Timer;

use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());
    let mut leds = [
        gpio::Output::new(p.PIN_1, gpio::Level::Low),
        gpio::Output::new(p.PIN_2, gpio::Level::Low),
        gpio::Output::new(p.PIN_3, gpio::Level::Low),
        gpio::Output::new(p.PIN_4, gpio::Level::Low),
        gpio::Output::new(p.PIN_5, gpio::Level::Low),
    ];

    let pattern = [
        0b00000, 0b00001, 0b00011, 0b00110, 0b01100, 0b11000, 0b10000, 0b00000, 0b10000, 0b11000,
        0b01100, 0b00110, 0b00011, 0b00001,
    ];

    loop {
        for p in pattern {
            for (i, led) in leds.iter_mut().enumerate() {
                led.set_level(if p & (1 << i) > 0 {
                    gpio::Level::High
                } else {
                    gpio::Level::Low
                })
            }
            Timer::after_millis(50).await;
        }
    }
}
