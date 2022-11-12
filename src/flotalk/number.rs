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

impl Add for TalkNumber {
    type Output = TalkNumber;

    fn add(self, to: TalkNumber) -> TalkNumber {
        match (self, to) {
            (TalkNumber::Int(val1), TalkNumber::Int(val2))  => TalkNumber::Int(val1 + val2),
            (val1, val2)                                    => TalkNumber::Float(val1.as_float() + val2.as_float()),
        }
    }
}

impl Sub for TalkNumber {
    type Output = TalkNumber;

    fn sub(self, to: TalkNumber) -> TalkNumber {
        match (self, to) {
            (TalkNumber::Int(val1), TalkNumber::Int(val2))  => TalkNumber::Int(val1 - val2),
            (val1, val2)                                    => TalkNumber::Float(val1.as_float() - val2.as_float()),
        }
    }
}

impl Mul for TalkNumber {
    type Output = TalkNumber;

    fn mul(self, to: TalkNumber) -> TalkNumber {
        match (self, to) {
            (TalkNumber::Int(val1), TalkNumber::Int(val2))  => TalkNumber::Int(val1 * val2),
            (val1, val2)                                    => TalkNumber::Float(val1.as_float() * val2.as_float()),
        }
    }
}

impl Div for TalkNumber {
    type Output = TalkNumber;

    fn div(self, to: TalkNumber) -> TalkNumber {
        match (self, to) {
            (TalkNumber::Int(val1), TalkNumber::Int(val2))  => TalkNumber::Int(val1 / val2),
            (val1, val2)                                    => TalkNumber::Float(val1.as_float() / val2.as_float()),
        }
    }
}

impl Rem for TalkNumber {
    type Output = TalkNumber;

    fn rem(self, to: TalkNumber) -> TalkNumber {
        match (self, to) {
            (TalkNumber::Int(val1), TalkNumber::Int(val2))  => TalkNumber::Int(val1 % val2),
            (val1, val2)                                    => TalkNumber::Float(val1.as_float() % val2.as_float()),
        }
    }
}

impl Neg for TalkNumber {
    type Output = TalkNumber;

    fn neg(self) -> TalkNumber {
        match self {
            TalkNumber::Int(val)    => TalkNumber::Int(-val),
            TalkNumber::Float(val)  => TalkNumber::Float(-val),
        }
    }
}
