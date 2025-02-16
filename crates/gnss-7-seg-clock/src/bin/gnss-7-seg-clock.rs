#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_rp::gpio;
use embassy_rp::i2c;
use embassy_rp::peripherals::{I2C1, SPI1, UART1};
use embassy_rp::spi;
use embassy_rp::uart;
use embassy_sync::{blocking_mutex::raw::ThreadModeRawMutex, channel::Channel, signal::Signal};
use embassy_time::{Duration, Instant, Timer};
use static_cell::StaticCell;

use chrono::{FixedOffset, NaiveTime, TimeDelta, Timelike};

use gnss_7_seg_clock::max_m10s::MaxM10s;

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

struct DisplayPayload([u8; 6]);

#[allow(dead_code)]
enum DisplayCommand {
    Set(DisplayPayload),
    SetOnPulse(DisplayPayload, Instant),
}

static DISPLAY_SIGNAL: Signal<ThreadModeRawMutex, DisplayCommand> = Signal::new();

fn time_to_display_payload(time: NaiveTime) -> DisplayPayload {
    DisplayPayload([
        TABLE[time.second() as usize % 10],
        TABLE[time.second() as usize / 10 % 10],
        TABLE[time.minute() as usize % 10] | MASK_DP,
        TABLE[time.minute() as usize / 10 % 10],
        TABLE[time.hour() as usize % 10] | MASK_DP,
        TABLE[time.hour() as usize / 10 % 10],
    ])
}

const TIME_ZOME: FixedOffset = FixedOffset::east_opt(9 * 60 * 60).unwrap();

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    defmt::info!("Hello World!");

    let mut display_noe = gpio::Output::new(p.PIN_11, gpio::Level::High);
    let mut display_le = gpio::Output::new(p.PIN_13, gpio::Level::Low);
    let gnss_pps = gpio::Input::new(p.PIN_19, gpio::Pull::Down);

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

    let _uart1_tx = gpio::Input::new(p.PIN_20, gpio::Pull::Up);
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

    defmt::unwrap!(spawner.spawn(task_display(display_spi, display_le, gnss_pps)));
    defmt::unwrap!(spawner.spawn(task_max_m10s(max_m10s, &NMEA_CHANNEL)));

    loop {
        let msg = NMEA_CHANNEL.receive().await;
        defmt::debug!("{}", msg);

        if let nmea::ParseResult::RMC(data) = msg {
            if let Some(time) = data.fix_time {
                let local = time + TIME_ZOME + TimeDelta::seconds(1);
                DISPLAY_SIGNAL.signal(DisplayCommand::SetOnPulse(
                    time_to_display_payload(local),
                    Instant::now() + Duration::from_secs(1),
                ));
            }
        }
    }
}

#[embassy_executor::task]
async fn task_display(
    mut spi: spi::Spi<'static, SPI1, spi::Async>,
    mut gpio_le: gpio::Output<'static>,
    mut gpio_pulse: gpio::Input<'static>,
) {
    loop {
        match DISPLAY_SIGNAL.wait().await {
            DisplayCommand::Set(payload) => {
                spi.write(&payload.0).await.unwrap();
            }
            DisplayCommand::SetOnPulse(payload, timeout) => {
                spi.write(&payload.0).await.unwrap();

                let _ = embassy_futures::select::select(
                    gpio_pulse.wait_for_rising_edge(),
                    Timer::at(timeout),
                )
                .await;
            }
        }

        gpio_le.set_high();
        Timer::after_nanos(15).await;
        gpio_le.set_low();
    }
}

#[embassy_executor::task]
async fn task_max_m10s(mut max_m10s: MaxM10s<'static, UART1, I2C1>, channel: &'static NmeaChannel) {
    max_m10s.run(channel.sender()).await;
}
