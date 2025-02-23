use embassy_rp::Peripheral;
use embassy_rp::flash::{self, Blocking, ERASE_SIZE, Flash, Instance};

#[derive(defmt::Format, Debug)]
pub enum Error {
    DataTooLarge(usize),
    BufferTooSmall(usize),
    Flash(flash::Error),
    Postcard(postcard::Error),
}

pub struct NonVolatileConfig<'d, C, T, const FLASH_SIZE: usize, const OFFSET: u32, const N: usize>
where
    T: Instance,
{
    buf: [u8; N],
    flash: Flash<'d, T, Blocking, FLASH_SIZE>,
    _pd: core::marker::PhantomData<C>,
}

impl<'d, C, T, const FLASH_SIZE: usize, const OFFSET: u32, const N: usize>
    NonVolatileConfig<'d, C, T, FLASH_SIZE, OFFSET, N>
where
    T: Instance,
    C: Default + serde::Serialize + serde::de::DeserializeOwned,
{
    pub fn new(flash: impl Peripheral<P = T> + 'd) -> Self {
        Self {
            buf: [0; N],
            flash: Flash::<_, Blocking, FLASH_SIZE>::new_blocking(flash),
            _pd: core::marker::PhantomData,
        }
    }

    pub fn read_or_default(&mut self) -> Result<C, Error> {
        match self.read() {
            r @ Ok(..) | r @ Err(Error::Flash(..)) => r,
            _ => {
                defmt::info!("fallback to default config");
                let default = C::default();
                self.write(&default)?;
                Ok(default)
            }
        }
    }

    pub fn read(&mut self) -> Result<C, Error> {
        self.flash.blocking_read(OFFSET, self.buf.as_mut_slice())?;
        Ok(postcard::from_bytes_cobs(self.buf.as_mut_slice())?)
    }

    pub fn write(&mut self, value: &C) -> Result<(), Error> {
        let data = postcard::to_slice_cobs(value, self.buf.as_mut_slice())?;
        let erase_size = ((data.len() + ERASE_SIZE) / ERASE_SIZE * ERASE_SIZE) as u32;
        self.flash.blocking_erase(OFFSET, OFFSET + erase_size)?;
        self.flash.blocking_write(OFFSET, data)?;
        Ok(())
    }
}

impl From<flash::Error> for Error {
    fn from(e: flash::Error) -> Self {
        Self::Flash(e)
    }
}

impl From<postcard::Error> for Error {
    fn from(e: postcard::Error) -> Self {
        Self::Postcard(e)
    }
}
