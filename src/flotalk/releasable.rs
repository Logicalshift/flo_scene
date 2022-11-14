use super::context::*;

use std::ops::{Deref};

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
}

impl<'a, TReleasable> Drop for TalkOwned<'a, TReleasable>
where
    TReleasable: TalkReleasable
{
    #[inline]
    fn drop(&mut self) {
        self.value.take().unwrap().release_in_context(self.context);
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
