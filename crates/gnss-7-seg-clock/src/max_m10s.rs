use embassy_futures::select::*;
use embassy_rp::{Peri, gpio, i2c, interrupt::typelevel::Binding, uart};
use embassy_sync::{blocking_mutex::raw::RawMutex, channel::Sender};
use embassy_time::{Duration, Instant, Timer};
use embedded_io_async::Read;

use chrono::NaiveDateTime;

use misc::crlf_stream::CrlfStream;

pub fn ubx_fill_ck(buf: &mut [u8]) {
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

const MAX_M10S_I2C_ADDRESS: u16 = 0x42;

pub struct MaxM10s<'d, I2c>
where
    I2c: i2c::Instance,
{
    uart: uart::BufferedUartRx,
    i2c: i2c::I2c<'d, I2c, i2c::Async>,
    gpio_nreset: gpio::Output<'d>,
    gpio_extint: gpio::Input<'d>,
}

#[derive(Copy, Clone, Debug, PartialEq, defmt::Format)]
enum State {
    PowerCycle,
    Setup,
    Ready,
}

pub enum Event {
    DateTime(NaiveDateTime),
}

impl<'d, I2c> MaxM10s<'d, I2c>
where
    I2c: i2c::Instance,
{
    #[allow(clippy::too_many_arguments)]
    pub fn new<Uart: uart::Instance>(
        uart: Peri<'d, Uart>,
        uart_rx: Peri<'d, impl uart::RxPin<Uart>>,
        uart_rx_buffer: &'d mut [u8],
        i2c: Peri<'d, I2c>,
        i2c_scl: Peri<'d, impl i2c::SclPin<I2c>>,
        i2c_sda: Peri<'d, impl i2c::SdaPin<I2c>>,
        gpio_nreset: Peri<'d, impl gpio::Pin>,
        gpio_extint: Peri<'d, impl gpio::Pin>,
        irq: impl Binding<Uart::Interrupt, uart::BufferedInterruptHandler<Uart>>
        + Binding<I2c::Interrupt, i2c::InterruptHandler<I2c>>
        + Copy,
    ) -> Self {
        let uart_config = {
            let mut c = uart::Config::default();
            c.baudrate = 115200;
            c.parity = uart::Parity::ParityNone;
            c.stop_bits = uart::StopBits::STOP1;
            c
        };

        Self {
            uart: uart::BufferedUartRx::new(uart, irq, uart_rx, uart_rx_buffer, uart_config),
            i2c: i2c::I2c::new_async(i2c, i2c_scl, i2c_sda, irq, i2c::Config::default()),
            gpio_nreset: gpio::Output::new(gpio_nreset, gpio::Level::Low),
            gpio_extint: gpio::Input::new(gpio_extint, gpio::Pull::Up),
        }
    }

    pub async fn run<M: RawMutex, const N: usize>(&mut self, sender: Sender<'_, M, Event, N>) {
        let mut state = State::PowerCycle;
        loop {
            let next_state = match state {
                State::PowerCycle => self.do_power_cycle().await,
                State::Setup => self.do_setup().await,
                State::Ready => self.do_reveive_nmea(&sender).await,
            };
            if next_state != state {
                defmt::info!("MAX-M10S: {} -> {}", state, next_state);
                state = next_state;
            }
        }
    }

    async fn do_power_cycle(&mut self) -> State {
        self.gpio_nreset.set_low();
        Timer::after_millis(500).await;
        self.gpio_nreset.set_high();
        State::Setup
    }

    async fn do_setup(&mut self) -> State {
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

        let t = Instant::now();
        while let Err(e) = self
            .i2c
            .write_async(MAX_M10S_I2C_ADDRESS, ubx_cfg_valset_frame)
            .await
        {
            defmt::debug!("{}", e);
            if Instant::now() > t + Duration::from_secs(5) {
                defmt::warn!("I2C bus or device not ready");
                return State::PowerCycle;
            }
            Timer::after_millis(100).await;
        }

        if let Either::Second(..) =
            select(self.gpio_extint.wait_for_low(), Timer::after_secs(1)).await
        {
            defmt::warn!("EXTINT pin (TX_READY) is not being asserted");
            return State::PowerCycle;
        }

        let mut len = [0; 2];
        if let Err(e) = self
            .i2c
            .write_read_async(MAX_M10S_I2C_ADDRESS, [0xfd_u8], &mut len)
            .await
        {
            defmt::warn!("I2C operation failed ({})", e);
            return State::PowerCycle;
        }

        let len = u16::from_be_bytes(len);
        defmt::debug!("len = {}", len);
        if len < 10 {
            defmt::warn!("unexpected data size ({})", len);
            return State::PowerCycle;
        }

        let mut buf = [0; 10];
        if let Err(e) = self.i2c.read_async(MAX_M10S_I2C_ADDRESS, &mut buf).await {
            defmt::warn!("I2C operation failed ({})", e);
            return State::PowerCycle;
        }

        if buf != ubx_ack_ack_frame {
            defmt::warn!("unexpected data ({}/{})", buf, ubx_ack_ack_frame);
            return State::PowerCycle;
        }

        State::Ready
    }

    async fn do_reveive_nmea<M: RawMutex, const N: usize>(
        &mut self,
        sender: &Sender<'_, M, Event, N>,
    ) -> State {
        let mut buf = CrlfStream::<512>::new();
        let mut errors = 0_u32;
        loop {
            if errors > 10 {
                defmt::warn!("too many UART errors");
                return State::PowerCycle;
            }

            match self.uart.read(buf.buf_unused_mut()).await {
                Ok(len) => buf.commit(len),
                Err(e) => {
                    defmt::warn!("error while reading UART: {}", e);
                    errors += 1;
                    continue;
                }
            }

            while let Some(line) = buf.pop() {
                defmt::debug!("{:a}", line);
                match nmea::parser::parse(line) {
                    Ok(msg) => {
                        if let nmea::parser::MessageType::Rmc(data) = msg.data {
                            defmt::info!("{}", data);
                            if let (Some(date), Some(time)) = (data.date, data.time) {
                                sender.send(Event::DateTime(date.and_time(time))).await;
                            }
                        }
                    }
                    Err(err) => defmt::warn!("{:a}: {}", line, err),
                }
            }

            if buf.buf_filled().len() == 512 {
                defmt::warn!("CrlfStream full");
                buf.consume(512);
            }
        }
    }
}
