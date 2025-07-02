use core::fmt;

type NomError<'a> = nom::Err<nom::error::Error<&'a str>>;

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, PartialEq)]
pub enum Error<'a> {
    ChecksumMismatch { expected: u8, actual: u8 },
    InvalidFrame,
    ParseError(#[cfg_attr(feature = "defmt", defmt(Debug2Format))] NomError<'a>),
    Utf8Error(#[cfg_attr(feature = "defmt", defmt(Debug2Format))] core::str::Utf8Error),
}

impl fmt::Display for Error<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::ChecksumMismatch { expected, actual } => write!(
                f,
                "checksum mismatch, expected: {expected}, actual: {actual}"
            ),
            Error::InvalidFrame => write!(f, "not an NMEA message"),
            Error::ParseError(e) => e.fmt(f),
            Error::Utf8Error(e) => e.fmt(f),
        }
    }
}

impl<'a> From<nom::Err<nom::error::Error<&'a str>>> for Error<'a> {
    fn from(e: nom::Err<nom::error::Error<&'a str>>) -> Self {
        Self::ParseError(e)
    }
}

impl From<core::str::Utf8Error> for Error<'_> {
    fn from(e: core::str::Utf8Error) -> Self {
        Self::Utf8Error(e)
    }
}

impl core::error::Error for Error<'_> {}
