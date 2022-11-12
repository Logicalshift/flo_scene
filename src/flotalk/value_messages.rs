use super::continuation::*;
use super::context::*;
use super::dispatch_table::*;
use super::error::*;
use super::message::*;
use super::value::*;

use smallvec::*;

lazy_static! {
    // Object protocol message signatures

    /// Returns true if the two objects are equivalent
    pub static ref TALK_BINARY_EQUALS: TalkMessageSignatureId                   = ("=").into();

    /// Returns true if two objects are the same object
    pub static ref TALK_BINARY_EQUALS_EQUALS: TalkMessageSignatureId            = ("==").into();

    /// Returns true if the two objects are not equivalent
    pub static ref TALK_BINARY_TILDE_EQUALS: TalkMessageSignatureId             = ("~=").into();

    /// Returns true of two objects are not the same object
    pub static ref TALK_BINARY_TILDE_TILDE: TalkMessageSignatureId              = ("~~").into();

    /// Returns the class object of the receiver
    pub static ref TALK_MSG_CLASS: TalkMessageSignatureId                       = "class".into();

    /// Creates a copy of the receiver
    pub static ref TALK_MSG_COPY: TalkMessageSignatureId                        = "copy".into();

    /// A message was sent to the receiver that has no behaviour defined for it
    pub static ref TALK_MSG_DOES_NOT_UNDERSTAND: TalkMessageSignatureId         = ("doesNotUnderstand:").into();

    /// Reports that an error occurred
    pub static ref TALK_MSG_ERROR: TalkMessageSignatureId                       = ("error:").into();

    /// Returns a hash code for this object
    pub static ref TALK_MSG_HASH: TalkMessageSignatureId                        = "hash".into();

    /// Returns a hash code for the identity of this object
    pub static ref TALK_MSG_IDENTITY_HASH: TalkMessageSignatureId               = "identityHash".into();

    /// Returns true if the object is an instance of a subclass of the specified class, or the class itself
    pub static ref TALK_MSG_IS_KIND_OF: TalkMessageSignatureId                  = ("isKindOf:").into();

    /// Returns true if the object is an instance of the specified class
    pub static ref TALK_MSG_IS_MEMBER_OF: TalkMessageSignatureId                = ("isMemberOf:").into();

    /// Returns true if this is the nil object
    pub static ref TALK_MSG_IS_NIL: TalkMessageSignatureId                      = "isNil".into();

    /// Returns true if this is not the nil object
    pub static ref TALK_MSG_NOT_NIL: TalkMessageSignatureId                     = "notNil".into();

    /// Performs the specified selector on the object
    pub static ref TALK_MSG_PERFORM: TalkMessageSignatureId                     = ("perform:").into();

    /// Performs the specified selector on the object, with the specified arguments
    pub static ref TALK_MSG_PERFORM_WITH: TalkMessageSignatureId                = ("perform:", "with:").into();

    /// Performs the specified selector on the object, with the specified arguments
    pub static ref TALK_MSG_PERFORM_WITH_WITH: TalkMessageSignatureId           = ("perform:", "with:", "with:").into();

    /// Performs the specified selector on the object, with the specified arguments
    pub static ref TALK_MSG_PERFORM_WITH_WITH_WITH: TalkMessageSignatureId      = ("perform:", "with:", "with:", "with:").into();

    /// Performs the specified selector on the object, with the specified arguments
    pub static ref TALK_MSG_PERFORM_WITH_ARGUMENTS: TalkMessageSignatureId      = ("perform:", "withAruments:").into();

    /// Writes a description of the object to a stream
    pub static ref TALK_MSG_PRINT_ON: TalkMessageSignatureId                    = ("printOn:").into();

    /// Returns a string description of the receiver
    pub static ref TALK_MSG_PRINT_STRING: TalkMessageSignatureId                = "printString".into();

    /// True if the receiver can respond to a message selector
    pub static ref TALK_MSG_RESPONDS_TO: TalkMessageSignatureId                 = ("respondsTo:").into();

    /// Returns the receiver as the result
    pub static ref TALK_MSG_YOURSELF: TalkMessageSignatureId                    = "yourself".into();
}

lazy_static! {
    // Valuable protocol messages

    pub static ref TALK_MSG_VALUE: TalkMessageSignatureId                       = "value".into();
    pub static ref TALK_MSG_WHILE_FALSE: TalkMessageSignatureId                 = "whileFalse".into();
    pub static ref TALK_MSG_WHILE_FALSE_COLON: TalkMessageSignatureId           = ("whileFalse:").into();
    pub static ref TALK_MSG_WHILE_TRUE: TalkMessageSignatureId                  = "whileTrue".into();
    pub static ref TALK_MSG_WHILE_TRUE_COLON: TalkMessageSignatureId            = ("whileTrue:").into();
}

lazy_static! {
    // Boolean protocol messages

    pub static ref TALK_BINARY_AND: TalkMessageSignatureId                      = ("&").into();
    pub static ref TALK_BINARY_OR: TalkMessageSignatureId                       = ("|").into();
    pub static ref TALK_MSG_AND: TalkMessageSignatureId                         = ("and:").into();
    pub static ref TALK_MSG_OR: TalkMessageSignatureId                          = ("or:").into();
    pub static ref TALK_MSG_XOR: TalkMessageSignatureId                         = ("xor:").into();
    pub static ref TALK_MSG_EQV: TalkMessageSignatureId                         = ("eqv:").into();
    pub static ref TALK_MSG_IF_FALSE: TalkMessageSignatureId                    = ("ifFalse:").into();
    pub static ref TALK_MSG_IF_FALSE_IF_TRUE: TalkMessageSignatureId            = ("ifFalse:", "ifTrue:").into();
    pub static ref TALK_MSG_IF_TRUE: TalkMessageSignatureId                     = ("ifTrue:").into();
    pub static ref TALK_MSG_IF_TRUE_IF_FALSE: TalkMessageSignatureId            = ("ifTrue:", "ifFalse:").into();
    pub static ref TALK_MSG_NOT: TalkMessageSignatureId                         = "not".into();
}

lazy_static! {
    // Number protocol messages

    pub static ref TALK_BINARY_ADD: TalkMessageSignatureId                      = ("+").into();
    pub static ref TALK_BINARY_SUB: TalkMessageSignatureId                      = ("-").into();
    pub static ref TALK_BINARY_MUL: TalkMessageSignatureId                      = ("*").into();
    pub static ref TALK_BINARY_DIV: TalkMessageSignatureId                      = ("/").into();
    pub static ref TALK_BINARY_DIV_TRUNCATE: TalkMessageSignatureId             = ("//").into();
    pub static ref TALK_BINARY_LT: TalkMessageSignatureId                       = ("<").into();
    pub static ref TALK_BINARY_GT: TalkMessageSignatureId                       = (">").into();
    pub static ref TALK_BINARY_REMAINDER: TalkMessageSignatureId                = ("\\").into();
    pub static ref TALK_MSG_ABS: TalkMessageSignatureId                         = "abs".into();
    pub static ref TALK_MSG_AS_FLOAT: TalkMessageSignatureId                    = "asFloat".into();
    pub static ref TALK_MSG_AS_FLOAT_D: TalkMessageSignatureId                  = "asFloatD".into();
    pub static ref TALK_MSG_AS_FLOAT_E: TalkMessageSignatureId                  = "asFloatE".into();
    pub static ref TALK_MSG_AS_FLOAT_Q: TalkMessageSignatureId                  = "asFloatQ".into();
    pub static ref TALK_MSG_AS_FRACTION: TalkMessageSignatureId                 = "asFraction".into();
    pub static ref TALK_MSG_AS_INTEGER: TalkMessageSignatureId                  = "asInteger".into();
    pub static ref TALK_MSG_AS_SCALED_DECIMAL: TalkMessageSignatureId           = ("asScaledDecimal:").into();
    pub static ref TALK_MSG_CEILING: TalkMessageSignatureId                     = "ceiling".into();
    pub static ref TALK_MSG_FLOOR: TalkMessageSignatureId                       = "floor".into();
    pub static ref TALK_MSG_FRACTION_PART: TalkMessageSignatureId               = "fractionPart".into();
    pub static ref TALK_MSG_INTEGER_PART: TalkMessageSignatureId                = "integerPart".into();
    pub static ref TALK_MSG_NEGATED: TalkMessageSignatureId                     = "negated".into();
    pub static ref TALK_MSG_NEGATIVE: TalkMessageSignatureId                    = "negative".into();
    pub static ref TALK_MSG_POSITIVE: TalkMessageSignatureId                    = "positive".into();
    pub static ref TALK_MSG_QUO: TalkMessageSignatureId                         = ("quo:").into();
    pub static ref TALK_MSG_RAISED_TO: TalkMessageSignatureId                   = ("raisedTo:").into();
    pub static ref TALK_MSG_RAISED_TO_INTEGER: TalkMessageSignatureId           = ("rasiedToInteger:").into();
    pub static ref TALK_MSG_RECIPROCAL: TalkMessageSignatureId                  = "reciprocal".into();
    pub static ref TALK_MSG_REM: TalkMessageSignatureId                         = ("rem:").into();
    pub static ref TALK_MSG_ROUNDED: TalkMessageSignatureId                     = "rounded".into();
    pub static ref TALK_MSG_ROUND_TO: TalkMessageSignatureId                    = ("roundTo:").into();
    pub static ref TALK_MSG_SIGN: TalkMessageSignatureId                        = "sign".into();
    pub static ref TALK_MSG_SQRT: TalkMessageSignatureId                        = "sqrt".into();
    pub static ref TALK_MSG_SQUARED: TalkMessageSignatureId                     = "squared".into();
    pub static ref TALK_MSG_STRICTLY_POSITIVE: TalkMessageSignatureId           = "strictlyPositive".into();
    pub static ref TALK_MSG_TO: TalkMessageSignatureId                          = ("to:").into();
    pub static ref TALK_MSG_TO_BY: TalkMessageSignatureId                       = ("to:", "by:").into();
    pub static ref TALK_MSG_TO_BY_DO: TalkMessageSignatureId                    = ("to:", "by:", "do:").into();
    pub static ref TALK_MSG_TO_DO: TalkMessageSignatureId                       = ("to:", "do:").into();
    pub static ref TALK_MSG_TRUNCATED: TalkMessageSignatureId                   = "truncated".into();
    pub static ref TALK_MSG_TRACATE_TO: TalkMessageSignatureId                  = ("truncateTo:").into();
}

lazy_static! {
    pub static ref TALK_DISPATCH_BOOLEAN: TalkMessageDispatchTable<bool> = TalkMessageDispatchTable::empty()
        .with_message(*TALK_BINARY_AND, |val, args| Ok::<_, TalkError>(val & args[0].try_as_bool()?))
        ;
}

impl TalkValue {
    ///
    /// Performs the default behaviour for a message when sent to a TalkValue
    ///
    pub fn default_send_message_in_context(&self, message: TalkMessage, talk_context: &mut TalkContext) -> TalkContinuation {
        TalkContinuation::Ready(TalkValue::Error(TalkError::MessageNotSupported(message.signature_id())))
    }
}
