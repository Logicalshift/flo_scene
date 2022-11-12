use super::continuation::*;
use super::context::*;
use super::error::*;
use super::message::*;
use super::value::*;

use smallvec::*;

lazy_static! {
    // Object protocol message signatures

    /// Returns true if the two objects are equivalent
    pub static ref TALK_BINARY_EQUALS: TalkMessageSignatureId                   = TalkMessageSignature::Arguments(smallvec!["=".into()]).id();

    /// Returns true if two objects are the same object
    pub static ref TALK_BINARY_EQUALS_EQUALS: TalkMessageSignatureId            = TalkMessageSignature::Arguments(smallvec!["==".into()]).id();

    /// Returns true if the two objects are not equivalent
    pub static ref TALK_BINARY_TILDE_EQUALS: TalkMessageSignatureId             = TalkMessageSignature::Arguments(smallvec!["~=".into()]).id();

    /// Returns true of two objects are not the same object
    pub static ref TALK_BINARY_TILDE_TILDE: TalkMessageSignatureId              = TalkMessageSignature::Arguments(smallvec!["~~".into()]).id();

    /// Returns the class object of the receiver
    pub static ref TALK_MSG_CLASS: TalkMessageSignatureId                       = TalkMessageSignature::Unary("class".into()).id();

    /// Creates a copy of the receiver
    pub static ref TALK_MSG_COPY: TalkMessageSignatureId                        = TalkMessageSignature::Unary("copy".into()).id();

    /// A message was sent to the receiver that has no behaviour defined for it
    pub static ref TALK_MSG_DOES_NOT_UNDERSTAND: TalkMessageSignatureId         = TalkMessageSignature::Arguments(smallvec!["doesNotUnderstand:".into()]).id();

    /// Reports that an error occurred
    pub static ref TALK_MSG_ERROR: TalkMessageSignatureId                       = TalkMessageSignature::Arguments(smallvec!["error:".into()]).id();

    /// Returns a hash code for this object
    pub static ref TALK_MSG_HASH: TalkMessageSignatureId                        = TalkMessageSignature::Unary("hash".into()).id();

    /// Returns a hash code for the identity of this object
    pub static ref TALK_MSG_IDENTITY_HASH: TalkMessageSignatureId               = TalkMessageSignature::Unary("identityHash".into()).id();

    /// Returns true if the object is an instance of a subclass of the specified class, or the class itself
    pub static ref TALK_MSG_IS_KIND_OF: TalkMessageSignatureId                  = TalkMessageSignature::Arguments(smallvec!["isKindOf:".into()]).id();

    /// Returns true if the object is an instance of the specified class
    pub static ref TALK_MSG_IS_MEMBER_OF: TalkMessageSignatureId                = TalkMessageSignature::Arguments(smallvec!["isMemberOf:".into()]).id();

    /// Returns true if this is the nil object
    pub static ref TALK_MSG_IS_NIL: TalkMessageSignatureId                      = TalkMessageSignature::Unary("isNil".into()).id();

    /// Returns true if this is not the nil object
    pub static ref TALK_MSG_NOT_NIL: TalkMessageSignatureId                     = TalkMessageSignature::Unary("notNil".into()).id();

    /// Performs the specified selector on the object
    pub static ref TALK_MSG_PERFORM: TalkMessageSignatureId                     = TalkMessageSignature::Arguments(smallvec!["perform:".into()]).id();

    /// Performs the specified selector on the object, with the specified arguments
    pub static ref TALK_MSG_PERFORM_WITH: TalkMessageSignatureId                = TalkMessageSignature::Arguments(smallvec!["perform:".into(), "with:".into()]).id();

    /// Performs the specified selector on the object, with the specified arguments
    pub static ref TALK_MSG_PERFORM_WITH_WITH: TalkMessageSignatureId           = TalkMessageSignature::Arguments(smallvec!["perform:".into(), "with:".into(), "with:".into()]).id();

    /// Performs the specified selector on the object, with the specified arguments
    pub static ref TALK_MSG_PERFORM_WITH_WITH_WITH: TalkMessageSignatureId      = TalkMessageSignature::Arguments(smallvec!["perform:".into(), "with:".into(), "with:".into(), "with:".into()]).id();

    /// Performs the specified selector on the object, with the specified arguments
    pub static ref TALK_MSG_PERFORM_WITH_ARGUMENTS: TalkMessageSignatureId      = TalkMessageSignature::Arguments(smallvec!["perform:".into(), "withAruments:".into()]).id();

    /// Writes a description of the object to a stream
    pub static ref TALK_MSG_PRINT_ON: TalkMessageSignatureId                    = TalkMessageSignature::Arguments(smallvec!["printOn:".into()]).id();

    /// Returns a string description of the receiver
    pub static ref TALK_MSG_PRINT_STRING: TalkMessageSignatureId                = TalkMessageSignature::Unary("printString".into()).id();

    /// True if the receiver can respond to a message selector
    pub static ref TALK_MSG_RESPONDS_TO: TalkMessageSignatureId                 = TalkMessageSignature::Arguments(smallvec!["respondsTo:".into()]).id();

    /// Returns the receiver as the result
    pub static ref TALK_MSG_YOURSELF: TalkMessageSignatureId                    = TalkMessageSignature::Unary("yourself".into()).id();
}

lazy_static! {
    // Valuable protocol messages

    pub static ref TALK_MSG_VALUE: TalkMessageSignatureId                       = TalkMessageSignature::Unary("value".into()).id();
    pub static ref TALK_MSG_WHILE_FALSE: TalkMessageSignatureId                 = TalkMessageSignature::Unary("whileFalse".into()).id();
    pub static ref TALK_MSG_WHILE_FALSE_COLON: TalkMessageSignatureId           = TalkMessageSignature::Arguments(smallvec!["whileFalse:".into()]).id();
    pub static ref TALK_MSG_WHILE_TRUE: TalkMessageSignatureId                  = TalkMessageSignature::Unary("whileTrue".into()).id();
    pub static ref TALK_MSG_WHILE_TRUE_COLON: TalkMessageSignatureId            = TalkMessageSignature::Arguments(smallvec!["whileTrue:".into()]).id();
}

lazy_static! {
    // Boolean protocol messages

    pub static ref TALK_BINARY_AND: TalkMessageSignatureId                      = TalkMessageSignature::Arguments(smallvec!["&".into()]).id();
    pub static ref TALK_BINARY_OR: TalkMessageSignatureId                       = TalkMessageSignature::Arguments(smallvec!["|".into()]).id();
    pub static ref TALK_MSG_AND: TalkMessageSignatureId                         = TalkMessageSignature::Arguments(smallvec!["and:".into()]).id();
    pub static ref TALK_MSG_OR: TalkMessageSignatureId                          = TalkMessageSignature::Arguments(smallvec!["or:".into()]).id();
    pub static ref TALK_MSG_XOR: TalkMessageSignatureId                         = TalkMessageSignature::Arguments(smallvec!["xor:".into()]).id();
    pub static ref TALK_MSG_EQV: TalkMessageSignatureId                         = TalkMessageSignature::Arguments(smallvec!["eqv:".into()]).id();
    pub static ref TALK_MSG_IF_FALSE: TalkMessageSignatureId                    = TalkMessageSignature::Arguments(smallvec!["ifFalse:".into()]).id();
    pub static ref TALK_MSG_IF_FALSE_IF_TRUE: TalkMessageSignatureId            = TalkMessageSignature::Arguments(smallvec!["ifFalse:".into(), "ifTrue:".into()]).id();
    pub static ref TALK_MSG_IF_TRUE: TalkMessageSignatureId                     = TalkMessageSignature::Arguments(smallvec!["ifTrue:".into()]).id();
    pub static ref TALK_MSG_IF_TRUE_IF_FALSE: TalkMessageSignatureId            = TalkMessageSignature::Arguments(smallvec!["ifTrue:".into(), "ifFalse:".into()]).id();
    pub static ref TALK_MSG_NOT: TalkMessageSignatureId                         = TalkMessageSignature::Unary("not".into()).id();
}

lazy_static! {
    // Number protocol messages

    pub static ref TALK_BINARY_ADD: TalkMessageSignatureId                      = TalkMessageSignature::Arguments(smallvec!["+".into()]).id();
    pub static ref TALK_BINARY_SUB: TalkMessageSignatureId                      = TalkMessageSignature::Arguments(smallvec!["-".into()]).id();
    pub static ref TALK_BINARY_MUL: TalkMessageSignatureId                      = TalkMessageSignature::Arguments(smallvec!["*".into()]).id();
    pub static ref TALK_BINARY_DIV: TalkMessageSignatureId                      = TalkMessageSignature::Arguments(smallvec!["/".into()]).id();
    pub static ref TALK_BINARY_DIV_TRUNCATE: TalkMessageSignatureId             = TalkMessageSignature::Arguments(smallvec!["//".into()]).id();
    pub static ref TALK_BINARY_LT: TalkMessageSignatureId                       = TalkMessageSignature::Arguments(smallvec!["<".into()]).id();
    pub static ref TALK_BINARY_GT: TalkMessageSignatureId                       = TalkMessageSignature::Arguments(smallvec![">".into()]).id();
    pub static ref TALK_BINARY_REMAINDER: TalkMessageSignatureId                = TalkMessageSignature::Arguments(smallvec!["\\".into()]).id();
    pub static ref TALK_MSG_ABS: TalkMessageSignatureId                         = TalkMessageSignature::Unary("abs".into()).id();
    pub static ref TALK_MSG_AS_FLOAT: TalkMessageSignatureId                    = TalkMessageSignature::Unary("asFloat".into()).id();
    pub static ref TALK_MSG_AS_FLOAT_D: TalkMessageSignatureId                  = TalkMessageSignature::Unary("asFloatD".into()).id();
    pub static ref TALK_MSG_AS_FLOAT_E: TalkMessageSignatureId                  = TalkMessageSignature::Unary("asFloatE".into()).id();
    pub static ref TALK_MSG_AS_FLOAT_Q: TalkMessageSignatureId                  = TalkMessageSignature::Unary("asFloatQ".into()).id();
    pub static ref TALK_MSG_AS_FRACTION: TalkMessageSignatureId                 = TalkMessageSignature::Unary("asFraction".into()).id();
    pub static ref TALK_MSG_AS_INTEGER: TalkMessageSignatureId                  = TalkMessageSignature::Unary("asInteger".into()).id();
    pub static ref TALK_MSG_AS_SCALED_DECIMAL: TalkMessageSignatureId           = TalkMessageSignature::Arguments(smallvec!["asScaledDecimal:".into()]).id();
    pub static ref TALK_MSG_CEILING: TalkMessageSignatureId                     = TalkMessageSignature::Unary("ceiling".into()).id();
    pub static ref TALK_MSG_FLOOR: TalkMessageSignatureId                       = TalkMessageSignature::Unary("floor".into()).id();
    pub static ref TALK_MSG_FRACTION_PART: TalkMessageSignatureId               = TalkMessageSignature::Unary("fractionPart".into()).id();
    pub static ref TALK_MSG_INTEGER_PART: TalkMessageSignatureId                = TalkMessageSignature::Unary("integerPart".into()).id();
    pub static ref TALK_MSG_NEGATED: TalkMessageSignatureId                     = TalkMessageSignature::Unary("negated".into()).id();
    pub static ref TALK_MSG_NEGATIVE: TalkMessageSignatureId                    = TalkMessageSignature::Unary("negative".into()).id();
    pub static ref TALK_MSG_POSITIVE: TalkMessageSignatureId                    = TalkMessageSignature::Unary("positive".into()).id();
    pub static ref TALK_MSG_QUO: TalkMessageSignatureId                         = TalkMessageSignature::Arguments(smallvec!["quo:".into()]).id();
    pub static ref TALK_MSG_RAISED_TO: TalkMessageSignatureId                   = TalkMessageSignature::Arguments(smallvec!["raisedTo:".into()]).id();
    pub static ref TALK_MSG_RAISED_TO_INTEGER: TalkMessageSignatureId           = TalkMessageSignature::Arguments(smallvec!["rasiedToInteger:".into()]).id();
    pub static ref TALK_MSG_RECIPROCAL: TalkMessageSignatureId                  = TalkMessageSignature::Unary("reciprocal".into()).id();
    pub static ref TALK_MSG_REM: TalkMessageSignatureId                         = TalkMessageSignature::Arguments(smallvec!["rem:".into()]).id();
    pub static ref TALK_MSG_ROUNDED: TalkMessageSignatureId                     = TalkMessageSignature::Unary("rounded".into()).id();
    pub static ref TALK_MSG_ROUND_TO: TalkMessageSignatureId                    = TalkMessageSignature::Arguments(smallvec!["roundTo:".into()]).id();
    pub static ref TALK_MSG_SIGN: TalkMessageSignatureId                        = TalkMessageSignature::Unary("sign".into()).id();
    pub static ref TALK_MSG_SQRT: TalkMessageSignatureId                        = TalkMessageSignature::Unary("sqrt".into()).id();
    pub static ref TALK_MSG_SQUARED: TalkMessageSignatureId                     = TalkMessageSignature::Unary("squared".into()).id();
    pub static ref TALK_MSG_STRICTLY_POSITIVE: TalkMessageSignatureId           = TalkMessageSignature::Unary("strictlyPositive".into()).id();
    pub static ref TALK_MSG_TO: TalkMessageSignatureId                          = TalkMessageSignature::Arguments(smallvec!["to:".into()]).id();
    pub static ref TALK_MSG_TO_BY: TalkMessageSignatureId                       = TalkMessageSignature::Arguments(smallvec!["to:".into(), "by:".into()]).id();
    pub static ref TALK_MSG_TO_BY_DO: TalkMessageSignatureId                    = TalkMessageSignature::Arguments(smallvec!["to:".into(), "by:".into(), "do:".into()]).id();
    pub static ref TALK_MSG_TO_DO: TalkMessageSignatureId                       = TalkMessageSignature::Arguments(smallvec!["to:".into(), "do:".into()]).id();
    pub static ref TALK_MSG_TRUNCATED: TalkMessageSignatureId                   = TalkMessageSignature::Unary("truncated".into()).id();
    pub static ref TALK_MSG_TRACATE_TO: TalkMessageSignatureId                  = TalkMessageSignature::Arguments(smallvec!["truncateTo:".into()]).id();
}

impl TalkValue {
    ///
    /// Performs the default behaviour for a message when sent to a TalkValue
    ///
    pub fn default_send_message_in_context(&self, message: TalkMessage, talk_context: &mut TalkContext) -> TalkContinuation {
        TalkContinuation::Ready(TalkValue::Error(TalkError::MessageNotSupported))
    }
}
