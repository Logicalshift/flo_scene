use super::script_class::*;

use crate::allocator::*;
use crate::context::*;
use crate::class::*;
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

use futures::channel::oneshot;

use std::sync::*;

/// The 'later' class, creates values that are set later by some other asynchronous part of the program
pub static LATER_CLASS: Lazy<TalkClass> = Lazy::new(|| TalkClass::create(TalkLaterClass));

/// The 'setValue:' message
pub static TALK_MSG_SETVALUE: Lazy<TalkMessageSignatureId> = Lazy::new(|| "setValue:".into());

///
/// The `Later` class is used for values that are set elsewhere
///
/// These are most useful with streams or other asynchronous scripts. You can create a new `Later` by calling `laterValue := Later new`, and block while waiting for
/// the value by calling `laterValue value`. The value can be set by calling `laterValue setValue: x`, which will unblock the waiting script.
///
pub struct TalkLaterClass;

///
/// Data storage for the 'Later' class
///
pub struct TalkLater {
    set_value:  Option<Vec<oneshot::Sender<TalkValue>>>,
    value:      Option<TalkValue>,
}

impl TalkReleasable for TalkLater { 
    fn release_in_context(mut self, context: &TalkContext) { 
        if let Some(value) = self.value.take() {
            value.release(context);
        }
    }
}

impl TalkClassDefinition for TalkLaterClass {
    /// The type of the data stored by an object of this class (this particular class is never instantiated)
    type Data = TalkLater;

    /// The allocator is used to manage the memory of this class within a context
    type Allocator = TalkStandardAllocator<TalkLater>;

    ///
    /// Creates the allocator for this class
    ///
    fn create_allocator(&self) -> Self::Allocator {
        Self::Allocator::empty()
    }

    ///
    /// Sends a message to the class object itself
    ///
    fn send_class_message(&self, message_id: TalkMessageSignatureId, _args: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, class_id: TalkClass, allocator: &Arc<Mutex<Self::Allocator>>) -> TalkContinuation<'static> {
        if message_id == *TALK_MSG_NEW {

            // Create a new 'Later' data object
            let new_value           = TalkLater {
                set_value:  Some(vec![]),
                value:      None,
            };

            // Store in the allocator
            let later_data_handle   = allocator.lock().unwrap().store(new_value);
            let later_reference     = TalkReference(class_id, later_data_handle);

            later_reference.into()

        } else if message_id == *TALK_MSG_SUBCLASS {

            TalkScriptClassClass::create_subclass(class_id, vec![*TALK_MSG_NEW])

        } else {
            TalkError::MessageNotSupported(message_id).into()
        }
    }

    ///
    /// Sends a message to an instance of this class
    ///
    fn send_instance_message(&self, message_id: TalkMessageSignatureId, args: TalkOwned<SmallVec<[TalkValue; 4]>, &'_ TalkContext>, reference: TalkReference, allocator: &Mutex<Self::Allocator>) -> TalkContinuation<'static> {
        if message_id == *TALK_MSG_VALUE {

            // Fetch the 'later' object
            let mut allocator   = allocator.lock().unwrap();
            let mut later       = allocator.retrieve(reference.1);

            if let Some(value) = later.value.clone() {
                // Value has already been generated, just re-use it
                TalkContinuation::soon(move |context| {
                    value.retain(context);
                    value.into()
                })
            } else if let Some(senders) = &mut later.set_value {
                // Wait for something to generate the value
                let (sender, receiver) = oneshot::channel();
                senders.push(sender);

                TalkContinuation::future_value(async move {
                    receiver.await.ok().unwrap_or(TalkValue::Nil)
                })
            } else {
                // Shouldn't ever end up in this state
                TalkError::Busy.into()
            }

        } else if message_id == *TALK_MSG_SETVALUE {

            // Fetch the 'later' object
            let mut allocator   = allocator.lock().unwrap();
            let mut later       = allocator.retrieve(reference.1);

            // Take the senders if no value has been sent yet
            let senders         = later.set_value.take();
            let senders         = if let Some(senders) = senders { senders } else { return TalkError::AlreadySentValue.into(); };

            // Argument 0 is the value to set
            let mut args    = args;
            let new_value   = args[0].take();
            later.value     = Some(new_value.clone());

            // Retain once more per sender, then send the results
            TalkContinuation::soon(move |context| {
                for _ in senders.iter() {
                    new_value.retain(context);
                }

                TalkContinuation::future_value(async move {
                    for sender in senders {
                        sender.send(new_value.clone()).ok();
                    }

                    TalkValue::Nil
                })
            })

        } else {

            TalkError::MessageNotSupported(message_id).into()
        }
    }
}
