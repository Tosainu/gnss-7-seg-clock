#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_futures::select::*;
use embassy_rp::gpio;
use embassy_rp::i2c;
use embassy_rp::peripherals::{I2C1, UART1};
use embassy_rp::uart;
use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, channel::Channel};
use static_cell::StaticCell;

use chrono::{Datelike, FixedOffset, NaiveDate, NaiveTime, TimeDelta, Timelike};

use gnss_7_seg_clock::{
    display::{self, Display},
    max_m10s::MaxM10s,
};

use {defmt_rtt as _, panic_probe as _};

embassy_rp::bind_interrupts!(struct Irqs {
    I2C1_IRQ => i2c::InterruptHandler<I2C1>;
    UART1_IRQ => uart::BufferedInterruptHandler<UART1>;
});

type NmeaChannel = Channel<ThreadModeRawMutex, nmea::ParseResult, 16>;

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

fn date_to_display_payload(date: NaiveDate) -> display::Payload {
    display::Payload([
        TABLE[date.day() as usize % 10],
        TABLE[date.day() as usize / 10 % 10],
        TABLE[date.month() as usize % 10] | MASK_DP,
        TABLE[date.month() as usize / 10 % 10],
        TABLE[date.year() as usize % 10] | MASK_DP,
        TABLE[date.year() as usize / 10 % 10],
    ])
}

fn time_to_display_payload(time: NaiveTime) -> display::Payload {
    display::Payload([
        TABLE[time.second() as usize % 10],
        TABLE[time.second() as usize / 10 % 10],
        TABLE[time.minute() as usize % 10] | MASK_DP,
        TABLE[time.minute() as usize / 10 % 10],
        TABLE[time.hour() as usize % 10] | MASK_DP,
        TABLE[time.hour() as usize / 10 % 10],
    ])
}

const TIME_ZOME: FixedOffset = FixedOffset::east_opt(9 * 60 * 60).unwrap();

#[derive(Copy, Clone, PartialEq)]
enum DisplayMode {
    Date,
    Time,
}

impl DisplayMode {
    fn next_state(&self) -> DisplayMode {
        match self {
            DisplayMode::Date => DisplayMode::Time,
            DisplayMode::Time => DisplayMode::Date,
        }
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    defmt::info!("Hello World!");

    let mut sw3 = gpio::Input::new(p.PIN_0, gpio::Pull::None);

    let _spi1_rx = gpio::Input::new(p.PIN_12, gpio::Pull::Down);
    let mut display = Display::new(p.SPI1, p.PIN_14, p.PIN_15, p.DMA_CH0, p.PIN_11, p.PIN_13);

    // "--.--.--"
    const PATTERN_NO_TIME: display::Payload = display::Payload([
        0b10000000_u8,
        0b10000000_u8,
        0b10100000_u8,
        0b10000000_u8,
        0b10100000_u8,
        0b10000000_u8,
    ]);

    display.shift(&PATTERN_NO_TIME).await;
    display.refresh().await;
    display.output(true);

    let _uart1_tx = gpio::Input::new(p.PIN_20, gpio::Pull::Up);
    let mut max_m10s_pps = gpio::Input::new(p.PIN_19, gpio::Pull::Down);
    let max_m10s = {
        static UART1_BUF_RX: StaticCell<[u8; 1024]> = StaticCell::new();
        MaxM10s::new(
            p.UART1,
            p.PIN_21,
            UART1_BUF_RX.init([0; 1024]).as_mut_slice(),
            p.I2C1,
            p.PIN_23,
            p.PIN_22,
            p.PIN_16,
            p.PIN_18,
            Irqs,
        )
    };

    static NMEA_CHANNEL: NmeaChannel = NmeaChannel::new();

    defmt::unwrap!(spawner.spawn(task_max_m10s(max_m10s, &NMEA_CHANNEL)));

    max_m10s_pps.wait_for_low().await;

    let mut mode = DisplayMode::Time;
    let mut datetime = None;
    let mut refresh_on_pps = false;

    loop {
        match select3(
            NMEA_CHANNEL.receive(),
            sw3.wait_for_falling_edge(),
            max_m10s_pps.wait_for_rising_edge(),
        )
        .await
        {
            Either3::First(msg) => {
                defmt::debug!("{}", msg);
                if let nmea::ParseResult::RMC(data) = msg {
                    if let (Some(date), Some(time)) = (data.fix_date, data.fix_time) {
                        let t = date.and_time(time) + TIME_ZOME;
                        let t_next = t + TimeDelta::seconds(1);

                        match mode {
                            DisplayMode::Date => {
                                display.shift(&date_to_display_payload(t.date())).await;
                                display.refresh().await;
                                display.shift(&date_to_display_payload(t_next.date())).await;
                                refresh_on_pps = true;
                            }
                            DisplayMode::Time => {
                                display.shift(&time_to_display_payload(t.time())).await;
                                display.refresh().await;
                                display.shift(&time_to_display_payload(t_next.time())).await;
                                refresh_on_pps = true;
                            }
                        }

                        datetime = Some(t);
                    }
                }
            }

            Either3::Second(..) => {
                mode = mode.next_state();
                if let Some(t) = datetime {
                    let t_next = t + TimeDelta::seconds(1);
                    match mode {
                        DisplayMode::Date => {
                            display.shift(&date_to_display_payload(t.date())).await;
                            display.refresh().await;
                            display.shift(&date_to_display_payload(t_next.date())).await;
                            refresh_on_pps = true;
                        }
                        DisplayMode::Time => {
                            display.shift(&time_to_display_payload(t.time())).await;
                            display.refresh().await;
                            display.shift(&time_to_display_payload(t_next.time())).await;
                            refresh_on_pps = true;
                        }
                    }
                } else {
                    display.shift(&PATTERN_NO_TIME).await;
                    display.refresh().await;
                    refresh_on_pps = false;
                }
            }

            Either3::Third(..) => {
                if refresh_on_pps {
                    display.refresh().await;
                    refresh_on_pps = false;
                }
            }
        }
    }
}

#[embassy_executor::task]
async fn task_max_m10s(mut max_m10s: MaxM10s<'static, UART1, I2C1>, channel: &'static NmeaChannel) {
    max_m10s.run(channel.sender()).await;
}
