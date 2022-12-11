use super::continuation::*;
use super::context::*;
use super::dispatch_table::*;
use super::error::*;
use super::message::*;
use super::number::*;
use super::symbol::*;
use super::reference::*;
use super::releasable::*;
use super::value::*;

use smallvec::*;

use std::hash::{Hash, Hasher};
use std::collections::hash_map::{DefaultHasher};
use std::sync::*;

lazy_static! {
    // Class protocol message singatures

    pub static ref TALK_MSG_NEW: TalkMessageSignatureId                         = "new".into();
    pub static ref TALK_MSG_SUPERCLASS: TalkMessageSignatureId                  = "superclass".into();
}

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
    pub static ref TALK_MSG_PERFORM_WITH4: TalkMessageSignatureId               = vec!["perform:", "with:", "with:", "with:", "with:"].into();

    /// Performs the specified selector on the object, with the specified arguments
    pub static ref TALK_MSG_PERFORM_WITH5: TalkMessageSignatureId               = vec!["perform:", "with:", "with:", "with:", "with:", "with:"].into();

    /// Performs the specified selector on the object, with the specified arguments
    pub static ref TALK_MSG_PERFORM_WITH6: TalkMessageSignatureId               = vec!["perform:", "with:", "with:", "with:", "with:", "with:", "with:"].into();

    /// Performs the specified selector on the object, with the specified arguments
    pub static ref TALK_MSG_PERFORM_WITH7: TalkMessageSignatureId               = vec!["perform:", "with:", "with:", "with:", "with:", "with:", "with:", "with:"].into();

    /// Performs the specified selector on the object, with the specified arguments
    pub static ref TALK_MSG_PERFORM_WITH8: TalkMessageSignatureId               = vec!["perform:", "with:", "with:", "with:", "with:", "with:", "with:", "with:", "with:"].into();

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
    pub static ref TALK_MSG_TRUNCATE_TO: TalkMessageSignatureId                 = ("truncateTo:").into();
}

///
/// Implements the various 'perform:with:' selectors
///
#[inline]
fn perform(mut val: TalkOwned<'_, TalkValue>, mut args: TalkOwned<'_, SmallVec<[TalkValue; 4]>>, context: &TalkContext) -> TalkContinuation<'static> {
    // First argument is the selector
    if let TalkValue::Selector(selector) = args[0] {
        // Remove the first argument to create the arguments for the message
        let _ = TalkOwned::new(args.remove(0), context);
        val.take().perform_message_in_context(selector, args, context)
    } else {
        // First argument was not a selector
        TalkError::NotASelector.into()
    }
}

///
/// Implements the 'perform:withArguments:' selector
///
#[inline]
fn perform_with_arguments(mut val: TalkOwned<'_, TalkValue>, args: TalkOwned<'_, SmallVec<[TalkValue; 4]>>, context: &TalkContext) -> TalkContinuation<'static> {
    // First argument is the selector, and second argument is the array
    match (&args[0], &args[1]) {
        (TalkValue::Selector(selector), TalkValue::Array(perform_args)) => {
            // Take the arguments out of the array to claim them for ourselves
            let perform_args = perform_args.iter().map(|arg| arg.clone_in_context(context)).collect();
            let perform_args = TalkOwned::new(perform_args, context);

            // Send the message
            val.take().perform_message_in_context(*selector, perform_args, context)
        }

        (TalkValue::Selector(_), _) => {
            TalkError::NotAnArray.into()
        }

        _ => {
            TalkError::NotASelector.into()
        }
    }
}

#[inline]
fn responds_to(val: TalkOwned<'_, TalkValue>, args: TalkOwned<'_, SmallVec<[TalkValue; 4]>>, context: &TalkContext) -> TalkContinuation<'static> {
    use TalkValue::*;

    match (&*val, &args[0]) {
        (Nil, Selector(selector))           => context.value_dispatch_tables.any_dispatch.responds_to(*selector).into(),
        (Bool(_), Selector(selector))       => context.value_dispatch_tables.bool_dispatch.responds_to(*selector).into(),
        (Int(_), Selector(selector))        => context.value_dispatch_tables.int_dispatch.responds_to(*selector).into(),
        (Float(_), Selector(selector))      => context.value_dispatch_tables.float_dispatch.responds_to(*selector).into(),
        (String(_), Selector(selector))     => context.value_dispatch_tables.string_dispatch.responds_to(*selector).into(),
        (Character(_), Selector(selector))  => context.value_dispatch_tables.character_dispatch.responds_to(*selector).into(),
        (Symbol(_), Selector(selector))     => context.value_dispatch_tables.symbol_dispatch.responds_to(*selector).into(),
        (Selector(_), Selector(selector))   => context.value_dispatch_tables.selector_dispatch.responds_to(*selector).into(),
        (Array(_), Selector(selector))      => context.value_dispatch_tables.array_dispatch.responds_to(*selector).into(),
        (Error(_), Selector(selector))      => context.value_dispatch_tables.error_dispatch.responds_to(*selector).into(),

        (Reference(TalkReference(class_id, _)), Selector(selector)) => {
            if let Some(callbacks) = context.get_callbacks(*class_id) {
                callbacks.responds_to(*selector).into()
            } else {
                false.into()
            }
        }

        _ => TalkError::NotASelector.into()
    }
}

lazy_static! {
    pub static ref TALK_DISPATCH_ANY: TalkMessageDispatchTable<TalkValue> = TalkMessageDispatchTable::empty()
        .with_message(*TALK_BINARY_EQUALS,                  |val: TalkOwned<'_, TalkValue>, args, _| *val == args[0])
        .with_message(*TALK_BINARY_EQUALS_EQUALS,           |val, args, _| *val == args[0])
        .with_message(*TALK_BINARY_TILDE_EQUALS,            |val, args, _| *val != args[0])
        .with_message(*TALK_BINARY_TILDE_TILDE,             |val, args, _| *val != args[0])
        .with_message(*TALK_MSG_CLASS,                      |_, _, _| TalkError::NotImplemented)
        .with_message(*TALK_MSG_COPY,                       |_, _, _| TalkError::NotImplemented)
        .with_message(*TALK_MSG_DOES_NOT_UNDERSTAND,        |_, _, _| TalkError::NotImplemented)
        .with_message(*TALK_MSG_ERROR,                      |_, _, _| TalkError::NotImplemented)
        .with_message(*TALK_MSG_HASH,                       |val, _, _| { let mut hasher = DefaultHasher::new(); (*val).hash(&mut hasher); hasher.finish() as i64 })
        .with_message(*TALK_MSG_IDENTITY_HASH,              |val, _, _| { let mut hasher = DefaultHasher::new(); (*val).hash(&mut hasher); hasher.finish() as i64 })
        .with_message(*TALK_MSG_IS_KIND_OF,                 |_, _, _| TalkError::NotImplemented)
        .with_message(*TALK_MSG_IS_MEMBER_OF,               |_, _, _| TalkError::NotImplemented)
        .with_message(*TALK_MSG_IS_NIL,                     |val, _, _| *val == TalkValue::Nil)
        .with_message(*TALK_MSG_NOT_NIL,                    |val, _, _| *val != TalkValue::Nil)
        .with_message(*TALK_MSG_PERFORM,                    |val, args, context| perform(val, args, context))
        .with_message(*TALK_MSG_PERFORM_WITH,               |val, args, context| perform(val, args, context))
        .with_message(*TALK_MSG_PERFORM_WITH_WITH,          |val, args, context| perform(val, args, context))
        .with_message(*TALK_MSG_PERFORM_WITH_WITH_WITH,     |val, args, context| perform(val, args, context))
        .with_message(*TALK_MSG_PERFORM_WITH4,              |val, args, context| perform(val, args, context))
        .with_message(*TALK_MSG_PERFORM_WITH5,              |val, args, context| perform(val, args, context))
        .with_message(*TALK_MSG_PERFORM_WITH6,              |val, args, context| perform(val, args, context))
        .with_message(*TALK_MSG_PERFORM_WITH7,              |val, args, context| perform(val, args, context))
        .with_message(*TALK_MSG_PERFORM_WITH8,              |val, args, context| perform(val, args, context))
        .with_message(*TALK_MSG_PERFORM_WITH_ARGUMENTS,     |val, args, context| perform_with_arguments(val, args, context))
        .with_message(*TALK_MSG_PRINT_ON,                   |_, _, _| TalkError::NotImplemented)
        .with_message(*TALK_MSG_PRINT_STRING,               |_, _, _| TalkError::NotImplemented)
        .with_message(*TALK_MSG_RESPONDS_TO,                |val, args, context| responds_to(val, args, context))
        .with_message(*TALK_MSG_YOURSELF,                   |mut val, _, _| val.take());
}

lazy_static! {
    ///
    /// The default message dispatcher for boolean values
    ///
    pub static ref TALK_DISPATCH_BOOLEAN: TalkMessageDispatchTable<bool> = TalkMessageDispatchTable::empty()
        .with_mapped_messages_from(&*TALK_DISPATCH_ANY, |bool_value| TalkValue::Bool(bool_value))
        .with_message(*TALK_BINARY_AND,             |val: TalkOwned<'_, bool>, args, _| Ok::<_, TalkError>(*val & args[0].try_as_bool()?))
        .with_message(*TALK_BINARY_OR,              |val, args, _| Ok::<_, TalkError>(*val | args[0].try_as_bool()?))
        .with_message(*TALK_MSG_AND,                |val, mut args, context| if !*val { TalkContinuation::from(false) } else { args[0].take().send_message_in_context(TalkMessage::Unary(*TALK_MSG_VALUE), context) })
        .with_message(*TALK_MSG_OR,                 |val, mut args, context| if *val { TalkContinuation::from(true) } else { args[0].take().send_message_in_context(TalkMessage::Unary(*TALK_MSG_VALUE), context) })
        .with_message(*TALK_MSG_XOR,                |val, args, _| Ok::<_, TalkError>(*val ^ args[0].try_as_bool()?))
        .with_message(*TALK_MSG_EQV,                |val, args, _| Ok(*val) == args[0].try_as_bool())
        .with_message(*TALK_MSG_IF_FALSE,           |val, mut args, context| if !*val { args[0].take().send_message_in_context(TalkMessage::Unary(*TALK_MSG_VALUE), context) } else { TalkValue::Nil.into() })
        .with_message(*TALK_MSG_IF_TRUE,            |val, mut args, context| if *val { args[0].take().send_message_in_context(TalkMessage::Unary(*TALK_MSG_VALUE), context) } else { TalkValue::Nil.into() })
        .with_message(*TALK_MSG_IF_FALSE_IF_TRUE,   |val, mut args, context| if !*val { args[0].take().send_message_in_context(TalkMessage::Unary(*TALK_MSG_VALUE), context) } else { args[1].take().send_message_in_context(TalkMessage::Unary(*TALK_MSG_VALUE), context) })
        .with_message(*TALK_MSG_IF_TRUE_IF_FALSE,   |val, mut args, context| if *val { args[0].take().send_message_in_context(TalkMessage::Unary(*TALK_MSG_VALUE), context) } else { args[1].take().send_message_in_context(TalkMessage::Unary(*TALK_MSG_VALUE), context) })
        .with_message(*TALK_MSG_NOT,                |val, _, _| !*val)
        ;
}

lazy_static! {
    ///
    /// The default message dispatcher for number values
    ///
    pub static ref TALK_DISPATCH_NUMBER: TalkMessageDispatchTable<TalkNumber> = TalkMessageDispatchTable::empty()
        .with_mapped_messages_from(&*TALK_DISPATCH_ANY, |number_value| TalkValue::from(number_value))
        .with_message(*TALK_BINARY_ADD,             |val: TalkOwned<'_, TalkNumber>, args, _| Ok::<_, TalkError>(*val + args[0].try_as_number()?))
        .with_message(*TALK_BINARY_SUB,             |val, args, _| Ok::<_, TalkError>(*val - args[0].try_as_number()?))
        .with_message(*TALK_BINARY_MUL,             |val, args, _| Ok::<_, TalkError>(*val * args[0].try_as_number()?))
        .with_message(*TALK_BINARY_DIV,             |val, args, _| Ok::<_, TalkError>(*val / args[0].try_as_number()?))
        .with_message(*TALK_BINARY_DIV_TRUNCATE,    |val, args, _| Ok::<_, TalkError>((*val / args[0].try_as_number()?).truncate()))
        .with_message(*TALK_BINARY_LT,              |val, args, _| Ok::<_, TalkError>(*val < args[0].try_as_number()?))
        .with_message(*TALK_BINARY_GT,              |val, args, _| Ok::<_, TalkError>(*val > args[0].try_as_number()?))
        .with_message(*TALK_BINARY_EQUALS,          |val, args, _| Ok::<_, TalkError>(*val == args[0].try_as_number()?))
        .with_message(*TALK_BINARY_REMAINDER,       |val, args, _| Ok::<_, TalkError>(*val % args[0].try_as_number()?))
        .with_message(*TALK_MSG_ABS,                |val, _, _| match *val { TalkNumber::Int(x) => TalkNumber::Int(x.abs()), TalkNumber::Float(x) => TalkNumber::Float(x.abs()) })
        .with_message(*TALK_MSG_AS_FLOAT,           |val, _, _| TalkValue::Float(val.as_float()))
        .with_message(*TALK_MSG_AS_FLOAT_D,         |val, _, _| TalkValue::Float(val.as_float()))
        .with_message(*TALK_MSG_AS_FLOAT_E,         |val, _, _| TalkValue::Float(val.as_float()))
        .with_message(*TALK_MSG_AS_FLOAT_Q,         |val, _, _| TalkValue::Float(val.as_float()))
        .with_message(*TALK_MSG_AS_FRACTION,        |_, _, _| TalkError::NotImplemented)
        .with_message(*TALK_MSG_AS_INTEGER,         |val, _, _| val.truncate())
        .with_message(*TALK_MSG_AS_SCALED_DECIMAL,  |_, _, _| TalkError::NotImplemented)
        .with_message(*TALK_MSG_CEILING,            |val, _, _| match *val { TalkNumber::Int(x) => TalkNumber::Int(x), TalkNumber::Float(x) => TalkNumber::Float(x.ceil()) })
        .with_message(*TALK_MSG_FLOOR,              |val, _, _| match *val { TalkNumber::Int(x) => TalkNumber::Int(x), TalkNumber::Float(x) => TalkNumber::Float(x.floor()) })
        .with_message(*TALK_MSG_FRACTION_PART,      |_, _, _| TalkError::NotImplemented)
        .with_message(*TALK_MSG_INTEGER_PART,       |_, _, _| TalkError::NotImplemented)
        .with_message(*TALK_MSG_NEGATED,            |val, _, _| -*val)
        .with_message(*TALK_MSG_NEGATIVE,           |val, _, _| match *val { TalkNumber::Int(x) => x < 0, TalkNumber::Float(x) => x < 0.0 })
        .with_message(*TALK_MSG_POSITIVE,           |val, _, _| match *val { TalkNumber::Int(x) => x >= 0, TalkNumber::Float(x) => x >= 0.0 })
        .with_message(*TALK_MSG_QUO,                |_, _, _| TalkError::NotImplemented)
        .with_message(*TALK_MSG_RAISED_TO,          |_, _, _| TalkError::NotImplemented)
        .with_message(*TALK_MSG_RAISED_TO_INTEGER,  |_, _, _| TalkError::NotImplemented)
        .with_message(*TALK_MSG_RECIPROCAL,         |val, _, _| TalkNumber::Float(1.0) / *val)
        .with_message(*TALK_MSG_REM,                |_, _, _| TalkError::NotImplemented)
        .with_message(*TALK_MSG_ROUNDED,            |val, _, _| match *val { TalkNumber::Int(x) => TalkNumber::Int(x), TalkNumber::Float(x) => TalkNumber::Float(x.round()) })
        .with_message(*TALK_MSG_ROUND_TO,           |_, _, _| TalkError::NotImplemented)
        .with_message(*TALK_MSG_SIGN,               |val, _, _| match *val { TalkNumber::Int(x) => TalkNumber::Int(x.signum()), TalkNumber::Float(x) => TalkNumber::Int(x.signum() as _) })
        .with_message(*TALK_MSG_SQRT,               |val, _, _| match *val { TalkNumber::Int(x) => TalkNumber::Int((x as f64).sqrt().floor() as i64), TalkNumber::Float(x) => TalkNumber::Float(x.sqrt()) })
        .with_message(*TALK_MSG_SQUARED,            |val, _, _| match *val { TalkNumber::Int(x) => TalkNumber::Int(x * x), TalkNumber::Float(x) => TalkNumber::Float(x * x) })
        .with_message(*TALK_MSG_STRICTLY_POSITIVE,  |val, _, _| match *val { TalkNumber::Int(x) => x > 0, TalkNumber::Float(x) => x > 0.0 })
        .with_message(*TALK_MSG_TO,                 |_, _, _| TalkError::NotImplemented)
        .with_message(*TALK_MSG_TO_BY,              |_, _, _| TalkError::NotImplemented)
        .with_message(*TALK_MSG_TO_BY_DO,           |_, _, _| TalkError::NotImplemented)
        .with_message(*TALK_MSG_TO_DO,              |_, _, _| TalkError::NotImplemented)
        .with_message(*TALK_MSG_TRUNCATED,          |val, _, _| val.truncate())
        .with_message(*TALK_MSG_TRUNCATE_TO,        |_, _, _| TalkError::NotImplemented)
        ;
}

///
/// Message dispatch tables for the raw values types
///
pub struct TalkValueDispatchTables {
    pub (super) any_dispatch:       TalkMessageDispatchTable<TalkValue>,
    pub (super) bool_dispatch:      TalkMessageDispatchTable<bool>,
    pub (super) int_dispatch:       TalkMessageDispatchTable<TalkNumber>,
    pub (super) float_dispatch:     TalkMessageDispatchTable<TalkNumber>,
    pub (super) string_dispatch:    TalkMessageDispatchTable<Arc<String>>,
    pub (super) character_dispatch: TalkMessageDispatchTable<char>,
    pub (super) symbol_dispatch:    TalkMessageDispatchTable<TalkSymbol>,
    pub (super) selector_dispatch:  TalkMessageDispatchTable<TalkMessageSignatureId>,
    pub (super) array_dispatch:     TalkMessageDispatchTable<Vec<TalkValue>>,
    pub (super) message_dispatch:   TalkMessageDispatchTable<Box<TalkMessage>>,
    pub (super) error_dispatch:     TalkMessageDispatchTable<TalkError>,
}

impl Default for TalkValueDispatchTables {
    fn default() -> TalkValueDispatchTables {
        TalkValueDispatchTables {
            any_dispatch:       TALK_DISPATCH_ANY.clone(),
            bool_dispatch:      TALK_DISPATCH_BOOLEAN.clone(),
            int_dispatch:       TALK_DISPATCH_NUMBER.clone(),
            float_dispatch:     TALK_DISPATCH_NUMBER.clone(),
            string_dispatch:    TalkMessageDispatchTable::empty(),
            character_dispatch: TalkMessageDispatchTable::empty(),
            symbol_dispatch:    TalkMessageDispatchTable::empty(),
            selector_dispatch:  TalkMessageDispatchTable::empty(),
            array_dispatch:     TalkMessageDispatchTable::empty(),
            message_dispatch:   TalkMessageDispatchTable::empty(),
            error_dispatch:     TalkMessageDispatchTable::empty(),
        }
    }
}

impl TalkValue {
    ///
    /// Performs the default behaviour for a message when sent to a TalkValue
    ///
    #[inline]
    pub fn default_send_message(self, message: TalkMessage, context: &TalkContext) -> TalkContinuation {
        match self {
            TalkValue::Nil                      => TalkError::IsNil.into(),
            TalkValue::Reference(reference)     => reference.send_message_later(message),
            TalkValue::Bool(val)                => TALK_DISPATCH_BOOLEAN.send_message(val, message, context),
            TalkValue::Int(val)                 => TALK_DISPATCH_NUMBER.send_message(TalkNumber::Int(val), message, context),
            TalkValue::Float(val)               => TALK_DISPATCH_NUMBER.send_message(TalkNumber::Float(val), message, context),
            TalkValue::String(_val)             => TalkError::MessageNotSupported(message.signature_id()).into(),
            TalkValue::Character(_val)          => TalkError::MessageNotSupported(message.signature_id()).into(),
            TalkValue::Symbol(_val)             => TalkError::MessageNotSupported(message.signature_id()).into(),
            TalkValue::Selector(_val)           => TalkError::MessageNotSupported(message.signature_id()).into(),
            TalkValue::Array(_val)              => TalkError::MessageNotSupported(message.signature_id()).into(),
            TalkValue::Message(_val)            => TalkError::MessageNotSupported(message.signature_id()).into(),
            TalkValue::Error(_err)              => TalkError::MessageNotSupported(message.signature_id()).into(),
        }
    }
}
