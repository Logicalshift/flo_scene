use super::context::*;
use super::error::*;
use super::message::*;
use super::number::*;
use super::reference::*;
use super::symbol::*;
use super::value::*;

use smallvec::*;

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
/// A value that will be released when dropped
///
pub struct TalkOwned<'a, TReleasable>
where
    TReleasable: TalkReleasable
{
    context:    &'a TalkContext,
    value:      Option<TReleasable>
}

impl<'a, TReleasable> TalkOwned<'a, TReleasable>
where
    TReleasable: TalkReleasable
{
    ///
    /// Creates a new TalkOwned object
    ///
    #[inline]
    pub fn new(value: TReleasable, context: &'a TalkContext) -> TalkOwned<'a, TReleasable> {
        TalkOwned {
            context:    context, 
            value:      Some(value),
        }
    }

    ///
    /// Changes the type/value of the internal value without releasing it
    ///
    #[inline]
    pub fn map<TTargetType>(mut self, map_fn: impl FnOnce(TReleasable) -> TTargetType) -> TalkOwned<'a, TTargetType>
    where
        TTargetType: TalkReleasable 
    {
        TalkOwned {
            context:    self.context,
            value:      Some((map_fn)(self.value.take().unwrap()))
        }
    }

    ///
    /// Retrieves the internal value, and no longer releases it when it's dropped
    ///
    #[inline]
    pub (super) fn leak(mut self) -> TReleasable {
        match self.value.take() {
            Some(value) => value,
            None        => unreachable!(),
        }
    }
}

impl<'a, TReleasable> Drop for TalkOwned<'a, TReleasable>
where
    TReleasable: TalkReleasable
{
    #[inline]
    fn drop(&mut self) {
        if let Some(value) = self.value.take() {
            value.release_in_context(self.context);
        }
    }
}

impl<'a, TReleasable> Clone for TalkOwned<'a, TReleasable>
where
    TReleasable: TalkReleasable + TalkCloneable
{
    fn clone(&self) -> Self {
        match &self.value {
            Some(value) => TalkOwned {
                context:    self.context,
                value:      Some(value.clone_in_context(self.context)),
            },
            None        => unreachable!()
        }
    }
}

impl<'a, TReleasable> Deref for TalkOwned<'a, TReleasable>
where
    TReleasable: TalkReleasable
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

impl<'a, TReleasable> DerefMut for TalkOwned<'a, TReleasable>
where
    TReleasable: TalkReleasable
{
    #[inline]
    fn deref_mut(&mut self) -> &mut TReleasable {
        match &mut self.value {
            Some(value) => value,
            None        => unreachable!()
        }
    }
}

impl TalkReleasable for TalkValue {
    ///
    /// Decreases the reference count of this value by 1
    ///
    #[inline]
    fn release_in_context(self, context: &TalkContext) {
        self.remove_reference(context);
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
            Error(error)            => Error(error.clone()),
        }
    }
}

impl TalkReleasable for ()                      { #[inline] fn release_in_context(self, _: &TalkContext) { } }
impl TalkReleasable for bool                    { #[inline] fn release_in_context(self, _: &TalkContext) { } }
impl TalkReleasable for i64                     { #[inline] fn release_in_context(self, _: &TalkContext) { } }
impl TalkReleasable for f64                     { #[inline] fn release_in_context(self, _: &TalkContext) { } }
impl TalkReleasable for char                    { #[inline] fn release_in_context(self, _: &TalkContext) { } }
impl TalkReleasable for Arc<String>             { #[inline] fn release_in_context(self, _: &TalkContext) { } }
impl TalkReleasable for TalkSymbol              { #[inline] fn release_in_context(self, _: &TalkContext) { } }
impl TalkReleasable for TalkNumber              { #[inline] fn release_in_context(self, _: &TalkContext) { } }
impl TalkReleasable for TalkError               { #[inline] fn release_in_context(self, _: &TalkContext) { } }
impl TalkReleasable for TalkMessageSignatureId  { #[inline] fn release_in_context(self, _: &TalkContext) { } }
impl<T> TalkReleasable for &T                   { #[inline] fn release_in_context(self, _: &TalkContext) { } }
impl<T> TalkReleasable for &mut T               { #[inline] fn release_in_context(self, _: &TalkContext) { } }

impl TalkReleasable for TalkReference {
    ///
    /// Decreases the reference count of this value by 1
    ///
    #[inline]
    fn release_in_context(self, context: &TalkContext) {
        self.remove_reference(context);
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
            callbacks.add_reference(self.1);
        }
        clone
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
