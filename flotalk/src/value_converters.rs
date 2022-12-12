use super::context::*;
use super::error::*;
use super::message::*;
use super::number::*;
use super::reference::*;
use super::releasable::*;
use super::value::*;

use std::sync::*;

impl TalkValueType for () {
    #[inline]
    fn try_into_talk_value<'a>(self, context: &'a TalkContext) -> Result<TalkOwned<'a, TalkValue>, TalkError> {
        Ok(TalkOwned::new(self.into(), context))
    }

    #[inline]
    fn try_from_talk_value<'a>(value: TalkOwned<'a, TalkValue>, context: &'a TalkContext) -> Result<Self, TalkError> {
        match &*value {
            TalkValue::Nil          => Ok(()),
            TalkValue::Message(msg) => {
                let msg = value.leak();
                if let TalkValue::Message(msg) = msg {
                    Self::from_message(TalkOwned::new(*msg, context), context)
                } else {
                    unreachable!()
                }
            }

            _                       => Err(TalkError::UnexpectedClass)
        }
    }
}

impl TalkValueType for TalkReference {
    #[inline]
    fn try_into_talk_value<'a>(self, context: &'a TalkContext) -> Result<TalkOwned<'a, TalkValue>, TalkError> {
        Ok(TalkOwned::new(self.into(), context))
    }

    #[inline]
    fn try_from_talk_value<'a>(value: TalkOwned<'a, TalkValue>, context: &'a TalkContext) -> Result<Self, TalkError> {
        match &*value {
            TalkValue::Reference(val)   => {
                let reference = value.leak();
                if let TalkValue::Reference(val) = reference {
                    Ok(val)
                } else {
                    unreachable!()
                }
            }

            TalkValue::Message(msg)     => {
                let msg = value.leak();
                if let TalkValue::Message(msg) = msg {
                    Self::from_message(TalkOwned::new(*msg, context), context)
                } else {
                    unreachable!()
                }
            }

            _                           => Err(TalkError::NotAReference)
        }
    }
}

impl TalkValueType for bool {
    #[inline]
    fn try_into_talk_value<'a>(self, context: &'a TalkContext) -> Result<TalkOwned<'a, TalkValue>, TalkError> {
        Ok(TalkOwned::new(self.into(), context))
    }

    #[inline]
    fn try_from_talk_value<'a>(value: TalkOwned<'a, TalkValue>, context: &'a TalkContext) -> Result<Self, TalkError> {
        match &*value {
            TalkValue::Bool(val)    => Ok(*val as _),
            TalkValue::Message(msg) => {
                let msg = value.leak();
                if let TalkValue::Message(msg) = msg {
                    Self::from_message(TalkOwned::new(*msg, context), context)
                } else {
                    unreachable!()
                }
            }

            _                       => Err(TalkError::NotABoolean)
        }
    }
}

impl TalkValueType for i32 {
    #[inline]
    fn try_into_talk_value<'a>(self, context: &'a TalkContext) -> Result<TalkOwned<'a, TalkValue>, TalkError> {
        Ok(TalkOwned::new(self.into(), context))
    }

    #[inline]
    fn try_from_talk_value<'a>(value: TalkOwned<'a, TalkValue>, context: &'a TalkContext) -> Result<Self, TalkError> {
        match &*value {
            TalkValue::Int(val)     => Ok(*val as _),
            TalkValue::Float(val)   => Ok(*val as _),
            TalkValue::Message(msg) => {
                let msg = value.leak();
                if let TalkValue::Message(msg) = msg {
                    Self::from_message(TalkOwned::new(*msg, context), context)
                } else {
                    unreachable!()
                }
            }

            _                       => Err(TalkError::NotAnInteger)
        }
    }
}

impl TalkValueType for i64 {
    #[inline]
    fn try_into_talk_value<'a>(self, context: &'a TalkContext) -> Result<TalkOwned<'a, TalkValue>, TalkError> {
        Ok(TalkOwned::new(self.into(), context))
    }

    #[inline]
    fn try_from_talk_value<'a>(value: TalkOwned<'a, TalkValue>, context: &'a TalkContext) -> Result<Self, TalkError> {
        match &*value {
            TalkValue::Int(val)     => Ok(*val as _),
            TalkValue::Float(val)   => Ok(*val as _),
            TalkValue::Message(msg) => {
                let msg = value.leak();
                if let TalkValue::Message(msg) = msg {
                    Self::from_message(TalkOwned::new(*msg, context), context)
                } else {
                    unreachable!()
                }
            }

            _                       => Err(TalkError::NotAnInteger)
        }
    }
}

impl TalkValueType for f32 {
    #[inline]
    fn try_into_talk_value<'a>(self, context: &'a TalkContext) -> Result<TalkOwned<'a, TalkValue>, TalkError> {
        Ok(TalkOwned::new(self.into(), context))
    }

    #[inline]
    fn try_from_talk_value<'a>(value: TalkOwned<'a, TalkValue>, context: &'a TalkContext) -> Result<Self, TalkError> {
        match &*value {
            TalkValue::Int(val)     => Ok(*val as _),
            TalkValue::Float(val)   => Ok(*val as _),
            TalkValue::Message(msg) => {
                let msg = value.leak();
                if let TalkValue::Message(msg) = msg {
                    Self::from_message(TalkOwned::new(*msg, context), context)
                } else {
                    unreachable!()
                }
            }

            _                       => Err(TalkError::NotAFloat)
        }
    }
}

impl TalkValueType for f64 {
    #[inline]
    fn try_into_talk_value<'a>(self, context: &'a TalkContext) -> Result<TalkOwned<'a, TalkValue>, TalkError> {
        Ok(TalkOwned::new(self.into(), context))
    }

    #[inline]
    fn try_from_talk_value<'a>(value: TalkOwned<'a, TalkValue>, context: &'a TalkContext) -> Result<Self, TalkError> {
        match &*value {
            TalkValue::Int(val)     => Ok(*val as _),
            TalkValue::Float(val)   => Ok(*val as _),
            TalkValue::Message(msg) => {
                let msg = value.leak();
                if let TalkValue::Message(msg) = msg {
                    Self::from_message(TalkOwned::new(*msg, context), context)
                } else {
                    unreachable!()
                }
            }

            _                       => Err(TalkError::NotAFloat)
        }
    }
}

impl TalkValueType for String {
    #[inline]
    fn try_into_talk_value<'a>(self, context: &'a TalkContext) -> Result<TalkOwned<'a, TalkValue>, TalkError> {
        Ok(TalkOwned::new(self.into(), context))
    }

    #[inline]
    fn try_from_talk_value<'a>(value: TalkOwned<'a, TalkValue>, context: &'a TalkContext) -> Result<Self, TalkError> {
        match &*value {
            TalkValue::String(val)  => Ok((**val).clone()),
            TalkValue::Message(msg) => {
                let msg = value.leak();
                if let TalkValue::Message(msg) = msg {
                    Self::from_message(TalkOwned::new(*msg, context), context)
                } else {
                    unreachable!()
                }
            }

            _                       => Err(TalkError::NotAString)
        }
    }
}

impl TalkValueType for Arc<String> {
    #[inline]
    fn try_into_talk_value<'a>(self, context: &'a TalkContext) -> Result<TalkOwned<'a, TalkValue>, TalkError> {
        Ok(TalkOwned::new(self.into(), context))
    }

    #[inline]
    fn try_from_talk_value<'a>(value: TalkOwned<'a, TalkValue>, context: &'a TalkContext) -> Result<Self, TalkError> {
        match &*value {
            TalkValue::String(val)  => Ok(val.clone()),
            TalkValue::Message(msg) => {
                let msg = value.leak();
                if let TalkValue::Message(msg) = msg {
                    Self::from_message(TalkOwned::new(*msg, context), context)
                } else {
                    unreachable!()
                }
            }

            _                       => Err(TalkError::NotAString)
        }
    }
}

impl TalkValueType for char {
    #[inline]
    fn try_into_talk_value<'a>(self, context: &'a TalkContext) -> Result<TalkOwned<'a, TalkValue>, TalkError> {
        Ok(TalkOwned::new(self.into(), context))
    }

    #[inline]
    fn try_from_talk_value<'a>(value: TalkOwned<'a, TalkValue>, context: &'a TalkContext) -> Result<Self, TalkError> {
        match &*value {
            TalkValue::Character(val)   => Ok(*val as _),
            TalkValue::Message(msg)     => {
                let msg = value.leak();
                if let TalkValue::Message(msg) = msg {
                    Self::from_message(TalkOwned::new(*msg, context), context)
                } else {
                    unreachable!()
                }
            }

            _                       => Err(TalkError::NotACharacter)
        }
    }
}

impl TalkValueType for TalkNumber {
    #[inline]
    fn try_into_talk_value<'a>(self, context: &'a TalkContext) -> Result<TalkOwned<'a, TalkValue>, TalkError> {
        Ok(TalkOwned::new(self.into(), context))
    }

    #[inline]
    fn try_from_talk_value<'a>(value: TalkOwned<'a, TalkValue>, context: &'a TalkContext) -> Result<Self, TalkError> {
        match &*value {
            TalkValue::Int(val)     => Ok(TalkNumber::Int(*val)),
            TalkValue::Float(val)   => Ok(TalkNumber::Float(*val)),
            TalkValue::Message(msg) => {
                let msg = value.leak();
                if let TalkValue::Message(msg) = msg {
                    Self::from_message(TalkOwned::new(*msg, context), context)
                } else {
                    unreachable!()
                }
            }

            _                       => Err(TalkError::NotANumber)
        }
    }
}

impl TalkValueType for TalkError {
    #[inline]
    fn try_into_talk_value<'a>(self, context: &'a TalkContext) -> Result<TalkOwned<'a, TalkValue>, TalkError> {
        Ok(TalkOwned::new(self.into(), context))
    }

    #[inline]
    fn try_from_talk_value<'a>(value: TalkOwned<'a, TalkValue>, context: &'a TalkContext) -> Result<Self, TalkError> {
        match &*value {
            TalkValue::Error(err)   => Ok(err.clone()),
            TalkValue::Message(msg) => {
                let msg = value.leak();
                if let TalkValue::Message(msg) = msg {
                    Self::from_message(TalkOwned::new(*msg, context), context)
                } else {
                    unreachable!()
                }
            }

            _                       => Err(TalkError::NotAnError)
        }
    }
}

impl<T> TalkValueType for Vec<T>
where
    T : TalkValueType + Sized,
{
    #[inline]
    fn try_into_talk_value<'a>(self, context: &'a TalkContext) -> Result<TalkOwned<'a, TalkValue>, TalkError> {
        let array = self.into_iter()
            .map(|val| val.try_into_talk_value(context).map(|val| val.leak()))
            .collect::<Result<Vec<_>, TalkError>>()?;

        Ok(TalkOwned::new(TalkValue::Array(array), context))
    }

    #[inline]
    fn try_from_talk_value<'a>(value: TalkOwned<'a, TalkValue>, context: &'a TalkContext) -> Result<Self, TalkError> {
        match &*value {
            TalkValue::Array(vals)  => {
                let mut result = vec![];

                for value in vals.iter() {
                    let value = TalkOwned::new(value.clone_in_context(context), context);
                    result.push(T::try_from_talk_value(value, context)?);
                }

                Ok(result)
            }

            _                       => Err(TalkError::NotAnArray)
        }
    }
}
