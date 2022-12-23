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
use once_cell::sync::{Lazy};

use std::hash::{Hash, Hasher};
use std::collections::hash_map::{DefaultHasher};
use std::sync::*;

// Class protocol message singatures

pub static TALK_MSG_NEW: Lazy<TalkMessageSignatureId>                         = Lazy::new(|| "new".into());
pub static TALK_MSG_SUPERCLASS: Lazy<TalkMessageSignatureId>                  = Lazy::new(|| "superclass".into());

// Object protocol message signatures

/// Returns true if the two objects are equivalent
pub static TALK_BINARY_EQUALS: Lazy<TalkMessageSignatureId>                   = Lazy::new(|| ("=").into());

/// Returns true if two objects are the same object
pub static TALK_BINARY_EQUALS_EQUALS: Lazy<TalkMessageSignatureId>            = Lazy::new(|| ("==").into());

/// Returns true if the two objects are not equivalent
pub static TALK_BINARY_TILDE_EQUALS: Lazy<TalkMessageSignatureId>             = Lazy::new(|| ("~=").into());

/// Returns true of two objects are not the same object
pub static TALK_BINARY_TILDE_TILDE: Lazy<TalkMessageSignatureId>              = Lazy::new(|| ("~~").into());

/// Returns the class object of the receiver
pub static TALK_MSG_CLASS: Lazy<TalkMessageSignatureId>                       = Lazy::new(|| "class".into());

/// Creates a copy of the receiver
pub static TALK_MSG_COPY: Lazy<TalkMessageSignatureId>                        = Lazy::new(|| "copy".into());

/// A message was sent to the receiver that has no behaviour defined for it
pub static TALK_MSG_DOES_NOT_UNDERSTAND: Lazy<TalkMessageSignatureId>         = Lazy::new(|| ("doesNotUnderstand:").into());

/// Reports that an error occurred
pub static TALK_MSG_ERROR: Lazy<TalkMessageSignatureId>                       = Lazy::new(|| ("error:").into());

/// Returns a hash code for this object
pub static TALK_MSG_HASH: Lazy<TalkMessageSignatureId>                        = Lazy::new(|| "hash".into());

/// Returns a hash code for the identity of this object
pub static TALK_MSG_IDENTITY_HASH: Lazy<TalkMessageSignatureId>               = Lazy::new(|| "identityHash".into());

/// Returns true if the object is an instance of a subclass of the specified class, or the class itself
pub static TALK_MSG_IS_KIND_OF: Lazy<TalkMessageSignatureId>                  = Lazy::new(|| ("isKindOf:").into());

/// Returns true if the object is an instance of the specified class
pub static TALK_MSG_IS_MEMBER_OF: Lazy<TalkMessageSignatureId>                = Lazy::new(|| ("isMemberOf:").into());

/// Returns true if this is the nil object
pub static TALK_MSG_IS_NIL: Lazy<TalkMessageSignatureId>                      = Lazy::new(|| "isNil".into());

/// Returns true if this is not the nil object
pub static TALK_MSG_NOT_NIL: Lazy<TalkMessageSignatureId>                     = Lazy::new(|| "notNil".into());

/// Performs the specified selector on the object
pub static TALK_MSG_PERFORM: Lazy<TalkMessageSignatureId>                     = Lazy::new(|| ("perform:").into());

/// Performs the specified selector on the object, with the specified arguments
pub static TALK_MSG_PERFORM_WITH: Lazy<TalkMessageSignatureId>                = Lazy::new(|| ("perform:", "with:").into());

/// Performs the specified selector on the object, with the specified arguments
pub static TALK_MSG_PERFORM_WITH_WITH: Lazy<TalkMessageSignatureId>           = Lazy::new(|| ("perform:", "with:", "with:").into());

/// Performs the specified selector on the object, with the specified arguments
pub static TALK_MSG_PERFORM_WITH_WITH_WITH: Lazy<TalkMessageSignatureId>      = Lazy::new(|| ("perform:", "with:", "with:", "with:").into());

/// Performs the specified selector on the object, with the specified arguments
pub static TALK_MSG_PERFORM_WITH4: Lazy<TalkMessageSignatureId>               = Lazy::new(|| vec!["perform:", "with:", "with:", "with:", "with:"].into());

/// Performs the specified selector on the object, with the specified arguments
pub static TALK_MSG_PERFORM_WITH5: Lazy<TalkMessageSignatureId>               = Lazy::new(|| vec!["perform:", "with:", "with:", "with:", "with:", "with:"].into());

/// Performs the specified selector on the object, with the specified arguments
pub static TALK_MSG_PERFORM_WITH6: Lazy<TalkMessageSignatureId>               = Lazy::new(|| vec!["perform:", "with:", "with:", "with:", "with:", "with:", "with:"].into());

/// Performs the specified selector on the object, with the specified arguments
pub static TALK_MSG_PERFORM_WITH7: Lazy<TalkMessageSignatureId>               = Lazy::new(|| vec!["perform:", "with:", "with:", "with:", "with:", "with:", "with:", "with:"].into());

/// Performs the specified selector on the object, with the specified arguments
pub static TALK_MSG_PERFORM_WITH8: Lazy<TalkMessageSignatureId>               = Lazy::new(|| vec!["perform:", "with:", "with:", "with:", "with:", "with:", "with:", "with:", "with:"].into());

/// Performs the specified selector on the object, with the specified arguments
pub static TALK_MSG_PERFORM_WITH_ARGUMENTS: Lazy<TalkMessageSignatureId>      = Lazy::new(|| ("perform:", "withArguments:").into());

/// Writes a description of the object to a stream
pub static TALK_MSG_PRINT_ON: Lazy<TalkMessageSignatureId>                    = Lazy::new(|| ("printOn:").into());

/// Returns a string description of the receiver
pub static TALK_MSG_PRINT_STRING: Lazy<TalkMessageSignatureId>                = Lazy::new(|| "printString".into());

/// True if the receiver can respond to a message selector
pub static TALK_MSG_RESPONDS_TO: Lazy<TalkMessageSignatureId>                 = Lazy::new(|| ("respondsTo:").into());

/// Returns the receiver as the result
pub static TALK_MSG_YOURSELF: Lazy<TalkMessageSignatureId>                    = Lazy::new(|| "yourself".into());


// Valuable protocol messages

pub static TALK_MSG_VALUE: Lazy<TalkMessageSignatureId>                       = Lazy::new(|| "value".into());
pub static TALK_MSG_WHILE_FALSE: Lazy<TalkMessageSignatureId>                 = Lazy::new(|| "whileFalse".into());
pub static TALK_MSG_WHILE_FALSE_COLON: Lazy<TalkMessageSignatureId>           = Lazy::new(|| ("whileFalse:").into());
pub static TALK_MSG_WHILE_TRUE: Lazy<TalkMessageSignatureId>                  = Lazy::new(|| "whileTrue".into());
pub static TALK_MSG_WHILE_TRUE_COLON: Lazy<TalkMessageSignatureId>            = Lazy::new(|| ("whileTrue:").into());


// Boolean protocol messages

pub static TALK_BINARY_AND: Lazy<TalkMessageSignatureId>                      = Lazy::new(|| ("&").into());
pub static TALK_BINARY_OR: Lazy<TalkMessageSignatureId>                       = Lazy::new(|| ("|").into());
pub static TALK_MSG_AND: Lazy<TalkMessageSignatureId>                         = Lazy::new(|| ("and:").into());
pub static TALK_MSG_OR: Lazy<TalkMessageSignatureId>                          = Lazy::new(|| ("or:").into());
pub static TALK_MSG_XOR: Lazy<TalkMessageSignatureId>                         = Lazy::new(|| ("xor:").into());
pub static TALK_MSG_EQV: Lazy<TalkMessageSignatureId>                         = Lazy::new(|| ("eqv:").into());
pub static TALK_MSG_IF_FALSE: Lazy<TalkMessageSignatureId>                    = Lazy::new(|| ("ifFalse:").into());
pub static TALK_MSG_IF_FALSE_IF_TRUE: Lazy<TalkMessageSignatureId>            = Lazy::new(|| ("ifFalse:", "ifTrue:").into());
pub static TALK_MSG_IF_TRUE: Lazy<TalkMessageSignatureId>                     = Lazy::new(|| ("ifTrue:").into());
pub static TALK_MSG_IF_TRUE_IF_FALSE: Lazy<TalkMessageSignatureId>            = Lazy::new(|| ("ifTrue:", "ifFalse:").into());
pub static TALK_MSG_NOT: Lazy<TalkMessageSignatureId>                         = Lazy::new(|| "not".into());


// Number protocol messages

pub static TALK_BINARY_ADD: Lazy<TalkMessageSignatureId>                      = Lazy::new(|| ("+").into());
pub static TALK_BINARY_SUB: Lazy<TalkMessageSignatureId>                      = Lazy::new(|| ("-").into());
pub static TALK_BINARY_MUL: Lazy<TalkMessageSignatureId>                      = Lazy::new(|| ("*").into());
pub static TALK_BINARY_DIV: Lazy<TalkMessageSignatureId>                      = Lazy::new(|| ("/").into());
pub static TALK_BINARY_DIV_TRUNCATE: Lazy<TalkMessageSignatureId>             = Lazy::new(|| ("//").into());
pub static TALK_BINARY_LT: Lazy<TalkMessageSignatureId>                       = Lazy::new(|| ("<").into());
pub static TALK_BINARY_GT: Lazy<TalkMessageSignatureId>                       = Lazy::new(|| (">").into());
pub static TALK_BINARY_REMAINDER: Lazy<TalkMessageSignatureId>                = Lazy::new(|| ("\\").into());
pub static TALK_MSG_ABS: Lazy<TalkMessageSignatureId>                         = Lazy::new(|| "abs".into());
pub static TALK_MSG_AS_FLOAT: Lazy<TalkMessageSignatureId>                    = Lazy::new(|| "asFloat".into());
pub static TALK_MSG_AS_FLOAT_D: Lazy<TalkMessageSignatureId>                  = Lazy::new(|| "asFloatD".into());
pub static TALK_MSG_AS_FLOAT_E: Lazy<TalkMessageSignatureId>                  = Lazy::new(|| "asFloatE".into());
pub static TALK_MSG_AS_FLOAT_Q: Lazy<TalkMessageSignatureId>                  = Lazy::new(|| "asFloatQ".into());
pub static TALK_MSG_AS_FRACTION: Lazy<TalkMessageSignatureId>                 = Lazy::new(|| "asFraction".into());
pub static TALK_MSG_AS_INTEGER: Lazy<TalkMessageSignatureId>                  = Lazy::new(|| "asInteger".into());
pub static TALK_MSG_AS_SCALED_DECIMAL: Lazy<TalkMessageSignatureId>           = Lazy::new(|| ("asScaledDecimal:").into());
pub static TALK_MSG_CEILING: Lazy<TalkMessageSignatureId>                     = Lazy::new(|| "ceiling".into());
pub static TALK_MSG_FLOOR: Lazy<TalkMessageSignatureId>                       = Lazy::new(|| "floor".into());
pub static TALK_MSG_FRACTION_PART: Lazy<TalkMessageSignatureId>               = Lazy::new(|| "fractionPart".into());
pub static TALK_MSG_INTEGER_PART: Lazy<TalkMessageSignatureId>                = Lazy::new(|| "integerPart".into());
pub static TALK_MSG_NEGATED: Lazy<TalkMessageSignatureId>                     = Lazy::new(|| "negated".into());
pub static TALK_MSG_NEGATIVE: Lazy<TalkMessageSignatureId>                    = Lazy::new(|| "negative".into());
pub static TALK_MSG_POSITIVE: Lazy<TalkMessageSignatureId>                    = Lazy::new(|| "positive".into());
pub static TALK_MSG_QUO: Lazy<TalkMessageSignatureId>                         = Lazy::new(|| ("quo:").into());
pub static TALK_MSG_RAISED_TO: Lazy<TalkMessageSignatureId>                   = Lazy::new(|| ("raisedTo:").into());
pub static TALK_MSG_RAISED_TO_INTEGER: Lazy<TalkMessageSignatureId>           = Lazy::new(|| ("rasiedToInteger:").into());
pub static TALK_MSG_RECIPROCAL: Lazy<TalkMessageSignatureId>                  = Lazy::new(|| "reciprocal".into());
pub static TALK_MSG_REM: Lazy<TalkMessageSignatureId>                         = Lazy::new(|| ("rem:").into());
pub static TALK_MSG_ROUNDED: Lazy<TalkMessageSignatureId>                     = Lazy::new(|| "rounded".into());
pub static TALK_MSG_ROUND_TO: Lazy<TalkMessageSignatureId>                    = Lazy::new(|| ("roundTo:").into());
pub static TALK_MSG_SIGN: Lazy<TalkMessageSignatureId>                        = Lazy::new(|| "sign".into());
pub static TALK_MSG_SQRT: Lazy<TalkMessageSignatureId>                        = Lazy::new(|| "sqrt".into());
pub static TALK_MSG_SQUARED: Lazy<TalkMessageSignatureId>                     = Lazy::new(|| "squared".into());
pub static TALK_MSG_STRICTLY_POSITIVE: Lazy<TalkMessageSignatureId>           = Lazy::new(|| "strictlyPositive".into());
pub static TALK_MSG_TO: Lazy<TalkMessageSignatureId>                          = Lazy::new(|| ("to:").into());
pub static TALK_MSG_TO_BY: Lazy<TalkMessageSignatureId>                       = Lazy::new(|| ("to:", "by:").into());
pub static TALK_MSG_TO_BY_DO: Lazy<TalkMessageSignatureId>                    = Lazy::new(|| ("to:", "by:", "do:").into());
pub static TALK_MSG_TO_DO: Lazy<TalkMessageSignatureId>                       = Lazy::new(|| ("to:", "do:").into());
pub static TALK_MSG_TRUNCATED: Lazy<TalkMessageSignatureId>                   = Lazy::new(|| "truncated".into());
pub static TALK_MSG_TRUNCATE_TO: Lazy<TalkMessageSignatureId>                 = Lazy::new(|| ("truncateTo:").into());


// FloTalk selector protocol messages

/// `#signature asMessage` - creates a unary message value from a signature
pub static TALK_MSG_AS_MESSAGE: Lazy<TalkMessageSignatureId>                  = Lazy::new(|| "asMessage".into());

/// `#signature: with: 42` - creates a message with an argument from a signature
pub static TALK_MSG_WITH: Lazy<TalkMessageSignatureId>                        = Lazy::new(|| "with:".into());

/// `#signature:two: with: 1 with: 2` - creates a message with some arguments from a signature
pub static TALK_MSG_WITH_WITH: Lazy<TalkMessageSignatureId>                   = Lazy::new(|| ("with:", "with:").into());

/// `#signature:two:three: with: 1 with: 2 with: 3` - creates a message with some arguments from a signature
pub static TALK_MSG_WITH_WITH_WITH: Lazy<TalkMessageSignatureId>              = Lazy::new(|| ("with:", "with:", "with:").into());

/// `#signature:two:three:four: with: 1 with: 2 with: 3 with: 4` - creates a message with some arguments from a signature
pub static TALK_MSG_WITH4: Lazy<TalkMessageSignatureId>                       = Lazy::new(|| ("with:", "with:", "with:", "with:").into());

/// `#signature:two:three:four:five: with: 1 with: 2 with: 3 with: 4 with: 5` - creates a message with some arguments from a signature
pub static TALK_MSG_WITH5: Lazy<TalkMessageSignatureId>                       = Lazy::new(|| vec!["with:", "with:", "with:", "with:", "with:"].into());

/// `#signature:two:three:four:five:six: with: 1 with: 2 with: 3 with: 4 with: 5 with: 6` - creates a message with some arguments from a signature
pub static TALK_MSG_WITH6: Lazy<TalkMessageSignatureId>                       = Lazy::new(|| vec!["with:", "with:", "with:", "with:", "with:", "with:"].into());

/// `#signature:two:three:four:five:six:seven: with: 1 with: 2 with: 3 with: 4 with: 5 with: 6 with: 7` - creates a message with some arguments from a signature
pub static TALK_MSG_WITH7: Lazy<TalkMessageSignatureId>                       = Lazy::new(|| vec!["with:", "with:", "with:", "with:", "with:", "with:", "with:"].into());

/// `#signature:two:three:four:five:six:seven:eight: with: 1 with: 2 with: 3 with: 4 with: 5 with: 6 with: 7 with: 8` - creates a message with some arguments from a signature
pub static TALK_MSG_WITH8: Lazy<TalkMessageSignatureId>                       = Lazy::new(|| vec!["with:", "with:", "with:", "with:", "with:", "with:", "with:", "with:"].into());

/// `#signature:two: withArguments: #(1 2)` - creates a message with an argument from a signature
pub static TALK_MSG_WITHARGUMENTS: Lazy<TalkMessageSignatureId>               = Lazy::new(|| "withArguments:".into());


// FloTalk message protocol messages

/// `msg matchesSignature: #signature` - true if a message object has a particular signature
pub static TALK_MSG_MATCHES_SELECTOR: Lazy<TalkMessageSignatureId>            = Lazy::new(|| "matchesSelector:".into());

/// `msg signatureStartsWith: #signature:with:` - true if a message object has a particular signature
pub static TALK_MSG_SELECTOR_STARTS_WITH: Lazy<TalkMessageSignatureId>        = Lazy::new(|| "selectorStartsWith:".into());

/// `msg messageAfter: #signature:withArg:` - creates a new message by removing the arguments matched by a signature from the start of a message
pub static TALK_MSG_MESSAGE_AFTER: Lazy<TalkMessageSignatureId>               = Lazy::new(|| "messageAfter:".into());

/// `msg messageCombinedWith: anotherMsg` - creates a new message by appending the signature of the 'other' message to the existing message
pub static TALK_MSG_MESSAGE_COMBINED_WITH: Lazy<TalkMessageSignatureId>       = Lazy::new(|| "messageCombinedWith:".into());

/// `msg argumentAt: 1` - retrieves the message argument at the specified position
pub static TALK_MSG_ARGUMENT_AT: Lazy<TalkMessageSignatureId>                 = Lazy::new(|| "argumentAt:".into());

/// `msg arguments` - retrieves the arguments of a message as an array
pub static TALK_MSG_ARGUMENTS: Lazy<TalkMessageSignatureId>                   = Lazy::new(|| "arguments".into());

/// `msg signature` - retrieves the signature of a message
pub static TALK_MSG_SELECTOR: Lazy<TalkMessageSignatureId>                    = Lazy::new(|| "selector".into());

/// `msg ifMatches: #signature: do: [ :arg | something ]` - if the message matches a signature, perform an action using the arguments
pub static TALK_MSG_IFMATCHES_DO: Lazy<TalkMessageSignatureId>                = Lazy::new(|| ("ifMatches:", "do:").into());

/// `msg ifMatches: #signature: do: [ :arg | something ]` - if the message matches a signature, perform an action using the arguments
pub static TALK_MSG_IFMATCHES_DO_IF_DOES_NOT_MATCH: Lazy<TalkMessageSignatureId> = Lazy::new(|| ("ifMatches:", "do:", "ifDoesNotMatch:").into());

/// `msg ifMatches: #signature: do: [ :arg | something ]` - if the message matches a signature, perform an action using the arguments
pub static TALK_MSG_IFDOESNOTMATCH_DO: Lazy<TalkMessageSignatureId>           = Lazy::new(|| ("ifDoesNotMatch:", "do:").into());


///
/// Implements the various 'perform:with:' selectors
///
#[inline]
fn perform(mut val: TalkOwned<TalkValue, &'_ TalkContext>, mut args: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, context: &TalkContext) -> TalkContinuation<'static> {
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
fn perform_with_arguments(mut val: TalkOwned<TalkValue, &'_ TalkContext>, args: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, context: &TalkContext) -> TalkContinuation<'static> {
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
fn responds_to(val: TalkOwned<TalkValue, &'_ TalkContext>, args: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, context: &TalkContext) -> TalkContinuation<'static> {
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

///
/// The default message dispatcher for 'any' type
///
pub static TALK_DISPATCH_ANY: Lazy<TalkMessageDispatchTable<TalkValue>> = Lazy::new(|| TalkMessageDispatchTable::empty()
    .with_message(*TALK_BINARY_EQUALS,                  |val: TalkOwned<TalkValue, &'_ TalkContext>, args, _| *val == args[0])
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
    .with_message(*TALK_MSG_YOURSELF,                   |mut val, _, _| val.take())
    );

///
/// The default message dispatcher for boolean values
///
pub static TALK_DISPATCH_BOOLEAN: Lazy<TalkMessageDispatchTable<bool>> = Lazy::new(|| TalkMessageDispatchTable::empty()
    .with_mapped_messages_from(&*TALK_DISPATCH_ANY, |bool_value| TalkValue::Bool(bool_value))
    .with_message(*TALK_BINARY_AND,             |val: TalkOwned<bool, &'_ TalkContext>, args, _| Ok::<_, TalkError>(*val & args[0].try_as_bool()?))
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
    );

///
/// The default message dispatcher for number values
///
pub static TALK_DISPATCH_NUMBER: Lazy<TalkMessageDispatchTable<TalkNumber>> = Lazy::new(|| TalkMessageDispatchTable::empty()
    .with_mapped_messages_from(&*TALK_DISPATCH_ANY, |number_value| TalkValue::from(number_value))
    .with_message(*TALK_BINARY_ADD,             |val: TalkOwned<TalkNumber, &'_ TalkContext>, args, _| Ok::<_, TalkError>(*val + args[0].try_as_number()?))
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
    );

///
/// Converts a message signature ID to a message
///
fn selector_as_message(selector: TalkMessageSignatureId) -> TalkContinuation<'static> {
    selector_as_message_with_args(selector, smallvec![])
}

///
/// Converts a message signature ID to a message
///
fn selector_as_message_from_array(selector: TalkMessageSignatureId, args_array: TalkValue, context: &TalkContext) -> TalkContinuation<'static> {
    match args_array {
        TalkValue::Array(values)    => selector_as_message_with_args(selector, values.into_iter().collect()),
        _                           => {
            args_array.release_in_context(context);
            TalkError::NotAnArray.into()
        }
    }
}

///
/// Converts a message signature ID to a message
///
fn selector_as_message_with_args(selector: TalkMessageSignatureId, arguments: SmallVec<[TalkValue; 4]>) -> TalkContinuation<'static> {
    if selector.len() != arguments.len() {
        TalkError::WrongNumberOfArguments.into()
    } else if arguments.len() == 0 {
        TalkValue::Message(Box::new(TalkMessage::Unary(selector))).into()
    } else {
        TalkValue::Message(Box::new(TalkMessage::WithArguments(selector, arguments))).into()
    }
}

///
/// The default message dispatcher for selectors (message signatures)
///
pub static TALK_DISPATCH_SELECTOR: Lazy<TalkMessageDispatchTable<TalkMessageSignatureId>> = Lazy::new(|| TalkMessageDispatchTable::empty()
    .with_message(*TALK_MSG_AS_MESSAGE,         |val: TalkOwned<TalkMessageSignatureId, &'_ TalkContext>, _, _| selector_as_message(val.leak()))
    .with_message(*TALK_MSG_WITH,               |val, args, _| selector_as_message_with_args(val.leak(), args.leak()))
    .with_message(*TALK_MSG_WITH_WITH,          |val, args, _| selector_as_message_with_args(val.leak(), args.leak()))
    .with_message(*TALK_MSG_WITH_WITH_WITH,     |val, args, _| selector_as_message_with_args(val.leak(), args.leak()))
    .with_message(*TALK_MSG_WITH4,              |val, args, _| selector_as_message_with_args(val.leak(), args.leak()))
    .with_message(*TALK_MSG_WITH5,              |val, args, _| selector_as_message_with_args(val.leak(), args.leak()))
    .with_message(*TALK_MSG_WITH6,              |val, args, _| selector_as_message_with_args(val.leak(), args.leak()))
    .with_message(*TALK_MSG_WITH7,              |val, args, _| selector_as_message_with_args(val.leak(), args.leak()))
    .with_message(*TALK_MSG_WITH8,              |val, args, _| selector_as_message_with_args(val.leak(), args.leak()))
    .with_message(*TALK_MSG_WITHARGUMENTS,      |val, mut args, context| selector_as_message_from_array(val.leak(), args[0].take(), context))
    );

///
/// Implements the `matchesSelector:` message
///
fn message_matches_selector(msg: &TalkMessage, selector: &TalkValue) -> bool {
    if &TalkValue::Selector(msg.signature_id()) == selector {
        true
    } else {
        false
    }
}

///
/// Implements the `selectorStartsWith:` message
///
fn message_selector_starts_with(msg: &TalkMessage, starts_with: &TalkValue) -> bool {
    let message_signature = msg.signature_id().to_signature();

    if let TalkValue::Selector(starts_with) = starts_with {
        use TalkMessageSignature::*;

        // Get the initial signature
        let starts_with = starts_with.to_signature();

        match (message_signature, starts_with) {
            (Unary(symbol_1), Unary(symbol_2))      => symbol_1 == symbol_2,
            (Arguments(args_1), Arguments(args_2))  => args_1.iter().take(args_2.len()).eq(args_2.iter()),
            _                                       => false,
        }
    } else {
        // Not a selector
        false
    }
}

pub static TALK_DISPATCH_MESSAGE: Lazy<TalkMessageDispatchTable<Box<TalkMessage>>> = Lazy::new(|| TalkMessageDispatchTable::empty()
    .with_message(*TALK_MSG_SELECTOR,                       |val: TalkOwned<Box<TalkMessage>, &'_ TalkContext>, _, _| TalkValue::Selector(val.signature_id()))
    .with_message(*TALK_MSG_MATCHES_SELECTOR,               |val, args, _| message_matches_selector(&**val, &args[0]))
    .with_message(*TALK_MSG_SELECTOR_STARTS_WITH,           |val, args, _| message_selector_starts_with(&**val, &args[0]))
    .with_message(*TALK_MSG_MESSAGE_AFTER,                  |_, _, _| TalkError::NotImplemented)
    .with_message(*TALK_MSG_MESSAGE_COMBINED_WITH,          |_, _, _| TalkError::NotImplemented)
    .with_message(*TALK_MSG_ARGUMENT_AT,                    |_, _, _| TalkError::NotImplemented)
    .with_message(*TALK_MSG_ARGUMENTS,                      |_, _, _| TalkError::NotImplemented)
    .with_message(*TALK_MSG_IFMATCHES_DO,                   |_, _, _| TalkError::NotImplemented)
    .with_message(*TALK_MSG_IFMATCHES_DO_IF_DOES_NOT_MATCH, |_, _, _| TalkError::NotImplemented)
    .with_message(*TALK_MSG_IFDOESNOTMATCH_DO,              |_, _, _| TalkError::NotImplemented)
    );

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
            selector_dispatch:  TALK_DISPATCH_SELECTOR.clone(),
            array_dispatch:     TalkMessageDispatchTable::empty(),
            message_dispatch:   TALK_DISPATCH_MESSAGE.clone(),
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
            TalkValue::Selector(selector)       => TALK_DISPATCH_SELECTOR.send_message(selector, message, context),
            TalkValue::Array(_val)              => TalkError::MessageNotSupported(message.signature_id()).into(),
            TalkValue::Message(target)          => TALK_DISPATCH_MESSAGE.send_message(target, message, context),
            TalkValue::Error(_err)              => TalkError::MessageNotSupported(message.signature_id()).into(),
        }
    }
}
