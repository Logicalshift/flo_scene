use crate::error::*;
use crate::input_stream::*;
use crate::scene_core::*;
use crate::scene_message::*;
use crate::stream_id::*;
use crate::subprogram_id::*;

use futures::prelude::*;
use futures::{pin_mut};
use futures::future::{poll_fn, BoxFuture};
use futures::task::{Poll};
use once_cell::sync::{Lazy};

use std::any::*;
use std::collections::{HashMap};
use std::sync::*;
use std::sync::atomic::{AtomicUsize, Ordering};

type CreateInputStreamFn = Box<dyn Send + Sync + Fn(SubProgramId, Arc<dyn Send + Sync + Any>) -> Result<(BoxFuture<'static, ()>, Arc<dyn Send + Sync + Any>), ConnectionError>>;
type StreamIdForTargetFn = Box<dyn Send + Sync + Fn(Option<SubProgramId>) -> StreamId>;

static NEXT_FILTER_HANDLE:      AtomicUsize                                                 = AtomicUsize::new(0);
static CREATE_INPUT_STREAM:     Lazy<RwLock<HashMap<FilterHandle, CreateInputStreamFn>>>    = Lazy::new(|| RwLock::new(HashMap::new()));
static STREAM_ID_FOR_TARGET:    Lazy<RwLock<HashMap<FilterHandle, StreamIdForTargetFn>>>    = Lazy::new(|| RwLock::new(HashMap::new()));

///
/// A filter is a way to convert from a stream of one message type to another, and a filter
/// handle references a predefined filter.
///
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct FilterHandle(usize);

impl FilterHandle {
    ///
    /// Returns a filter handle for a filtering function
    ///
    /// A filter can be used to convert between an output of one subprogram and the input of another when they are different types. This makes it
    /// possible to connect subprograms without needing an intermediate program that performs the conversion.
    ///
    pub fn for_filter<TSourceMessage, TTargetStream>(filter: impl 'static + Send + Sync + Fn(InputStream<TSourceMessage>) -> TTargetStream) -> FilterHandle
    where
        TSourceMessage:         'static + Unpin + SceneMessage,
        TTargetStream:          'static + Send + Stream,
        TTargetStream::Item:    'static + Unpin + SceneMessage,
    {
        use std::mem;

        // Create a new filter handle
        let handle = NEXT_FILTER_HANDLE.fetch_add(1, Ordering::Relaxed);
        let handle = FilterHandle(handle);

        // Create a reference to the filter so we can share it in more than one function if needed
        let filter = Arc::new(filter);

        // Generate the filter functions for this filter
        let mut create_input_stream = CREATE_INPUT_STREAM.write().unwrap();
        create_input_stream.insert(handle, Box::new(move |sending_program, target_input_core| {
            // Downcast the source and target to the expected types
            let target_input_core   = target_input_core.downcast::<Mutex<InputStreamCore<TTargetStream::Item>>>().or(Err(ConnectionError::FilterOutputDoesNotMatch))?;
            let buffer_size         = target_input_core.lock().unwrap().num_slots();

            let source_input_stream = InputStream::<TSourceMessage>::new(sending_program, buffer_size);
            let target_input_core   = Arc::downgrade(&target_input_core);

            // The source core is what should be attached to the output sink here
            let source_core = source_input_stream.core();

            // Create a future for reading from the source stream and sending to the target stream
            let filter_stream = filter(source_input_stream);

            let run_filter = async move {
                // Read from the filtered stream
                pin_mut!(filter_stream);
                while let Some(item) = filter_stream.next().await {
                    // Write to the core
                    let mut item = Some(item);

                    poll_fn(|context| {
                        // Send the item to the core
                        let (pending_item, waker) = {
                            if let Some(target_input_core) = target_input_core.upgrade() {
                                let mut input_core = target_input_core.lock().unwrap();

                                if let Some(item_to_send) = item.take() {
                                    match input_core.send(sending_program, item_to_send) {
                                        Ok(waker)   => (None, waker),
                                        Err(item)   => {
                                            // Core has no slots, so wait until it does
                                            input_core.wake_when_slots_available(context);
                                            (Some(item), None)
                                        },
                                    }
                                } else {
                                    // Somehow the item has already been sent
                                    (None, None)
                                }
                            } else {
                                // Target core has been released, so we can no longer send any messages
                                (None, None)
                            }
                        };

                        // If the item failed to send, keep it for the next attempt
                        item = pending_item;

                        // Now the core is unlocked, we can wake it up if necessary as it has a new item
                        if let Some(waker) = waker {
                            waker.wake();
                        }

                        // Keep waiting if the input is not sent
                        if item.is_some() {
                            Poll::Pending
                        } else {
                            Poll::Ready(())
                        }
                    }).await;
                }
            };

            Ok((run_filter.boxed(), source_core))
        }));

        mem::drop(create_input_stream);

        let mut stream_id_for_target = STREAM_ID_FOR_TARGET.write().unwrap();
        stream_id_for_target.insert(handle, Box::new(|maybe_target_program| {
            if let Some(target_program) = maybe_target_program {
                StreamId::for_target::<TTargetStream::Item>(target_program)
            } else {
                StreamId::with_message_type::<TTargetStream::Item>()
            }
        }));

        mem::drop(stream_id_for_target);

        handle
    }

    ///
    /// Creates an input stream core which will filter its results using this filter and send them to a target core
    ///
    /// This is an input stream that accepts the 'source' type of the filter, and sends its results to the target core, as if they came 
    /// from the specified sending program. The core returned by this function should be closed when disconnected, or it will leave
    /// behind a process in the scene that can never run.
    ///
    pub (crate) fn create_input_stream_core(&self, scene_core: &Arc<Mutex<SceneCore>>, sending_program: SubProgramId, target_input_core: Arc<dyn Send + Sync + Any>) -> Result<Arc<dyn Send + Sync + Any>, ConnectionError> {
        // Create a future that will run the filter
        let (send_future, filtering_input_core) = {
            let create_input_stream  = CREATE_INPUT_STREAM.read().unwrap();
            let create_future   = create_input_stream.get(self).ok_or(ConnectionError::FilterHandleNotFound)?;

            create_future(sending_program, target_input_core)
        }?;

        // Start it as a process in the core
        let (_process_handle, waker) = {
            let mut scene_core = scene_core.lock().unwrap();

            scene_core.start_process(send_future)
        };

        // Wake up a thread to run the new future if needed
        if let Some(waker) = waker {
            waker.wake();
        }

        Ok(filtering_input_core)
    }

    ///
    /// Creates a stream ID for the output of a filter and a target program
    ///
    pub (crate) fn target_stream_id(&self, target_program: SubProgramId) -> Result<StreamId, ConnectionError> {
        let stream_id_for_target    = STREAM_ID_FOR_TARGET.read().unwrap();
        let create_stream_id        = stream_id_for_target.get(self).ok_or(ConnectionError::FilterHandleNotFound)?;

        Ok(create_stream_id(Some(target_program)))
    }
}
