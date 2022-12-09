use super::context::*;
use super::error::*;
use super::message::*;
use super::reference::*;
use super::releasable::*;
use super::value::*;

use smallvec::*;

use std::sync::*;

lazy_static! {
    static ref VALUE_MSG: TalkMessageSignatureId        = "value".into();
    static ref VALUE_COLON_MSG: TalkMessageSignatureId  = "value:".into();
}


impl TalkMessageType for () {
    fn from_message<'a>(message: TalkOwned<'a, TalkMessage>, _context: &'a TalkContext) -> Result<Self, TalkError> {
        if let TalkMessage::Unary(_any_message) = &*message {
            Ok(())
        } else {
            Err(TalkError::MessageNotSupported(message.signature_id()))
        }
    }

    fn to_message<'a>(&self, context: &'a mut TalkContext) -> TalkOwned<'a, TalkMessage> {
        TalkOwned::new(TalkMessage::Unary(*VALUE_MSG), context)
    }
}

impl TalkMessageType for TalkReference {
    fn from_message<'a>(message: TalkOwned<'a, TalkMessage>, context: &'a TalkContext) -> Result<Self, TalkError> {
        let signature = message.signature_id();

        if let TalkMessage::WithArguments(_, mut args) = message.leak() {
            if args.len() == 1 {
                match args[0].take() {
                    TalkValue::Reference(reference) => Ok(reference),
                    _                               => {
                        args.release_in_context(context);
                        Err(TalkError::NotAReference)
                    },
                }
            } else {
                args.release_in_context(context);
                Err(TalkError::MessageNotSupported(signature))
            }
        } else {
            Err(TalkError::MessageNotSupported(signature))
        }
    }

    fn to_message<'a>(&self, context: &'a mut TalkContext) -> TalkOwned<'a, TalkMessage> {
        TalkOwned::new(TalkMessage::WithArguments(*VALUE_COLON_MSG, smallvec![TalkValue::Reference(self.clone_in_context(context))]), context)
    }
}

impl TalkMessageType for TalkValue {
    fn from_message<'a>(message: TalkOwned<'a, TalkMessage>, _context: &'a TalkContext) -> Result<Self, TalkError> {
        unimplemented!()
    }

    fn to_message<'a>(&self, _context: &'a mut TalkContext) -> TalkOwned<'a, TalkMessage> {
        unimplemented!()
    }
}

impl TalkMessageType for bool {
    fn from_message<'a>(message: TalkOwned<'a, TalkMessage>, _context: &'a TalkContext) -> Result<Self, TalkError> {
        unimplemented!()
    }

    fn to_message<'a>(&self, _context: &'a mut TalkContext) -> TalkOwned<'a, TalkMessage> {
        unimplemented!()
    }
}

impl TalkMessageType for i32 {
    fn from_message<'a>(message: TalkOwned<'a, TalkMessage>, _context: &'a TalkContext) -> Result<Self, TalkError> {
        unimplemented!()
    }

    fn to_message<'a>(&self, _context: &'a mut TalkContext) -> TalkOwned<'a, TalkMessage> {
        unimplemented!()
    }
}

impl TalkMessageType for i64 {
    fn from_message<'a>(message: TalkOwned<'a, TalkMessage>, _context: &'a TalkContext) -> Result<Self, TalkError> {
        unimplemented!()
    }

    fn to_message<'a>(&self, _context: &'a mut TalkContext) -> TalkOwned<'a, TalkMessage> {
        unimplemented!()
    }
}

impl TalkMessageType for f32 {
    fn from_message<'a>(message: TalkOwned<'a, TalkMessage>, _context: &'a TalkContext) -> Result<Self, TalkError> {
        unimplemented!()
    }

    fn to_message<'a>(&self, _context: &'a mut TalkContext) -> TalkOwned<'a, TalkMessage> {
        unimplemented!()
    }
}

impl TalkMessageType for f64 {
    fn from_message<'a>(message: TalkOwned<'a, TalkMessage>, _context: &'a TalkContext) -> Result<Self, TalkError> {
        unimplemented!()
    }

    fn to_message<'a>(&self, _context: &'a mut TalkContext) -> TalkOwned<'a, TalkMessage> {
        unimplemented!()
    }
}

impl TalkMessageType for &str {
    fn from_message<'a>(message: TalkOwned<'a, TalkMessage>, _context: &'a TalkContext) -> Result<Self, TalkError> {
        unimplemented!()
    }

    fn to_message<'a>(&self, _context: &'a mut TalkContext) -> TalkOwned<'a, TalkMessage> {
        unimplemented!()
    }
}

impl TalkMessageType for String {
    fn from_message<'a>(message: TalkOwned<'a, TalkMessage>, _context: &'a TalkContext) -> Result<Self, TalkError> {
        unimplemented!()
    }

    fn to_message<'a>(&self, _context: &'a mut TalkContext) -> TalkOwned<'a, TalkMessage> {
        unimplemented!()
    }
}

impl TalkMessageType for Arc<String> {
    fn from_message<'a>(message: TalkOwned<'a, TalkMessage>, _context: &'a TalkContext) -> Result<Self, TalkError> {
        unimplemented!()
    }

    fn to_message<'a>(&self, _context: &'a mut TalkContext) -> TalkOwned<'a, TalkMessage> {
        unimplemented!()
    }
}

impl TalkMessageType for char {
    fn from_message<'a>(message: TalkOwned<'a, TalkMessage>, _context: &'a TalkContext) -> Result<Self, TalkError> {
        unimplemented!()
    }

    fn to_message<'a>(&self, _context: &'a mut TalkContext) -> TalkOwned<'a, TalkMessage> {
        unimplemented!()
    }
}
