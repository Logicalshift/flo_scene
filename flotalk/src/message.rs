use super::context::*;
use super::error::*;
use super::expression::*;
use super::releasable::*;
use super::sparse_array::*;
use super::symbol::*;
use super::value::*;

use smallvec::*;
use once_cell::sync::{Lazy};

use std::fmt;
use std::sync::*;
use std::collections::{HashMap};

/// The ID to assign to the next message signature
static NEXT_SIGNATURE_ID: Lazy<Mutex<usize>>                                                  = Lazy::new(|| Mutex::new(0));

/// Maps between signatures and their IDs
static ID_FOR_SIGNATURE: Lazy<Mutex<HashMap<TalkMessageSignature, TalkMessageSignatureId>>>   = Lazy::new(|| Mutex::new(HashMap::new()));

/// Maps between IDs and signatures
static SIGNATURE_FOR_ID: Lazy<Mutex<HashMap<TalkMessageSignatureId, TalkMessageSignature>>>   = Lazy::new(|| Mutex::new(HashMap::new()));

///
/// Represents a FloTalk message
///
/// Messages can be either unary (a call with no arguments):
///
/// ```
/// # use flo_talk::*;
/// let message = TalkMessage::Unary("message".into());
/// ```
///
/// Or they can supply some arguments, where the number of arguments must match the message signature ID:
///
/// ```
/// # use flo_talk::*;
/// # use smallvec::*;
/// let message = TalkMessage::WithArguments(("arg1:", "arg2:").into(), smallvec![42.into(), "String".into()]);
/// ```
///
#[derive(Clone, PartialEq, Hash)]
pub enum TalkMessage {
    /// A message with no arguments
    Unary(TalkMessageSignatureId),

    /// A message with named arguments
    WithArguments(TalkMessageSignatureId, SmallVec<[TalkValue; 4]>),
}

///
/// The data represented by a message send action
///
/// This is commonly used to create a continuation that will send the specified message:
///
/// ```
/// # use flo_talk::*;
/// # let some_value = TalkValue::Nil;
/// let continuation = TalkContinuation::from(TalkSendMessage(some_value, TalkMessage::Unary("value".into())));
/// ```
///
pub struct TalkSendMessage(pub TalkValue, pub TalkMessage);

///
/// Trait implemented by types that can be converted to and from `TalkMessage`s
///
pub trait TalkMessageType : Sized {
    /// Converts a message to an object of this type
    fn from_message<'a>(message: TalkOwned<TalkMessage, &'a TalkContext>, context: &'a TalkContext) -> Result<Self, TalkError>;

    /// Converts an object of this type to a message
    fn to_message<'a>(&self, context: &'a TalkContext) -> TalkOwned<TalkMessage, &'a TalkContext>;
}

///
/// A message signature describes a message
///
/// Signatures are usually used to generate message IDs, though they can be used for introspection of arbitrary messages.
///
/// ```
/// # use flo_talk::*;
/// let signature   = TalkMessageSignature::unary("message");
/// let id          = signature.id();
/// let num_args    = signature.len();      // == 0
/// # assert!(num_args == 0);
/// ```
///
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TalkMessageSignature {
    Unary(TalkSymbol),
    Arguments(SmallVec<[TalkSymbol; 4]>),
}

///
/// A unique ID for a message signature
///
/// Every message in FloTalk is mapped to a unique ID, which can be used as a means to quickly match a message against its action. An important
/// property of this is that every message ID has a fixed number of arguments, so it's generally not necessary to inspect signatures at runtime.
///
/// IDs are generated from signatures, but there are some convenience methods for converting other types into IDs.
///
/// ```
/// # use flo_talk::*;
/// let message_id: TalkMessageSignatureId  = ("arg1:", "arg2:").into();
/// let signature: TalkMessageSignature     = message_id.to_signature();
/// let num_args                            = signature.len();          // == 2
/// # debug_assert!(num_args == 2);
/// ```
///
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TalkMessageSignatureId(usize);

impl TalkMessage {
    ///
    /// The signature ID of this message
    ///
    #[inline]
    pub fn signature_id(&self) -> TalkMessageSignatureId {
        match self {
            TalkMessage::Unary(id)              => *id,
            TalkMessage::WithArguments(id, _)   => *id,
        }
    }

    ///
    /// Consumes this message and returns the arguments
    ///
    #[inline]
    pub fn to_arguments(self) -> SmallVec<[TalkValue; 4]> {
        match self {
            TalkMessage::Unary(_)               => smallvec![],
            TalkMessage::WithArguments(_, args) => args,
        }
    }

    ///
    /// Creates a message from a signature and its arguments (assumes the arguments matches the signature)
    ///
    #[inline]
    pub fn from_signature(sig: impl Into<TalkMessageSignatureId>, arguments: SmallVec<[TalkValue; 4]>) -> TalkMessage {
        if arguments.len() == 0 {
            TalkMessage::Unary(sig.into())
        } else {
            TalkMessage::WithArguments(sig.into(), arguments)
        }
    }

    ///
    /// Creates a unary message
    ///
    pub fn unary(symbol: impl Into<TalkSymbol>) -> TalkMessage {
        TalkMessage::Unary(TalkMessageSignature::Unary(symbol.into()).id())
    }

    ///
    /// Creates a message with arguments
    ///
    pub fn with_arguments(arguments: impl IntoIterator<Item=(impl Into<TalkSymbol>, impl Into<TalkValue>)>) -> TalkMessage {
        let mut signature_symbols   = smallvec![];
        let mut argument_values     = smallvec![];

        for (symbol, value) in arguments {
            signature_symbols.push(symbol.into());
            argument_values.push(value.into());
        }

        TalkMessage::WithArguments(TalkMessageSignature::Arguments(signature_symbols).id(), argument_values)
    }

    ///
    /// Returns the number of arguments in this message
    ///
    pub fn len(&self) -> usize {
        match self {
            TalkMessage::Unary(_)               => 0,
            TalkMessage::WithArguments(_, args) => args.len(),
        }
    }

    ///
    /// Retains all the references contained in this message
    ///
    pub fn retain(&self, context: &TalkContext) {
        match self {
            TalkMessage::Unary(_)               => { }
            TalkMessage::WithArguments(_, args) => { args.iter().for_each(|arg| arg.retain(context)); }
        }
    }

    ///
    /// Releases all the references contained in this message
    ///
    pub fn release(&self, context: &TalkContext) {
        match self {
            TalkMessage::Unary(_)               => { }
            TalkMessage::WithArguments(_, args) => { context.release_values(args); }
        }
    }

    ///
    /// Converts a message to its signature
    ///
    #[inline]
    pub fn signature(&self) -> TalkMessageSignature {
        match self {
            TalkMessage::Unary(id)                  => id.to_signature(),
            TalkMessage::WithArguments(id, _args)   => id.to_signature()
        }
    }

    ///
    /// Appends an extra parameter to this message
    ///
    /// The extra parameter must always have a value. For a message like `foo:`, adding a parameter like `bar:` will generate
    /// the message `foo:bar:`. However, for a unary message like `foo`, the generated signature will be `fooBar:`.
    ///
    pub fn with_extra_parameter(self, symbol: impl Into<TalkSymbol>, value: impl Into<TalkValue>) -> TalkMessage {
        // Split up this object
        let (message_id, args) = match self {
            TalkMessage::Unary(message_id)                  => (message_id, smallvec![]),
            TalkMessage::WithArguments(message_id, args)    => (message_id, args),
        };

        // Add the new argument
        let mut args = args;
        args.push(value.into());

        // Generate a new message ID
        let message_id = message_id.with_extra_parameter(symbol);

        // New message always has an argument
        TalkMessage::WithArguments(message_id, args)
    }
}

impl TalkReleasable for TalkMessage {
    #[inline]
    fn release_in_context(self, context: &TalkContext) {
        match self {
            TalkMessage::Unary(_)               => { }
            TalkMessage::WithArguments(_, args) => args.release_in_context(context)
        }
    }
}

impl TalkCloneable for TalkMessage {
    ///
    /// Creates a copy of this value in the specified context
    ///
    /// This will copy this value and increase its reference count
    ///
    fn clone_in_context(&self, context: &TalkContext) -> Self {
        match self {
            TalkMessage::Unary(sig)                 => TalkMessage::Unary(*sig),
            TalkMessage::WithArguments(sig, args)   => TalkMessage::WithArguments(*sig, args.iter().map(|arg| arg.clone_in_context(context)).collect())
        }
    }
}

impl TalkMessageSignature {
    ///
    /// Returns the ID for this signature
    ///
    pub fn id(&self) -> TalkMessageSignatureId {
        let id_for_signature = ID_FOR_SIGNATURE.lock().unwrap();

        if let Some(id) = id_for_signature.get(self) {
            // ID already defined
            *id
        } else {
            let mut id_for_signature = id_for_signature;

            // Create a new ID
            let new_id = {
                let mut next_signature_id   = NEXT_SIGNATURE_ID.lock().unwrap();
                let new_id                  = *next_signature_id;
                *next_signature_id += 1;

                new_id
            };
            let new_id = TalkMessageSignatureId(new_id);

            // Store the ID for this signature
            id_for_signature.insert(self.clone(), new_id);
            SIGNATURE_FOR_ID.lock().unwrap().insert(new_id, self.clone());

            new_id
        }
    }

    ///
    /// Creates a unary message signature
    ///
    pub fn unary(symbol: impl Into<TalkSymbol>) -> TalkMessageSignature {
        TalkMessageSignature::Unary(symbol.into())
    }

    ///
    /// Creates a message signature with arguments
    ///
    pub fn with_arguments(symbols: impl IntoIterator<Item=impl Into<TalkSymbol>>) -> TalkMessageSignature {
        TalkMessageSignature::Arguments(symbols.into_iter().map(|sym| sym.into()).collect())
    }

    ///
    /// Returns true if an argument list represents a unary list
    ///
    pub fn arguments_are_unary<'a>(args: impl IntoIterator<Item=&'a TalkArgument>) -> bool {
        let mut arg_iter = args.into_iter();

        if let Some(first) = arg_iter.next() {
            if first.value.is_none() {
                // Unary if there's a single argument with no value
                let next = arg_iter.next();

                debug_assert!(next.is_none());

                next.is_none()
            } else {
                // First argument has a value
                false
            }
        } else {
            // Empty message
            false
        }
    }

    ///
    /// Creates a signature from a list of arguments
    ///
    pub fn from_expression_arguments<'a>(args: impl IntoIterator<Item=&'a TalkArgument>) -> TalkMessageSignature {
        let arguments = args.into_iter().collect::<SmallVec<[_; 4]>>();

        if arguments.len() == 1 && arguments[0].value.is_none() {
            Self::unary(&arguments[0].name)
        } else {
            Self::with_arguments(arguments.into_iter().map(|arg| &arg.name))
        }
    }

    ///
    /// True if this is a signature for a unary message (one with no arguments)
    ///
    pub fn is_unary(&self) -> bool {
        match self {
            TalkMessageSignature::Unary(_)  => true,
            _                               => false,
        }
    }

    ///
    /// Returns the symbol at the start of this signature
    ///
    pub fn first_symbol(&self) -> TalkSymbol {
        match self {
            TalkMessageSignature::Unary(symbol)     => *symbol,
            TalkMessageSignature::Arguments(args)   => args[0],
        }
    }

    ///
    /// Number of arguments in this message signature
    ///
    pub fn len(&self) -> usize {
        match self {
            TalkMessageSignature::Unary(_)          => 0,
            TalkMessageSignature::Arguments(args)   => args.len(),
        }
    }

    ///
    /// Appends an extra parameter to this signature
    ///
    /// The extra parameter must always have a value. For a message like `foo:`, adding a parameter like `bar:` will generate
    /// the message `foo:bar:`. However, for a unary message like `foo`, the generated signature will be `fooBar:`.
    ///
    pub fn with_extra_parameter(&self, new_symbol: impl Into<TalkSymbol>) -> TalkMessageSignature {
        // This is implemented by the message signature ID type (which does caching)
        self.id().with_extra_parameter(new_symbol).to_signature()
    }
}

impl From<&TalkMessageSignature> for TalkMessageSignatureId {
    fn from(sig: &TalkMessageSignature) -> TalkMessageSignatureId {
        sig.id()
    }
}

impl From<TalkMessageSignature> for TalkMessageSignatureId {
    fn from(sig: TalkMessageSignature) -> TalkMessageSignatureId {
        sig.id()
    }
}

impl TalkMessageSignatureId {
    ///
    /// Retrieves the signature corresponding to this ID
    ///
    pub fn to_signature(&self) -> TalkMessageSignature {
        SIGNATURE_FOR_ID.lock().unwrap().get(self).unwrap().clone()
    }

    ///
    /// Retrieves the number of arguments for this signature ID
    ///
    pub fn len(&self) -> usize {
        SIGNATURE_FOR_ID.lock().unwrap().get(self).unwrap().len()
    }

    ///
    /// Appends an extra parameter to this signature ID
    ///
    /// The extra parameter must always have a value. For a message like `foo:`, adding a parameter like `bar:` will generate
    /// the message `foo:bar:`. However, for a unary message like `foo`, the generated signature will be `fooBar:`.
    ///
    pub fn with_extra_parameter(&self, new_symbol: impl Into<TalkSymbol>) -> TalkMessageSignatureId {
        let new_symbol = new_symbol.into();

        // Cache of the extra parameters for each symbol
        static EXTRA_PARAMETERS: Lazy<Mutex<TalkSparseArray<TalkSparseArray<TalkMessageSignatureId>>>> = Lazy::new(|| Mutex::new(TalkSparseArray::empty()));
        let mut extra_parameters = EXTRA_PARAMETERS.lock().unwrap();

        // Get the list of extra parameters for the symbol we're adding, or create a new map (assumption is we're adding a particular symbol to different messages so this is the more efficient representation)
        let new_symbol_id   = usize::from(new_symbol);
        let map_for_symbol  = if let Some(map_for_symbol) = extra_parameters.get_mut(new_symbol_id) {
            map_for_symbol
        } else {
            let map_for_symbol = TalkSparseArray::empty();
            extra_parameters.insert(new_symbol_id, map_for_symbol);
            extra_parameters.get_mut(new_symbol_id).unwrap()
        };

        // Fetch the signature ID for the specified symbol, or create a new one
        if let Some(new_signature) = map_for_symbol.get(self.0) {
            // We've already mapped this symbol
            *new_signature
        } else {
            // Generate a new message signature
            let signature       = self.to_signature();
            let new_signature   = match signature {
                TalkMessageSignature::Arguments(args) => {
                    let mut new_args = args.clone();
                    new_args.push(new_symbol);
                    TalkMessageSignature::Arguments(new_args).id()
                }

                TalkMessageSignature::Unary(old_symbol) => {
                    let old_symbol_name = old_symbol.name();
                    let new_symbol_name = format!("{}{}", old_symbol.name(), capitalized(new_symbol.name()));
                    let new_symbol      = TalkSymbol::from(new_symbol_name);

                    TalkMessageSignature::Arguments(smallvec![new_symbol]).id()
                }
            };

            // Cache the new signature so we can find it more quickly next time
            map_for_symbol.insert(self.0, new_signature);

            // The new signature is the result
            new_signature
        }
    }
}

impl<T> From<T> for TalkMessageSignatureId 
where
    T: Into<TalkSymbol>,
{
    fn from(into_symbol: T) -> TalkMessageSignatureId {
        let symbol = into_symbol.into();
        if symbol.is_keyword() || symbol.is_binary() {
            TalkMessageSignature::Arguments(smallvec![symbol]).into()
        } else {
            TalkMessageSignature::Unary(symbol).into()
        }
    }
}

impl<'a, T> From<&'a Vec<T>> for TalkMessageSignatureId  
where
    TalkSymbol: From<&'a T>,
{
    fn from(into_symbol: &'a Vec<T>) -> TalkMessageSignatureId {
        if into_symbol.len() == 1 {
            let symbol = TalkSymbol::from(&into_symbol[0]);
            if symbol.is_keyword() || symbol.is_binary() {
                TalkMessageSignature::Arguments(smallvec![symbol]).into()
            } else {
                TalkMessageSignature::Unary(symbol).into()
            }
        } else {
            TalkMessageSignature::Arguments(into_symbol.iter().map(|sym| sym.into()).collect()).into()
        }
    }
}

impl<T> From<Vec<T>> for TalkMessageSignatureId  
where
    TalkSymbol: From<T>,
{
    fn from(into_symbol: Vec<T>) -> TalkMessageSignatureId {
        if into_symbol.len() == 1 {
            let symbol = TalkSymbol::from(into_symbol.into_iter().next().unwrap());
            
            if symbol.is_keyword() || symbol.is_binary() {
                TalkMessageSignature::Arguments(smallvec![symbol]).into()
            } else {
                TalkMessageSignature::Unary(symbol).into()
            }
        } else {
            TalkMessageSignature::Arguments(into_symbol.into_iter().map(|sym| sym.into()).collect()).into()
        }
    }
}

impl<T1, T2> From<(T1, T2)> for TalkMessageSignatureId 
where
    T1: Into<TalkSymbol>,
    T2: Into<TalkSymbol>,
{
    fn from((a, b): (T1, T2)) -> TalkMessageSignatureId {
        TalkMessageSignature::Arguments(smallvec![a.into(), b.into()]).into()
    }
}

impl<T1, T2, T3> From<(T1, T2, T3)> for TalkMessageSignatureId 
where
    T1: Into<TalkSymbol>,
    T2: Into<TalkSymbol>,
    T3: Into<TalkSymbol>,
{
    fn from((a, b, c): (T1, T2, T3)) -> TalkMessageSignatureId {
        TalkMessageSignature::Arguments(smallvec![a.into(), b.into(), c.into()]).into()
    }
}

impl<T1, T2, T3, T4> From<(T1, T2, T3, T4)> for TalkMessageSignatureId 
where
    T1: Into<TalkSymbol>,
    T2: Into<TalkSymbol>,
    T3: Into<TalkSymbol>,
    T4: Into<TalkSymbol>,
{
    fn from((a, b, c, d): (T1, T2, T3, T4)) -> TalkMessageSignatureId {
        TalkMessageSignature::Arguments(smallvec![a.into(), b.into(), c.into(), d.into()]).into()
    }
}

impl From<TalkMessageSignatureId> for usize {
    fn from(message_sig: TalkMessageSignatureId) -> usize {
        message_sig.0
    }
}

impl fmt::Debug for TalkMessageSignature {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            TalkMessageSignature::Unary(symbol)     => fmt.write_fmt(format_args!("{:?}", symbol)),
            TalkMessageSignature::Arguments(args)   => fmt.write_fmt(format_args!("{:?}", args.iter().map(|arg| format!("{:?}", arg)).collect::<Vec<_>>().join(" "))),
        }
    }
}

impl fmt::Debug for TalkMessageSignatureId {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        self.to_signature().fmt(fmt)
    }
}

impl fmt::Debug for TalkMessage {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            TalkMessage::Unary(signature_id)                => signature_id.fmt(fmt),
            TalkMessage::WithArguments(signature_id, args)  => {
                // Fetch the symbols making up the arguments
                let signature = signature_id.to_signature();

                let arg_symbols = match &signature {
                    TalkMessageSignature::Unary(_)          => vec![],
                    TalkMessageSignature::Arguments(args)   => args.iter().collect::<Vec<_>>(),
                };

                // Convert each argument to a message
                for idx in 0..args.len() {
                    if idx != 0 {
                        fmt.write_str(" ")?;
                    }

                    if idx > arg_symbols.len() {
                        fmt.write_fmt(format_args!("?: {:?}", args[idx]))?;
                    } else {
                        fmt.write_fmt(format_args!("{:?} {:?}", arg_symbols[idx], args[idx]))?;
                    }
                }

                Ok(())
            }
        }
    }
}

impl From<TalkMessage> for TalkValue {
    fn from(message: TalkMessage) -> TalkValue {
        TalkValue::Message(Box::new(message))
    }
}

///
/// Single-parameter messages can be treated as TalkValues
///
impl TalkValueType for TalkMessage {
    fn into_talk_value<'a>(&self, context: &'a TalkContext) -> TalkOwned<TalkValue, &'a TalkContext> {
        let message = self.clone_in_context(context);
        TalkOwned::new(TalkValue::from(message), context)
    }

    #[inline]
    fn try_from_talk_value<'a>(value: TalkOwned<TalkValue, &'a TalkContext>, context: &'a TalkContext) -> Result<Self, TalkError> {
        match &*value {
            TalkValue::Message(_) => {
                let msg = value.leak();
                if let TalkValue::Message(msg) = msg {
                    let msg = *msg;
                    msg.retain(context);
                    Ok(msg)
                } else {
                    unreachable!()
                }
            }

            _ => Err(TalkError::NotAMessage)
        }
    }
}

impl TalkMessageType for TalkMessage {
    /// Converts a message to an object of this type
    fn from_message<'a>(message: TalkOwned<TalkMessage, &'a TalkContext>, _context: &'a TalkContext) -> Result<Self, TalkError> {
        Ok(message.leak())
    }

    /// Converts an object of this type to a message
    fn to_message<'a>(&self, context: &'a TalkContext) -> TalkOwned<TalkMessage, &'a TalkContext> {
        TalkOwned::new(self.clone_in_context(context), context)
    }
}

///
/// Capitalizes the first letter of a string
///
fn capitalized(name: &str) -> String {
    let mut name_chrs = name.chars();

    if let Some(first_chr) = name_chrs.next() {
        first_chr.to_uppercase()
            .chain(name_chrs)
            .collect()
    } else {
        String::new()
    }
}
