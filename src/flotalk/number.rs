use super::error::*;

use std::ops::*;

///
/// The valid numeric values for a FloTalk number
///
#[derive(Copy, Clone, PartialEq, Debug)]
pub enum TalkNumber {
    Int(i64),
    Float(f64),
}

impl TalkNumber {
    ///
    /// Returns the integer values of this number, if it's an integer
    ///
    pub fn try_as_int(self) -> Result<i64, TalkError> {
        match self {
            TalkNumber::Int(val)    => Ok(val),
            TalkNumber::Float(_)    => Err(TalkError::NotAnInteger),
        }
    }

    ///
    /// Returns the value of this number as a floating point value
    ///
    pub fn as_float(self) -> f64 {
        match self {
            TalkNumber::Int(val)    => val as _,
            TalkNumber::Float(val)  => val,
        }
    }
}
