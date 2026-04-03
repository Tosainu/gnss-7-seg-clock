use embassy_rp::{Peri, dma, gpio, interrupt::typelevel::Binding, spi};
use embassy_time::Timer;

pub struct Payload(pub [u8; 6]);

pub struct Display<'d, Spi>
where
    Spi: spi::Instance,
{
    spi: spi::Spi<'d, Spi, spi::Async>,
    gpio_noe: gpio::Output<'d>,
    gpio_le: gpio::Output<'d>,
}

impl<'d, Spi> Display<'d, Spi>
where
    Spi: spi::Instance,
{
    pub fn new<TxDma: dma::ChannelInstance>(
        spi: Peri<'d, Spi>,
        spi_clk: Peri<'d, impl spi::ClkPin<Spi>>,
        spi_tx: Peri<'d, impl spi::MosiPin<Spi>>,
        spi_tx_dma: Peri<'d, TxDma>,
        gpio_noe: Peri<'d, impl gpio::Pin>,
        gpio_le: Peri<'d, impl gpio::Pin>,
        irq: impl Binding<TxDma::Interrupt, dma::InterruptHandler<TxDma>> + 'd,
    ) -> Self {
        let spi_config = {
            let mut c = spi::Config::default();
            c.frequency = 30_000_000;
            c.phase = spi::Phase::CaptureOnFirstTransition;
            c.polarity = spi::Polarity::IdleLow;
            c
        };
        Self {
            spi: spi::Spi::new_txonly(spi, spi_clk, spi_tx, spi_tx_dma, irq, spi_config),
            gpio_noe: gpio::Output::new(gpio_noe, gpio::Level::High),
            gpio_le: gpio::Output::new(gpio_le, gpio::Level::Low),
        }
    }

    pub async fn shift(&mut self, payload: &Payload) {
        self.spi.write(&payload.0).await.unwrap();
    }

    pub async fn refresh(&mut self) {
        self.gpio_le.set_high();
        Timer::after_nanos(15).await;
        self.gpio_le.set_low();
    }

    pub fn output(&mut self, on: bool) {
        self.gpio_noe.set_level(if on {
            gpio::Level::Low
        } else {
            gpio::Level::High
        });
    }
}
