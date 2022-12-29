use crate::class::*;
use crate::context::*;
use crate::continuation::*;
use crate::dispatch_table::*;
use crate::error::*;
use crate::message::*;
use crate::reference::*;
use crate::releasable::*;
use crate::value::*;
use crate::value_messages::*;

use smallvec::*;
use once_cell::sync::{Lazy};

use std::sync::*;

// TODO: we could store a pool of classes that can be used to create custom classes in the allocator and make this where new classes are created

static CLASS_CLASS: Lazy<TalkClass> = Lazy::new(|| TalkClass::create(TalkClassClass));

///
/// The class representing a FloTalk class
///
struct TalkClassClass;

///
/// Allocator for the talk class class (the data handle part of the reference is always a TalkClass; classes cannot be freed)
///
struct TalkClassClassAllocator {
    nothing: ()
}

impl TalkClassDefinition for TalkClassClass {
    type Data       = ();
    type Allocator  = TalkClassClassAllocator;

    fn create_allocator(&self) -> Self::Allocator {
        TalkClassClassAllocator {
            nothing: ()
        }
    }

    fn send_class_message(&self, message_id: TalkMessageSignatureId, _args: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, _class_id: TalkClass, _allocator: &Arc<Mutex<Self::Allocator>>) -> TalkContinuation<'static> {
        TalkError::MessageNotSupported(message_id).into()
    }

    fn send_instance_message(&self, message_id: TalkMessageSignatureId, args: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, reference: TalkReference, _allocator: &Mutex<Self::Allocator>) -> TalkContinuation<'static> {
        // The data handle is the TalkClass ID
        let talk_class  = TalkClass(reference.1.0);

        // Leak the args as we're going to re-send the message
        let args        = args.leak();

        // Send a class message to this class
        TalkContinuation::Soon(Box::new(move |talk_context| {
            if args.len() == 0 {
                talk_class.send_message_in_context(TalkMessage::Unary(message_id), talk_context)
            } else {
                talk_class.send_message_in_context(TalkMessage::WithArguments(message_id, args), talk_context)
            }
        }))
    }

    fn default_instance_dispatch_table(&self) -> TalkMessageDispatchTable<TalkReference> { 
        TalkMessageDispatchTable::empty().with_mapped_messages_from(&*TALK_DISPATCH_ANY, |v| TalkValue::Reference(v))
    }

    fn default_class_dispatch_table(&self) -> TalkMessageDispatchTable<()> {
        TalkMessageDispatchTable::empty() 
    }
}

///
/// A class allocator is used to manage the memory of a class
///
impl TalkClassAllocator for TalkClassClassAllocator {
    /// The type of data stored for this class
    type Data = ();

    fn retrieve<'a>(&'a mut self, _handle: TalkDataHandle) -> &'a mut Self::Data { &mut self.nothing }

    fn retain(_allocator: &Arc<Mutex<Self>>, _handle: TalkDataHandle, _context: &TalkContext) { /* Classes don't count references */ }

    fn release(_allocator: &Arc<Mutex<Self>>, _handle: TalkDataHandle, _context: &TalkContext) { /* Class objects cannot be freed */ }
}

impl TalkClass {
    ///
    /// Retrieves the data object for this TalkClass
    ///
    pub fn class_object_in_context(&self, context: &mut TalkContext) -> TalkReference {
        context.get_callbacks_mut(*CLASS_CLASS);
        TalkReference(*CLASS_CLASS, TalkDataHandle(self.0))
    }

    ///
    /// Creates a continuation that will generate the class object for this class
    ///
    pub fn class_object(&self) -> TalkContinuation<'static> {
        let ourselves = *self;
        TalkContinuation::Soon(Box::new(move |talk_context| ourselves.class_object_in_context(talk_context).into()))
    }
}
