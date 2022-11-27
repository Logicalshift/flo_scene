use super::class::*;
use super::context::*;
use super::continuation::*;
use super::message::*;
use super::runtime::*;
use super::value::*;

use futures::prelude::*;

///
/// A reference to the data for a class from the allocator
///
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct TalkDataHandle(pub usize);

///
/// A reference to a data structure within a TalkContext
///
/// FloTalk data is stored by class and handle. References are only valid for the context that they were created for.
///
#[derive(PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct TalkReference(pub (super) TalkClass, pub (super) TalkDataHandle);

///
/// A reference to a cell block (set of reference-counted values stored within a TalkContext)
///
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct TalkCellBlock(pub u32);

///
/// A reference to a specific value within a cell block
///
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct TalkCell(pub TalkCellBlock, pub u32);

///
/// A reference to a cell in a frame
///
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Debug)]
pub struct TalkFrameCell {
    /// The frame this cell is found in (with '0' being the current frame, '1' being the parent's frame, etc). The frame should contain the cell block.
    pub frame: u32,

    /// The cell number within the frame
    pub cell: u32,
}

impl TalkReference {
    ///
    /// Creates a reference from a data handle
    ///
    #[inline]
    pub fn from_handle(class: TalkClass, data_handle: TalkDataHandle) -> TalkReference {
        TalkReference(class, data_handle)
    }

    ///
    /// Increases the reference count for this reference. References are freed once the count reaches 0.
    ///
    #[inline]
    pub fn add_reference(&self, context: &TalkContext) {
        match context.get_callbacks(self.0) {
            Some(callbacks) => callbacks.add_reference(self.1),
            None            => { /* Should be unreachable */ }
        }
    }

    ///
    /// Decreases the reference count for this reference. References are freed once the count reaches 0.
    ///
    #[inline]
    pub fn remove_reference(&self, context: &TalkContext) {
        match context.get_callbacks(self.0) {
            Some(callbacks) => callbacks.remove_reference(self.1),
            None            => { /* Should be unreachable */ }
        }
    }

    ///
    /// Sends a message to the object this reference points at, then releases it.
    ///
    #[inline]
    pub fn send_message_in_context<'a>(self, message: TalkMessage, context: &TalkContext) -> TalkContinuation<'a> {
        match context.get_callbacks(self.0) {
            Some(callbacks)     => callbacks.send_message(self, message, context),
            None                => unreachable!("A reference should not reference a class that has not been initialized in the context"),   // As we have to send a message to an instance of a class before we can have a reference to that class, the callbacks should always exist when sending a message to a reference
        }
    }

    ///
    /// Sends a message to this object
    ///
    pub fn send_message_later<'a>(self, message: TalkMessage) -> TalkContinuation<'a> {
        TalkContinuation::Soon(Box::new(move |talk_context| self.send_message_in_context(message, talk_context)))
    }

    ///
    /// Sends a message to this object
    ///
    pub fn send_message(self, message: TalkMessage, runtime: &TalkRuntime) -> impl Future<Output=TalkValue> {
        runtime.run_continuation(self.send_message_later(message))
    }

    ///
    /// Return the data for a reference cast to a target type (if it can be read as that type)
    ///
    pub fn read_data_in_context<TTargetData>(&self, context: &mut TalkContext) -> Option<TTargetData> 
    where
        TTargetData: 'static,
    {
        context.get_callbacks(self.0).unwrap().read_data(self.1)
    }

    ///
    /// Return the data for a reference cast to a target type (if it can be read as that type)
    ///
    pub fn read_data<'a, TTargetData>(&'a self, runtime: &'a TalkRuntime) -> impl 'a+Future<Output=Option<TTargetData>>
    where
        TTargetData: 'static,
    {
        async move {
            let mut context = runtime.context.lock().await;

            self.read_data_in_context(&mut *context)
        }
    }
}

impl TalkCellBlock {
    ///
    /// Returns a cell with a particular index
    ///
    #[inline]
    pub fn cell(&self, cell_number: u32) -> TalkCell {
        TalkCell(*self, cell_number)
    }
}
