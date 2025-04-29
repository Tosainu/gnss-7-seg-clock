use nom::{
    IResult, Parser,
    bytes::complete::take,
    character::complete::{anychar, char, digit1},
    combinator::{map_res, opt},
    sequence::{preceded, terminated},
};

use chrono::{NaiveDate, NaiveTime};

use crate::error::*;
use crate::messages::*;

pub(crate) fn number<T: core::str::FromStr>(i: &str) -> IResult<&str, T> {
    map_res(digit1, str::parse::<T>).parse(i)
}

pub(crate) fn time_hms_nano(i: &str) -> IResult<&str, NaiveTime> {
    map_res(
        (
            map_res(take(2usize), str::parse::<u32>),
            map_res(take(2usize), str::parse::<u32>),
            map_res(take(2usize), str::parse::<u32>),
            opt(preceded(char('.'), digit1)),
        ),
        |(h, m, s, nanos)| {
            let nanos = if let Some(nanos) = nanos {
                let num = nanos.parse::<u32>().map_err(|_| "invalid time")?;
                let len = nanos.len() as u32;
                if len > 9 {
                    num / 10_u32.pow(len - 9)
                } else {
                    num * 10_u32.pow(9 - len)
                }
            } else {
                0
            };
            NaiveTime::from_hms_nano_opt(h, m, s, nanos).ok_or("invalid time")
        },
    )
    .parse(i)
}

pub(crate) fn date_dmy(i: &str) -> IResult<&str, NaiveDate> {
    map_res(
        (
            map_res(take(2usize), str::parse::<u32>),
            map_res(take(2usize), str::parse::<u32>),
            map_res(take(2usize), str::parse::<u32>),
        ),
        |(d, m, y)| {
            NaiveDate::from_ymd_opt(
                y as i32 + 2000, // assume 2000's
                m,
                d,
            )
            .ok_or("invalid date")
        },
    )
    .parse(i)
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, PartialEq)]
pub enum MessageType<'a> {
    Rmc(RmcData),
    Txt(TxtData<'a>),
    Unsupported((char, char, char)),
}

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, PartialEq)]
pub struct Message<'a> {
    pub talker: (char, char),
    pub data: MessageType<'a>,
}

fn message(i: &str) -> IResult<&str, Message<'_>> {
    let (i, talker) = (anychar, anychar).parse(i)?;
    let (i, mt) = terminated((anychar, anychar, anychar), char(',')).parse(i)?;
    let (i, data) = match mt {
        ('R', 'M', 'C') => {
            let (i, data) = rmc(i)?;
            (i, MessageType::Rmc(data))
        }
        ('T', 'X', 'T') => {
            let (i, data) = txt(i)?;
            (i, MessageType::Txt(data))
        }
        _ => (i, MessageType::Unsupported(mt)),
    };
    Ok((i, Message { talker, data }))
}

fn hex(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'A'..=b'F' => 0x0a + c - b'A',
        b'a'..=b'f' => 0x0a + c - b'a',
        _ => 0,
    }
}

pub fn parse(s: &[u8]) -> Result<Message<'_>, Error<'_>> {
    let s = match s {
        [b'$', s @ .., b'*', c0, c1, b'\r', b'\n']
            if c0.is_ascii_hexdigit() && c1.is_ascii_hexdigit() =>
        {
            let expected = hex(*c1) | (hex(*c0) << 4);
            let actual = s.iter().fold(0, |c, acc| c ^ acc);
            if expected != actual {
                return Err(Error::ChecksumMismatch { expected, actual });
            }
            s
        }
        [b'$', s @ .., b'\r', b'\n'] => s,
        _ => return Err(Error::InvalidFrame),
    };
    Ok(message(core::str::from_utf8(s)?)?.1)
}

#[cfg(test)]
mod tests {
    extern crate std;

    use crate::parser::*;

    #[test]
    fn test_hms_nano() {
        assert_eq!(
            time_hms_nano("083559"),
            Ok(("", NaiveTime::from_hms_opt(8, 35, 59).unwrap()))
        );
        assert_eq!(
            time_hms_nano("012345.1"),
            Ok((
                "",
                NaiveTime::from_hms_nano_opt(1, 23, 45, 100_000_000).unwrap()
            ))
        );
        assert_eq!(
            time_hms_nano("012345.02"),
            Ok((
                "",
                NaiveTime::from_hms_nano_opt(1, 23, 45, 20_000_000).unwrap()
            ))
        );
        assert_eq!(
            time_hms_nano("012345.000003"),
            Ok(("", NaiveTime::from_hms_nano_opt(1, 23, 45, 3_000).unwrap()))
        );
        assert_eq!(
            time_hms_nano("012345.000000004"),
            Ok(("", NaiveTime::from_hms_nano_opt(1, 23, 45, 4).unwrap()))
        );
        assert_eq!(
            time_hms_nano("012345.0000000005"),
            Ok(("", NaiveTime::from_hms_nano_opt(1, 23, 45, 0).unwrap()))
        );
        assert_eq!(
            time_hms_nano("083559.200"),
            Ok((
                "",
                NaiveTime::from_hms_nano_opt(8, 35, 59, 200_000_000).unwrap()
            ))
        );
        assert_eq!(
            time_hms_nano("083559.200000"),
            Ok((
                "",
                NaiveTime::from_hms_nano_opt(8, 35, 59, 200_000_000).unwrap()
            ))
        );
        assert_eq!(
            time_hms_nano("083559.200000000"),
            Ok((
                "",
                NaiveTime::from_hms_nano_opt(8, 35, 59, 200_000_000).unwrap()
            ))
        );
        assert_eq!(
            time_hms_nano("083559.2000000000"),
            Ok((
                "",
                NaiveTime::from_hms_nano_opt(8, 35, 59, 200_000_000).unwrap()
            ))
        );
    }

    #[test]
    fn test_date_dmy() {
        assert_eq!(
            date_dmy("091202"),
            Ok(("", NaiveDate::from_ymd_opt(2002, 12, 9).unwrap()))
        );
    }

    #[test]
    fn test_message_rmc() {
        assert_eq!(
            message("GPRMC,083559.00,A,4717.11437,N,00833.91522,E,0.004,77.52,091202,,,A,V"),
            Ok((
                ",,A,V",
                Message {
                    talker: ('G', 'P'),
                    data: MessageType::Rmc(RmcData {
                        time: NaiveTime::from_hms_opt(8, 35, 59),
                        date: NaiveDate::from_ymd_opt(2002, 12, 9),
                    }),
                }
            ))
        );
        assert_eq!(
            message("GPRMC,,V,,,,,,,,,,,"),
            Ok((
                ",,,",
                Message {
                    talker: ('G', 'P'),
                    data: MessageType::Rmc(RmcData {
                        time: None,
                        date: None,
                    }),
                }
            ))
        );
    }

    #[test]
    fn test_message_txt() {
        assert_eq!(
            message("GPTXT,01,02,03,ANTARIS ATR0620 HW 00000040"),
            Ok((
                "",
                Message {
                    talker: ('G', 'P'),
                    data: MessageType::Txt(TxtData {
                        num_msg: 1,
                        num: 2,
                        id: 3,
                        text: "ANTARIS ATR0620 HW 00000040"
                    }),
                }
            ))
        );
    }

    #[test]
    fn test_parse() {
        assert_eq!(
            parse(b"$GPTXT,01,01,02,ANTARIS ATR0620 HW 00000040*67\r\n"),
            Ok(Message {
                talker: ('G', 'P'),
                data: MessageType::Txt(TxtData {
                    num_msg: 1,
                    num: 1,
                    id: 2,
                    text: "ANTARIS ATR0620 HW 00000040"
                }),
            })
        );

        assert_eq!(
            parse(b"$GNRMC,081915.00,V,,,,,,,030525,,,N,V*1C\r\n"),
            Ok(Message {
                talker: ('G', 'N'),
                data: MessageType::Rmc(RmcData {
                    time: NaiveTime::from_hms_opt(8, 19, 15),
                    date: NaiveDate::from_ymd_opt(2025, 5, 3),
                }),
            })
        );
    }
}
