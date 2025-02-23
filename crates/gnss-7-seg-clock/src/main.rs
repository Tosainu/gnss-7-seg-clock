#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_rp::flash;
use embassy_rp::gpio;
use embassy_rp::i2c;
use embassy_rp::peripherals::{I2C1, UART1};
use embassy_rp::spi;
use embassy_rp::uart;
use embassy_sync::{
    blocking_mutex::raw::{RawMutex, ThreadModeRawMutex},
    channel::Channel,
};
use static_cell::StaticCell;

use chrono::{Datelike, FixedOffset, NaiveDate, NaiveTime, TimeDelta, Timelike};

use gnss_7_seg_clock::{
    display::{self, Display},
    events::*,
    flash::NonVolatileConfig,
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

// "--.--.--"
const PATTERN_NO_TIME: display::Payload = display::Payload([
    0b10000000_u8,
    0b10000000_u8,
    0b10100000_u8,
    0b10000000_u8,
    0b10100000_u8,
    0b10000000_u8,
]);

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

const FLASH_SIZE: usize = 4 * 1024 * 1024; // W25Q32JVSS
const ADDR_OFFSET: u32 = (FLASH_SIZE - flash::ERASE_SIZE) as u32;

#[derive(Copy, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize, defmt::Format)]
struct Config {
    time_zone_secs: i32,
}

impl Config {
    fn time_zone(&self) -> FixedOffset {
        FixedOffset::east_opt(self.time_zone_secs).unwrap()
    }
}

#[derive(Copy, Clone, PartialEq, defmt::Format)]
enum DisplayMode {
    Time,
    Date,
    ConfigTimeZone,
}

impl DisplayMode {
    fn next_state(&self) -> DisplayMode {
        match self {
            DisplayMode::Time => DisplayMode::Date,
            DisplayMode::Date => DisplayMode::ConfigTimeZone,
            DisplayMode::ConfigTimeZone => DisplayMode::Time,
        }
    }
}

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    defmt::info!("Hello World!");

    let mut nvcfg = NonVolatileConfig::<_, _, FLASH_SIZE, ADDR_OFFSET, 512>::new(p.FLASH);
    let mut cfg: Config = defmt::unwrap!(nvcfg.read_or_default());
    defmt::info!("{}", cfg);

    let sw3 = gpio::Input::new(p.PIN_0, gpio::Pull::None);
    let sw4 = gpio::Input::new(p.PIN_6, gpio::Pull::None);
    let sw5 = gpio::Input::new(p.PIN_7, gpio::Pull::None);

    let _spi1_rx = gpio::Input::new(p.PIN_12, gpio::Pull::Down);
    let mut display = Display::new(p.SPI1, p.PIN_14, p.PIN_15, p.DMA_CH0, p.PIN_11, p.PIN_13);

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
    let mut es = EventSources::new(NMEA_CHANNEL.receiver(), sw3, sw4, sw5, max_m10s_pps);

    loop {
        defmt::info!("mode: {}", mode);
        match mode {
            DisplayMode::Time => handle_mode_time(&mut es, &cfg, &mut display).await,
            DisplayMode::Date => handle_mode_date(&mut es, &cfg, &mut display).await,
            DisplayMode::ConfigTimeZone => {
                let t = handle_mode_config_time_zone(&mut es, &cfg, &mut display).await;
                if t != cfg.time_zone_secs {
                    cfg.time_zone_secs = t;
                    defmt::unwrap!(nvcfg.write(&cfg));
                }
            }
        }
        mode = mode.next_state();
    }
}

async fn handle_mode_time<R: RawMutex, Spi: spi::Instance, const N: usize>(
    es: &mut EventSources<'_, R, N>,
    cfg: &Config,
    display: &mut Display<'_, Spi>,
) {
    if let Some(t) = es.datetime {
        let t = t + cfg.time_zone();
        let t_next = t + TimeDelta::seconds(1);
        display.shift(&time_to_display_payload(t.time())).await;
        display.refresh().await;
        display.shift(&time_to_display_payload(t_next.time())).await;
    } else {
        display.shift(&PATTERN_NO_TIME).await;
        display.refresh().await;
    }

    loop {
        match es.wait().await {
            Event::DateTimeUpdated(t) => {
                let t = t + cfg.time_zone();
                let t_next = t + TimeDelta::seconds(1);
                display.shift(&time_to_display_payload(t.time())).await;
                display.refresh().await;
                display.shift(&time_to_display_payload(t_next.time())).await;
            }
            Event::TimePulse => {
                display.refresh().await;
            }
            Event::Sw3Pressed => return,
            _ => (),
        }
    }
}

async fn handle_mode_date<R: RawMutex, Spi: spi::Instance, const N: usize>(
    es: &mut EventSources<'_, R, N>,
    cfg: &Config,
    display: &mut Display<'_, Spi>,
) {
    if let Some(t) = es.datetime {
        let t = t + cfg.time_zone();
        let t_next = t + TimeDelta::seconds(1);
        display.shift(&date_to_display_payload(t.date())).await;
        display.refresh().await;
        display.shift(&date_to_display_payload(t_next.date())).await;
    } else {
        display.shift(&PATTERN_NO_TIME).await;
        display.refresh().await;
    }

    loop {
        match es.wait().await {
            Event::DateTimeUpdated(t) => {
                let t = t + cfg.time_zone();
                let t_next = t + TimeDelta::seconds(1);
                display.shift(&date_to_display_payload(t.date())).await;
                display.refresh().await;
                display.shift(&date_to_display_payload(t_next.date())).await;
            }
            Event::TimePulse => {
                display.refresh().await;
            }
            Event::Sw3Pressed => return,
            _ => (),
        }
    }
}

async fn handle_mode_config_time_zone<R: RawMutex, Spi: spi::Instance, const N: usize>(
    es: &mut EventSources<'_, R, N>,
    cfg: &Config,
    display: &mut Display<'_, Spi>,
) -> i32 {
    let mut time_zone_secs = cfg.time_zone_secs;
    'outer: loop {
        let hour = time_zone_secs / 60 / 60;
        let min = (time_zone_secs / 60 % 60).unsigned_abs();
        let payload = display::Payload([
            TABLE[min as usize % 10],
            TABLE[min as usize / 10 % 10],
            TABLE[hour.unsigned_abs() as usize % 10] | MASK_DP,
            TABLE[hour.unsigned_abs() as usize / 10 % 10],
            if time_zone_secs.is_negative() {
                0b10000000_u8
            } else {
                0b00000000_u8
            },
            0,
        ]);
        display.shift(&payload).await;
        display.refresh().await;

        time_zone_secs = 'inner: loop {
            match es.wait().await {
                Event::Sw3Pressed => break 'outer time_zone_secs,
                Event::Sw4Pressed => break 'inner (time_zone_secs + 30 * 60).min(24 * 60 * 60),
                Event::Sw5Pressed => break 'inner (time_zone_secs - 30 * 60).max(-24 * 60 * 60),
                _ => (),
            }
        };
    }
}

#[embassy_executor::task]
async fn task_max_m10s(mut max_m10s: MaxM10s<'static, UART1, I2C1>, channel: &'static NmeaChannel) {
    max_m10s.run(channel.sender()).await;
}
