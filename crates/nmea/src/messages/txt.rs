use nom::{IResult, character::complete::char};

use crate::parser::number;

#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, PartialEq)]
pub struct TxtData<'a> {
    /// Total number of messages
    pub num_msg: u8,
    /// Message number
    pub num: u8,
    /// Text identifier
    pub id: u8,
    /// Text
    pub text: &'a str,
}

pub fn txt(i: &str) -> IResult<&str, TxtData<'_>> {
    let (i, num_msg) = number::<u8>(i)?;
    let (i, _) = char(',')(i)?;
    let (i, num) = number::<u8>(i)?;
    let (i, _) = char(',')(i)?;
    let (i, id) = number::<u8>(i)?;
    let (i, _) = char(',')(i)?;
    Ok((
        "",
        TxtData {
            num_msg,
            num,
            id,
            text: i,
        },
    ))
}
