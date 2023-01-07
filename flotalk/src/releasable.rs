use super::context::*;
use super::error::*;
use super::message::*;
use super::number::*;
use super::reference::*;
use super::symbol::*;
use super::value::*;

use smallvec::*;

use std::fmt;
use std::ops::{Deref, DerefMut};
use std::sync::*;

///
/// Trait implemented by types that can be released in a context
///
pub trait TalkReleasable {
    ///
    /// Releases this item within the specified context
    ///
    fn release_in_context(self, context: &TalkContext);
}

///
/// Trait implemented by types that can be cloned in a context
///
pub trait TalkCloneable {
    ///
    /// Releases this item within the specified context
    ///
    fn clone_in_context(&self, context: &TalkContext) -> Self;
}

///
/// Trait implemented by something that can own a TalkReleasable
///
pub trait TalkReleasableOwner<TOwned: TalkReleasable> {
    ///
    /// Reduces the reference count for a variable
    ///
    fn release_value(&self, value: TOwned);
}

///
/// Trait implemented by something that can own a TalkReleasable
///
pub trait TalkCloningOwner<TOwned> {
    ///
    /// Creates a clone of this value using the context of this owner
    ///
    fn clone_value(&self, value: &TOwned) -> TOwned;
}

///
/// A value that will be released when dropped
///
pub struct TalkOwned<TReleasable, TOwner>
where
    TReleasable:    TalkReleasable,
    TOwner:         TalkReleasableOwner<TReleasable>,
{
    owner: TOwner,
    value: Option<TReleasable>
}

impl<'a, TReleasable> TalkReleasableOwner<TReleasable> for &'a TalkContext
where
    TReleasable: TalkReleasable,
{
    #[inline]
    fn release_value(&self, value: TReleasable) {
        value.release_in_context(*self)
    }
}

impl<'a, TReleasable> TalkCloningOwner<TReleasable> for &'a TalkContext
where
    TReleasable: TalkCloneable,
{
    #[inline]
    fn clone_value(&self, value: &TReleasable) -> TReleasable {
        value.clone_in_context(*self)
    }
}

impl<'a, TReleasable> TalkReleasableOwner<TReleasable> for &'a mut TalkContext
where
    TReleasable: TalkReleasable,
{
    #[inline]
    fn release_value(&self, value: TReleasable) {
        value.release_in_context(*self)
    }
}

impl<TReleasable, TOwner> TalkOwned<TReleasable, TOwner>
where
    TReleasable:    TalkReleasable,
    TOwner:         TalkReleasableOwner<TReleasable>,
{
    ///
    /// Creates a new TalkOwned object
    ///
    #[inline]
    pub fn new(value: TReleasable, owner: TOwner) -> TalkOwned<TReleasable, TOwner> {
        TalkOwned {
            owner: owner, 
            value: Some(value),
        }
    }

    ///
    /// Changes the type/value of the internal value without releasing it
    ///
    #[inline]
    pub fn map<TTargetType>(mut self, map_fn: impl FnOnce(TReleasable) -> TTargetType) -> TalkOwned<TTargetType, TOwner>
    where
        TTargetType:    TalkReleasable,
        TOwner:         TalkReleasableOwner<TTargetType> + Clone,
    {
        TalkOwned {
            owner: self.owner.clone(),
            value: Some((map_fn)(self.value.take().unwrap()))
        }
    }

    ///
    /// Retrieves the internal value, and no longer releases it when it's dropped (the caller becomes responsible for managing the lifetime of this value)
    ///
    #[inline]
    pub fn leak(mut self) -> TReleasable {
        match self.value.take() {
            Some(value) => value,
            None        => unreachable!(),
        }
    }
}

impl<'a, TReleasable> TalkOwned<TReleasable, &'a TalkContext>
where
    TReleasable:    TalkReleasable,
{
    ///
    /// Returns the context for this 'owned' item
    ///
    pub fn context(&self) -> &'a TalkContext {
        self.owner
    }
}

impl<TReleasable, TOwner> Drop for TalkOwned<TReleasable, TOwner>
where
    TReleasable:    TalkReleasable,
    TOwner:         TalkReleasableOwner<TReleasable>,
{
    #[inline]
    fn drop(&mut self) {
        if let Some(value) = self.value.take() {
            self.owner.release_value(value);
        }
    }
}

impl<TReleasable, TOwner> Clone for TalkOwned<TReleasable, TOwner>
where
    TReleasable:    TalkReleasable,
    TOwner:         TalkReleasableOwner<TReleasable> + TalkCloningOwner<TReleasable> + Clone,
{
    fn clone(&self) -> Self {
        match &self.value {
            Some(value) => TalkOwned {
                owner: self.owner.clone(),
                value: Some(self.owner.clone_value(value)),
            },
            None        => unreachable!()
        }
    }
}

impl<TReleasable, TOwner> Deref for TalkOwned<TReleasable, TOwner>
where
    TReleasable:    TalkReleasable,
    TOwner:         TalkReleasableOwner<TReleasable>,
{
    type Target = TReleasable;

    #[inline]
    fn deref(&self) -> &TReleasable {
        match &self.value {
            Some(value) => value,
            None        => unreachable!()
        }
    }
}

impl<TReleasable, TOwner> DerefMut for TalkOwned<TReleasable, TOwner>
where
    TReleasable:    TalkReleasable,
    TOwner:         TalkReleasableOwner<TReleasable>,
{
    #[inline]
    fn deref_mut(&mut self) -> &mut TReleasable {
        match &mut self.value {
            Some(value) => value,
            None        => unreachable!()
        }
    }
}

impl<TReleasable, TOwner> fmt::Debug for TalkOwned<TReleasable, TOwner>
where
    TReleasable:    TalkReleasable + fmt::Debug,
    TOwner:         TalkReleasableOwner<TReleasable>,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        if let Some(value) = &self.value {
            fmt.write_fmt(format_args!("{:?}", value))
        } else {
            fmt.write_fmt(format_args!("<< RELEASED >>"))
        }
    }
}

impl<TReleasable, TOwner> TalkReleasable for TalkOwned<TReleasable, TOwner>
where
    TReleasable:    TalkReleasable + fmt::Debug,
    TOwner:         TalkReleasableOwner<TReleasable>,
{
    fn release_in_context(mut self, context: &TalkContext) {
        if let Some(value) = self.value.take() {
            value.release_in_context(context);
        }
    }
}

impl TalkReleasable for TalkValue {
    ///
    /// Decreases the reference count of this value by 1
    ///
    #[inline]
    fn release_in_context(self, context: &TalkContext) {
        self.release(context);
    }
}

impl TalkReleasable for TalkCellBlock {
    ///
    /// Decreases the reference count of this value by 1
    ///
    #[inline]
    fn release_in_context(self, context: &TalkContext) {
        context.release_cell_block(self);
    }
}

impl TalkCloneable for TalkValue {
    ///
    /// Creates a copy of this value in the specified context
    ///
    /// This will copy this value and increase its reference count
    ///
    #[inline]
    fn clone_in_context(&self, context: &TalkContext) -> Self {
        use TalkValue::*;

        match self {
            Nil                     => Nil,
            Reference(reference)    => Reference(reference.clone_in_context(context)),
            Bool(boolean)           => Bool(*boolean),
            Int(int)                => Int(*int),
            Float(float)            => Float(*float),
            String(string)          => String(Arc::clone(string)),
            Character(character)    => Character(*character),
            Symbol(symbol)          => Symbol(*symbol),
            Selector(symbol)        => Selector(*symbol),
            Array(array)            => Array(array.iter().map(|val| val.clone_in_context(context)).collect()),
            Message(msg)            => Message(Box::new(msg.clone_in_context(context))),
            Error(error)            => Error(error.clone()),
        }
    }
}

impl TalkReleasable for ()                      { #[inline] fn release_in_context(self, _: &TalkContext) { } }
impl TalkReleasable for bool                    { #[inline] fn release_in_context(self, _: &TalkContext) { } }
impl TalkReleasable for usize                   { #[inline] fn release_in_context(self, _: &TalkContext) { } }
impl TalkReleasable for i64                     { #[inline] fn release_in_context(self, _: &TalkContext) { } }
impl TalkReleasable for f64                     { #[inline] fn release_in_context(self, _: &TalkContext) { } }
impl TalkReleasable for char                    { #[inline] fn release_in_context(self, _: &TalkContext) { } }
impl TalkReleasable for Arc<String>             { #[inline] fn release_in_context(self, _: &TalkContext) { } }
impl TalkReleasable for TalkSymbol              { #[inline] fn release_in_context(self, _: &TalkContext) { } }
impl TalkReleasable for TalkNumber              { #[inline] fn release_in_context(self, _: &TalkContext) { } }
impl TalkReleasable for TalkError               { #[inline] fn release_in_context(self, _: &TalkContext) { } }
impl TalkReleasable for TalkMessageSignatureId  { #[inline] fn release_in_context(self, _: &TalkContext) { } }

impl TalkReleasable for TalkReference {
    ///
    /// Decreases the reference count of this value by 1
    ///
    #[inline]
    fn release_in_context(self, context: &TalkContext) {
        self.release(context);
    }
}

impl TalkCloneable for TalkReference {
    ///
    /// This will create a copy of this reference and increase its reference count
    ///
    #[inline]
    fn clone_in_context(&self, context: &TalkContext) -> TalkReference {
        let clone = TalkReference(self.0, self.1);
        if let Some(callbacks) = context.get_callbacks(self.0) {
            callbacks.retain(self.1, context);
        }
        clone
    }
}

impl<T> TalkReleasable for Box<T>
where
    T: TalkReleasable
{
    #[inline] fn release_in_context(self, context: &TalkContext) {
        (*self).release_in_context(context)
    }
}

impl<T> TalkReleasable for Option<T>
where
    T: TalkReleasable
{
    #[inline] fn release_in_context(self, context: &TalkContext) {
        if let Some(to_release) = self {
            to_release.release_in_context(context)
        }
    }
}

impl<T, E> TalkReleasable for Result<T, E>
where
    T: TalkReleasable
{
    #[inline] fn release_in_context(self, context: &TalkContext) {
        if let Ok(to_release) = self {
            to_release.release_in_context(context)
        }
    }
}

impl<TReleasable> TalkReleasable for Vec<TReleasable>
where
    TReleasable:        TalkReleasable,
{
    #[inline]
    fn release_in_context(self, context: &TalkContext) {
        self.into_iter().for_each(|item| item.release_in_context(context));
    }
}

impl<TReleasable> TalkReleasable for SmallVec<[TReleasable; 4]>
where
    TReleasable:        TalkReleasable,
{
    #[inline]
    fn release_in_context(self, context: &TalkContext) {
        self.into_iter().for_each(|item| item.release_in_context(context));
    }
}
