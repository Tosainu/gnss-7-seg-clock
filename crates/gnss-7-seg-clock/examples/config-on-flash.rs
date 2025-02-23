#![no_std]
#![no_main]

use embassy_executor::Spawner;

use gnss_7_seg_clock::flash::NonVolatileConfig;

use {defmt_rtt as _, panic_probe as _};

const ADDR_OFFSET: u32 = 0x100000;
const FLASH_SIZE: usize = 2 * 1024 * 1024;

#[derive(serde::Serialize, serde::Deserialize, defmt::Format)]
struct Config {
    value: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self { value: 42 }
    }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let mut nvcfg = NonVolatileConfig::<_, _, FLASH_SIZE, ADDR_OFFSET, 512>::new(p.FLASH);
    let mut cfg: Config = defmt::unwrap!(nvcfg.read_or_default());

    defmt::info!("got: {}", cfg);

    cfg.value += 1;

    defmt::info!("writing: {}", cfg);

    defmt::unwrap!(nvcfg.write(&cfg));
}
