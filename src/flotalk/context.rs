use super::class::*;
use super::dispatch_table::*;
use super::reference::*;
use super::value_messages::*;

use ouroboros::{self_referencing};

use std::ops::{Deref};

///
/// A talk context is a self-contained representation of the state of a flotalk interpreter
///
/// Contexts are only accessed on one thread at a time. They're wrapped by a `TalkRuntime`, which deals with
/// scheduling continuations on a context
///
pub struct TalkContext {
    /// Callbacks for this context, indexed by class ID
    context_callbacks: Vec<Option<TalkClassContextCallbacks>>,

    /// Dispatch tables by class
    pub (super) class_dispatch_tables: Vec<Option<TalkMessageDispatchTable<TalkDataHandle>>>,

    /// Dispatch tables by value
    pub (super) value_dispatch_tables: TalkValueDispatchTables,
}

///
/// A reference to some data contained within a TalkContext
///
#[self_referencing]
pub struct TalkContextReference<'a, TData> 
where
    TData: 'a,
{
    /// The context that the data is borrowed from
    context: &'a mut TalkContext,

    /// The data borrowed from the context
    #[borrows(mut context)]
    data: &'this mut TData,
}

impl<'a, TData> TalkContextReference<'a, TData>
where
    TData: 'a,
{
    ///
    /// Accesses the data inside this reference
    ///
    #[inline]
    pub fn data(&self) -> &TData {
        self.borrow_data()
    }

    ///
    /// Access the data in this reference using a mutable update
    ///
    #[inline]
    pub fn update_data<TReturn>(&mut self, with_fn: impl for<'b> FnOnce(&'b mut TData) -> TReturn) -> TReturn {
        self.with_data_mut(move |data| with_fn(*data))
    }

    ///
    /// Releases the reference borrowed by this item and returns the underlying context
    ///
    #[inline]
    pub fn to_context(self) -> &'a mut TalkContext {
        let heads = self.into_heads();

        heads.context
    }
}

impl<'a, TData> Deref for TalkContextReference<'a, TData> {
    type Target = TData;

    fn deref(&self) -> &Self::Target {
        self.borrow_data()
    }
}

impl TalkContext {
    ///
    /// Creates a new, empty context
    ///
    pub fn empty() -> TalkContext {
        TalkContext {
            context_callbacks:      vec![],
            class_dispatch_tables:  vec![],
            value_dispatch_tables:  TalkValueDispatchTables::default(),
        }
    }

    ///
    /// Creates the allocator for a particular class
    ///
    fn create_callbacks<'a>(&'a mut self, class: TalkClass) -> &'a mut TalkClassContextCallbacks {
        let TalkClass(class_id) = class;

        while self.context_callbacks.len() <= class_id {
            self.context_callbacks.push(None);
        }

        let class_callbacks     = class.callbacks();
        let context_callbacks   = class_callbacks.create_in_context();

        self.context_callbacks[class_id] = Some(context_callbacks);
        self.context_callbacks[class_id].as_mut().unwrap()
    }

    ///
    /// Retrieves the allocator for a class
    ///
    #[inline]
    pub (super) fn get_callbacks<'a>(&'a mut self, class: TalkClass) -> &'a mut TalkClassContextCallbacks {
        let TalkClass(class_id) = class;

        if self.context_callbacks.len() > class_id {
            if self.context_callbacks[class_id].is_some() {
                return self.context_callbacks[class_id].as_mut().unwrap()
            }
        }

        self.create_callbacks(class)
    }

    ///
    /// Creates a 'borrowed context reference' via some class context callbacks
    ///
    #[inline]
    pub (super) fn borrow_with_callbacks<'a, TData>(&'a mut self, class: TalkClass, with_fn: impl for<'b> FnOnce(&'b mut TalkClassContextCallbacks) -> &'b mut TData) -> TalkContextReference<'a, TData> 
    where
        TData: 'a
    {
        let reference = TalkContextReferenceBuilder {
            context:        self,
            data_builder:   |val| { 
                let callbacks = val.get_callbacks(class);
                with_fn(callbacks)
            },
        }.build();

        reference
    }
}
