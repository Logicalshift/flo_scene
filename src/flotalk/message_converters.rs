use super::context::*;
use super::error::*;
use super::message::*;
use super::reference::*;
use super::value::*;

use std::sync::*;

lazy_static! {
    static ref VALUE_MSG: TalkMessageSignatureId        = "value".into();
    static ref VALUE_COLON_MSG: TalkMessageSignatureId  = "value:".into();
}


impl TalkMessageType for () {
    fn from_message(message: TalkMessage, _context: &TalkContext) -> Result<Self, TalkError> {
        if let TalkMessage::Unary(_any_message) = message {
            Ok(())
        } else {
            Err(TalkError::MessageNotSupported(message.signature_id()))
        }
    }

    fn to_message(&self, _context: &mut TalkContext) -> TalkMessage {
        TalkMessage::Unary(*VALUE_MSG)
    }
}

impl TalkMessageType for TalkReference {
    fn from_message(message: TalkMessage, _context: &TalkContext) -> Result<Self, TalkError> {
        if let TalkMessage::WithArguments(signature, mut args) = message {
            if args.len() == 1 {
                match args[0].take() {
                    TalkValue::Reference(reference) => Ok(reference),
                    _                               => Err(TalkError::NotAReference),
                }
            } else {
                Err(TalkError::MessageNotSupported(signature))
            }
        } else {
            Err(TalkError::MessageNotSupported(message.signature_id()))
        }
    }

    fn to_message(&self, _context: &mut TalkContext) -> TalkMessage {
        unimplemented!()
    }
}

impl TalkMessageType for TalkValue {
    fn from_message(message: TalkMessage, _context: &TalkContext) -> Result<Self, TalkError> {
        unimplemented!()
    }

    fn to_message(&self, _context: &mut TalkContext) -> TalkMessage {
        unimplemented!()
    }
}

impl TalkMessageType for bool {
    fn from_message(message: TalkMessage, _context: &TalkContext) -> Result<Self, TalkError> {
        unimplemented!()
    }

    fn to_message(&self, _context: &mut TalkContext) -> TalkMessage {
        unimplemented!()
    }
}

impl TalkMessageType for i32 {
    fn from_message(message: TalkMessage, _context: &TalkContext) -> Result<Self, TalkError> {
        unimplemented!()
    }

    fn to_message(&self, _context: &mut TalkContext) -> TalkMessage {
        unimplemented!()
    }
}

impl TalkMessageType for i64 {
    fn from_message(message: TalkMessage, _context: &TalkContext) -> Result<Self, TalkError> {
        unimplemented!()
    }

    fn to_message(&self, _context: &mut TalkContext) -> TalkMessage {
        unimplemented!()
    }
}

impl TalkMessageType for f32 {
    fn from_message(message: TalkMessage, _context: &TalkContext) -> Result<Self, TalkError> {
        unimplemented!()
    }

    fn to_message(&self, _context: &mut TalkContext) -> TalkMessage {
        unimplemented!()
    }
}

impl TalkMessageType for f64 {
    fn from_message(message: TalkMessage, _context: &TalkContext) -> Result<Self, TalkError> {
        unimplemented!()
    }

    fn to_message(&self, _context: &mut TalkContext) -> TalkMessage {
        unimplemented!()
    }
}

impl TalkMessageType for &str {
    fn from_message(message: TalkMessage, _context: &TalkContext) -> Result<Self, TalkError> {
        unimplemented!()
    }

    fn to_message(&self, _context: &mut TalkContext) -> TalkMessage {
        unimplemented!()
    }
}

impl TalkMessageType for String {
    fn from_message(message: TalkMessage, _context: &TalkContext) -> Result<Self, TalkError> {
        unimplemented!()
    }

    fn to_message(&self, _context: &mut TalkContext) -> TalkMessage {
        unimplemented!()
    }
}

impl TalkMessageType for Arc<String> {
    fn from_message(message: TalkMessage, _context: &TalkContext) -> Result<Self, TalkError> {
        unimplemented!()
    }

    fn to_message(&self, _context: &mut TalkContext) -> TalkMessage {
        unimplemented!()
    }
}

impl TalkMessageType for char {
    fn from_message(message: TalkMessage, _context: &TalkContext) -> Result<Self, TalkError> {
        unimplemented!()
    }

    fn to_message(&self, _context: &mut TalkContext) -> TalkMessage {
        unimplemented!()
    }
}
