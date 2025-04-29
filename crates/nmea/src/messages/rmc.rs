use nom::{
    IResult, Parser, bytes::complete::take_until, character::complete::char, combinator::opt,
};

use chrono::{NaiveDate, NaiveTime};

use crate::parser::{date_dmy, time_hms_nano};

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, PartialEq)]
pub struct RmcData {
    #[cfg_attr(feature = "defmt", defmt(Debug2Format))]
    pub time: Option<NaiveTime>,
    #[cfg_attr(feature = "defmt", defmt(Debug2Format))]
    pub date: Option<NaiveDate>,
}

pub fn rmc(i: &str) -> IResult<&str, RmcData> {
    let (i, time) = opt(time_hms_nano).parse(i)?;
    let (i, _) = char(',')(i)?;
    let (i, _) = take_until(",")(i)?; // status
    let (i, _) = char(',')(i)?;
    let (i, _) = take_until(",")(i)?; // lat
    let (i, _) = char(',')(i)?;
    let (i, _) = take_until(",")(i)?; // NS
    let (i, _) = char(',')(i)?;
    let (i, _) = take_until(",")(i)?; // lon
    let (i, _) = char(',')(i)?;
    let (i, _) = take_until(",")(i)?; // EW
    let (i, _) = char(',')(i)?;
    let (i, _) = take_until(",")(i)?; // spd
    let (i, _) = char(',')(i)?;
    let (i, _) = take_until(",")(i)?; // cog
    let (i, _) = char(',')(i)?;
    let (i, date) = opt(date_dmy).parse(i)?;
    let (i, _) = char(',')(i)?;
    Ok((i, RmcData { time, date }))
}
