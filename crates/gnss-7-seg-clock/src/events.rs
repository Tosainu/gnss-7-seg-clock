use embassy_futures::select::*;
use embassy_rp::gpio;
use embassy_sync::{blocking_mutex::raw::RawMutex, channel::Receiver};
use embassy_time::Timer;

use chrono::NaiveDateTime;

use crate::max_m10s::Event as MaxM10sEvent;

pub enum Event {
    DateTimeAndVelocity {
        datetime: NaiveDateTime,
        ground_speed_meter_hour: u32,
    },
    DateTimeNextPulse(NaiveDateTime),
    Sw3Pressed,
    Sw4Pressed,
    Sw5Pressed,
    TimePulse,
}

pub struct EventSources<'d, M: RawMutex, const N: usize> {
    receiver_nmea: Receiver<'d, M, MaxM10sEvent, N>,
    gpio_sw3: DebouncedInput<'d>,
    gpio_sw4: DebouncedInput<'d>,
    gpio_sw5: DebouncedInput<'d>,
    gpio_pps: gpio::Input<'d>,
    pub datetime: Option<NaiveDateTime>,
    pub datetime_next_pulse: Option<NaiveDateTime>,
}

impl<'d, M: RawMutex, const N: usize> EventSources<'d, M, N> {
    pub fn new(
        receiver_nmea: Receiver<'d, M, MaxM10sEvent, N>,
        gpio_sw3: gpio::Input<'d>,
        gpio_sw4: gpio::Input<'d>,
        gpio_sw5: gpio::Input<'d>,
        gpio_pps: gpio::Input<'d>,
    ) -> Self {
        Self {
            receiver_nmea,
            gpio_sw3: DebouncedInput(gpio_sw3),
            gpio_sw4: DebouncedInput(gpio_sw4),
            gpio_sw5: DebouncedInput(gpio_sw5),
            gpio_pps,
            datetime: None,
            datetime_next_pulse: None,
        }
    }

    pub async fn wait(&mut self) -> Event {
        #[allow(clippy::never_loop)]
        loop {
            match select5(
                self.receiver_nmea.receive(),
                self.gpio_sw3.wait_for_falling_edge(),
                self.gpio_sw4.wait_for_falling_edge(),
                self.gpio_sw5.wait_for_falling_edge(),
                self.gpio_pps.wait_for_rising_edge(),
            )
            .await
            {
                Either5::First(MaxM10sEvent::DateTimeAndVelocity {
                    datetime,
                    ground_speed_meter_hour,
                }) => {
                    self.datetime = Some(datetime);
                    return Event::DateTimeAndVelocity {
                        datetime,
                        ground_speed_meter_hour,
                    };
                }
                Either5::First(MaxM10sEvent::DateTimeNextPulse(datetime)) => {
                    self.datetime_next_pulse = Some(datetime);
                    return Event::DateTimeNextPulse(datetime);
                }
                Either5::Second(..) => return Event::Sw3Pressed,
                Either5::Third(..) => return Event::Sw4Pressed,
                Either5::Fourth(..) => return Event::Sw5Pressed,
                Either5::Fifth(..) => {
                    self.datetime_next_pulse = None;
                    return Event::TimePulse;
                }
            }
        }
    }
}

struct DebouncedInput<'d>(gpio::Input<'d>);

impl DebouncedInput<'_> {
    async fn wait_for_falling_edge(&mut self) {
        loop {
            self.0.wait_for_falling_edge().await;
            Timer::after_millis(20).await;
            if self.0.get_level() == gpio::Level::Low {
                break;
            }
        }
    }
}
