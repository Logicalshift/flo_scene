use super::context::*;
use super::continuation::*;
use super::message::*;
use super::reference::*;
use super::runtime::*;
use super::value::*;

use futures::prelude::*;
use futures::task::{Poll};

use std::any::*;
use std::cell::*;
use std::sync::*;
use std::collections::{HashMap};

lazy_static! {
    /// The ID to assign to the next class that is created
    static ref NEXT_CLASS_ID: Mutex<usize>                                      = Mutex::new(0);

    /// A vector containing the boxed class definitions (as an Arc<TClassDefinition>), indexed by class ID
    static ref CLASS_DEFINITIONS: Mutex<Vec<Option<Box<dyn Send + Any>>>>       = Mutex::new(vec![]);

    /// A vector containing the callbacks for each class, indexed by class ID (callbacks can be used without knowing the underlying types)
    static ref CLASS_CALLBACKS: Mutex<Vec<Option<&'static TalkClassCallbacks>>> = Mutex::new(vec![]);

    /// A hashmap containing data conversions for fetching the values stored for a particular class (class definition type -> target type -> converter function)
    static ref CLASS_CONVERTERS: Mutex<HashMap<TypeId, HashMap<TypeId, Box<dyn Send + Fn(Box<dyn Any>) -> Box<dyn Any>>>>> = Mutex::new(HashMap::new());
}

thread_local! {
    static LOCAL_CLASS_CALLBACKS: RefCell<Vec<Option<&'static TalkClassCallbacks>>> = RefCell::new(vec![]);
}

///
/// Callbacks for addressing a TalkClass
///
pub (crate) struct TalkClassCallbacks {
    /// Creates the callbacks for this class in a context
    create_in_context: Box<dyn Send + Sync + Fn() -> TalkClassContextCallbacks>,
}

///
/// Callbacks for addressing a TalkClass within a context
///
pub (crate) struct TalkClassContextCallbacks {
    /// Sends a message to an object
    send_message: Box<dyn Send + FnMut(TalkDataHandle, TalkMessage) -> TalkContinuation>,

    /// Sends a message to the class object
    send_class_message: Box<dyn Send + Sync + Fn(TalkMessage) -> TalkContinuation>,

    /// Add to the reference count for a data handle
    add_reference: Box<dyn Send + FnMut(TalkDataHandle) -> ()>,

    /// Decreases the reference count for a data handle, and frees it if the count reaches 0
    remove_reference: Box<dyn Send + FnMut(TalkDataHandle) -> ()>,

    /// The definition for this class (a boxed Arc<TalkClassDefinition>)
    class_definition: Box<dyn Send + Any>,

    /// The allocator for this class (a boxed Arc<Mutex<TalkClassDefinition::Allocator>>)
    allocator: Box<dyn Send + Any>,
}

///
/// A TalkClass is an identifier for a FloTalk class
///
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TalkClass(pub (crate) usize);

impl TalkClassCallbacks {
    #[inline]
    pub (crate) fn create_in_context(&self) -> TalkClassContextCallbacks {
        (self.create_in_context)()
    }
}

impl TalkClassContextCallbacks {
    #[inline]
    pub (crate) fn send_message(&mut self, data_handle: TalkDataHandle, message: TalkMessage) -> TalkContinuation {
        (self.send_message)(data_handle, message)
    }

    #[inline]
    pub (crate) fn add_reference(&mut self, data_handle: TalkDataHandle) {
        (self.add_reference)(data_handle)
    }

    #[inline]
    pub (crate) fn remove_reference(&mut self, data_handle: TalkDataHandle) {
        (self.remove_reference)(data_handle)
    }

    #[inline]
    pub (crate) fn send_class_message(&self, message: TalkMessage) -> TalkContinuation {
        (self.send_class_message)(message)
    }
}

impl TalkClass {
    ///
    /// Creates a new class identifier
    ///
    fn new() -> TalkClass {
        let class_id = {
            let mut next_class_id   = NEXT_CLASS_ID.lock().unwrap();
            let class_id            = *next_class_id;
            *next_class_id          += 1;
            
            class_id
        };

        TalkClass(class_id)
    }
}

///
/// A class definition is a trait implemented by a FloTalk class
///
pub trait TalkClassDefinition : Send + Sync {
    /// The type of the data stored by an object of this class
    type Data: Send;

    /// The allocator is used to manage the memory of this class within a context
    type Allocator: TalkClassAllocator<Data=Self::Data>;

    ///
    /// Creates the allocator for this class
    ///
    fn create_allocator(&self) -> Self::Allocator;

    ///
    /// Sends a message to the class object itself
    ///
    fn send_class_message(&self, message: TalkMessage, class_id: TalkClass, allocator: &mut Self::Allocator) -> TalkContinuation;

    ///
    /// Sends a message to an instance of this class
    ///
    fn send_instance_message(&self, message: TalkMessage, reference: TalkReference, target: &mut Self::Data) -> TalkContinuation;
}

///
/// A class allocator is used to manage the memory of a class
///
pub trait TalkClassAllocator : Send {
    /// The type of data stored for this class
    type Data: Send;

    ///
    /// Retrieves a reference to the data attached to a handle (panics if the handle has been released)
    ///
    fn retrieve<'a>(&'a mut self, handle: TalkDataHandle) -> &'a mut Self::Data;

    ///
    /// Adds to the reference count for a data handle
    ///
    fn add_reference(&mut self, handle: TalkDataHandle);

    ///
    /// Removes from the reference count for a data handle (freeing it if the count reaches 0)
    ///
    fn remove_reference(&mut self, handle: TalkDataHandle);
}

impl TalkClass {
    // TODO: we need to share the allocator between several functions, but those functions should all 'exist' in the same thread,
    //       so the allocator should not need to be an Arc<Mutex<...>>: can we use something faster to access? Normally this 
    //       doesn't matter too much but as this ends up in the inner loop of a language interpreter it seems that this could make
    //       a noticeable performance difference.

    ///
    /// Creates the 'send message' method for an allocator
    ///
    fn callback_send_message<TClass>(class_id: TalkClass, class_definition: Arc<TClass>, allocator: Arc<Mutex<TClass::Allocator>>) -> Box<dyn Send + FnMut(TalkDataHandle, TalkMessage) -> TalkContinuation> 
    where
        TClass: 'static + TalkClassDefinition,
    {
        Box::new(move |data_handle, message| {
            let mut allocator   = allocator.lock().unwrap();
            let data            = allocator.retrieve(data_handle);

            class_definition.send_instance_message(message, TalkReference(class_id, data_handle), data)
        })
    }

    ///
    /// Creates the 'add reference' method for an allocator
    ///
    fn callback_add_reference(allocator: Arc<Mutex<impl 'static + TalkClassAllocator>>) -> Box<dyn Send + FnMut(TalkDataHandle) -> ()> {
        Box::new(move |data_handle| {
            let mut allocator = allocator.lock().unwrap();
            allocator.add_reference(data_handle)
        })
    }

    ///
    /// Creates the 'remove reference' method for an allocator
    ///
    fn callback_remove_reference(allocator: Arc<Mutex<impl 'static + TalkClassAllocator>>) -> Box<dyn Send + FnMut(TalkDataHandle) -> ()> {
        Box::new(move |data_handle| {
            let mut allocator = allocator.lock().unwrap();
            allocator.remove_reference(data_handle)
        })
    }

    ///
    /// Creates the 'send class message' function for a class
    ///
    fn callback_send_class_message<TClass>(class_id: TalkClass, definition: Arc<TClass>, allocator: Arc<Mutex<TClass::Allocator>>) -> Box<dyn Send + Sync + Fn(TalkMessage) -> TalkContinuation> 
    where
        TClass: 'static + TalkClassDefinition,
    {
        Box::new(move |message| {
            let mut allocator   = allocator.lock().unwrap();

            definition.send_class_message(message, class_id, &mut *allocator)
        })
    }

    ///
    /// Creates the 'create in context' function for a class
    ///
    fn callback_create_in_context(class_id: TalkClass, definition: Arc<impl 'static + TalkClassDefinition>) -> Box<dyn Send + Sync + Fn() -> TalkClassContextCallbacks> {
        Box::new(move || {
            let allocator = Arc::new(Mutex::new(definition.create_allocator()));

            TalkClassContextCallbacks {
                send_message:       Self::callback_send_message(class_id, Arc::clone(&definition), Arc::clone(&allocator)),
                send_class_message: Self::callback_send_class_message(class_id, Arc::clone(&definition), Arc::clone(&allocator)),
                add_reference:      Self::callback_add_reference(Arc::clone(&allocator)),
                remove_reference:   Self::callback_remove_reference(Arc::clone(&allocator)),
                class_definition:   Box::new(Arc::clone(&definition)),
                allocator:          Box::new(Arc::clone(&allocator)),
            }
        })
    }

    ///
    /// Creates a TalkClass from a definition
    ///
    pub fn create(definition: impl 'static + TalkClassDefinition) -> TalkClass {
        // Create an identifier for this class
        let definition      = Arc::new(definition);
        let class           = TalkClass::new();
        let TalkClass(idx)  = class;

        // Store the class definition
        let mut class_definitions = CLASS_DEFINITIONS.lock().unwrap();
        while class_definitions.len() <= idx {
            class_definitions.push(None);
        }
        class_definitions[idx] = Some(Box::new(Arc::clone(&definition)));

        // Create the class callbacks
        let class_callbacks = TalkClassCallbacks {
            create_in_context:  Self::callback_create_in_context(class, Arc::clone(&definition)),
        };

        // Store as a static reference (classes live for the lifetime of the program)
        let class_callbacks     = Box::new(class_callbacks);
        let class_callbacks     = Box::leak(class_callbacks);
        let mut all_callbacks   = CLASS_CALLBACKS.lock().unwrap();

        while all_callbacks.len() <= idx {
            all_callbacks.push(None);
        }
        all_callbacks[idx] = Some(class_callbacks);

        // Return the definition we just created
        class
    }

    ///
    /// Looks up the callbacks for this class, 
    ///
    fn make_local_callbacks(&self) -> &'static TalkClassCallbacks {
        let TalkClass(idx) = *self;

        // Look up the callback in the global set
        let callback = (*CLASS_CALLBACKS.lock().unwrap())[idx].unwrap();

        // Store in the thread-local set so we can retrieve it more quickly in future
        LOCAL_CLASS_CALLBACKS.with(|local_class_callbacks| {
            let mut local_class_callbacks = local_class_callbacks.borrow_mut();

            while local_class_callbacks.len() <= idx {
                local_class_callbacks.push(None);
            }
            local_class_callbacks[idx] = Some(callback);
        });

        // Result is the callback we looked up
        callback
    }

    ///
    /// Retrieve the callbacks for this class
    ///
    #[inline]
    pub (crate) fn callbacks(&self) -> &'static TalkClassCallbacks {
        let TalkClass(idx)  = *self;
        let callback        = LOCAL_CLASS_CALLBACKS.with(|callbacks| {
            let callbacks = callbacks.borrow();

            if idx < callbacks.len() {
                callbacks[idx]
            } else {
                None
            }
        });

        if let Some(callback) = callback {
            callback
        } else {
            self.make_local_callbacks()
        }
    }

    ///
    /// Sends a message to this class
    ///
    #[inline]
    pub fn send_message_in_context(&self, message: TalkMessage, context: &mut TalkContext) -> TalkContinuation {
        context.get_callbacks(*self).send_class_message(message)
    }

    ///
    /// Sends a message to this class
    ///
    pub fn send_message(&self, message: TalkMessage, runtime: &TalkRuntime) -> impl Future<Output=TalkValue> {
        let class                       = *self;
        let mut message                 = Some(message);
        let mut message_continuation    = None;

        runtime.run_continuation(TalkContinuation::Later(Box::new(move |talk_context, future_context| {
            // First, send the message
            if let Some(message) = message.take() {
                message_continuation = Some(class.send_message_in_context(message, talk_context));
            }

            // Then, wait for the message to complete
            match message_continuation.as_mut().unwrap() {
                TalkContinuation::Ready(value)  => Poll::Ready(value.clone()),
                TalkContinuation::Later(later)  => later(talk_context, future_context),
            }
        })))
    }

    ///
    /// Retrieves the definition for this class, or None if the definition is not of the right type
    ///
    pub fn definition<TClass>(&self) -> Option<Arc<TClass>> 
    where
        TClass: 'static + TalkClassDefinition
    {
        let class_definitions = CLASS_DEFINITIONS.lock().unwrap();

        if let Some(Some(any_defn)) = class_definitions.get(self.0) {
            any_defn.downcast_ref::<Arc<TClass>>()
                .map(|defn| Arc::clone(defn))
        } else {
            // Definition not stored/registered
            None
        }
    }

    ///
    /// Retrieves the allocator for this class in a context, or None if the definition is not of the right type
    ///
    pub fn allocator<TClass>(&self, context: &mut TalkContext) -> Option<Arc<Mutex<TClass::Allocator>>>
    where
        TClass: 'static + TalkClassDefinition
    {
        let callbacks = context.get_callbacks(*self);

        callbacks.allocator.downcast_ref::<Arc<Mutex<TClass::Allocator>>>()
            .map(|defn| Arc::clone(defn))
    }
}
