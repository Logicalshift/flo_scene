use crate::error::*;
use crate::input_stream::*;
use crate::scene_core::*;
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

static NEXT_FILTER_HANDLE: AtomicUsize = AtomicUsize::new(0);
static CONNECT_INPUTS: Lazy<RwLock<HashMap<FilterHandle, Box<dyn Send + Sync + Fn(SubProgramId, Box<dyn Send + Sync + Any>, Arc<dyn Send + Sync + Any>) -> Result<BoxFuture<'static, ()>, ConnectionError>>>>> = Lazy::new(|| RwLock::new(HashMap::new()));
static STREAM_ID_FOR_TARGET: Lazy<RwLock<HashMap<FilterHandle, Box<dyn Send + Sync + Fn(Option<SubProgramId>) -> StreamId>>>> = Lazy::new(|| RwLock::new(HashMap::new()));

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
        TSourceMessage:         'static + Unpin + Send + Sync,
        TTargetStream:          'static + Send + Stream,
        TTargetStream::Item:    'static + Unpin + Send + Sync,
    {
        use std::mem;

        // Create a new filter handle
        let handle = NEXT_FILTER_HANDLE.fetch_add(1, Ordering::Relaxed);
        let handle = FilterHandle(handle);

        // Create a reference to the filter so we can share it in more than one function if needed
        let filter = Arc::new(filter);

        // Generate the filter functions for this filter
        let mut connect_inputs = CONNECT_INPUTS.write().unwrap();
        connect_inputs.insert(handle, Box::new(move |sending_program, source_input_stream, target_input_core| {
            // Downcast the source and target to the expected types
            let source_input_stream = source_input_stream.downcast::<InputStream<TSourceMessage>>().or(Err(ConnectionError::FilterInputDoesNotMatch))?;
            let target_input_core   = target_input_core.downcast::<Mutex<InputStreamCore<TTargetStream::Item>>>().or(Err(ConnectionError::FilterOutputDoesNotMatch))?;
            let target_input_core   = Arc::downgrade(&target_input_core);

            // Extract the input stream from its box
            let source_input_stream = *source_input_stream;

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

            Ok(run_filter.boxed())
        }));

        mem::drop(connect_inputs);

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
    /// Connects a filter to a target core
    ///
    /// The source is always an InputStream of the soruce type
    ///
    pub (crate) fn connect_inputs(&self, scene_core: &Arc<Mutex<SceneCore>>, sending_program: SubProgramId, source_input_stream: Box<dyn Send + Sync + Any>, target_input_core: Arc<dyn Send + Sync + Any>) -> Result<(), ConnectionError> {
        // Create a future that will run the filter
        let send_future = {
            let connect_inputs  = CONNECT_INPUTS.read().unwrap();
            let create_future   = connect_inputs.get(self).ok_or(ConnectionError::FilterHandleNotFound)?;

            create_future(sending_program, source_input_stream, target_input_core)
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

        Ok(())
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
