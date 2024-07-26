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

#[cfg(feature="serde_support")] use serde::*;

type CreateInputStreamFn = Box<dyn Send + Sync + Fn(SubProgramId, Arc<dyn Send + Sync + Any>) -> Result<(BoxFuture<'static, ()>, Arc<dyn Send + Sync + Any>), ConnectionError>>;
type StreamIdForTargetFn = Box<dyn Send + Sync + Fn(Option<SubProgramId>) -> StreamId>;

static NEXT_FILTER_HANDLE:      AtomicUsize                                                 = AtomicUsize::new(0);

/// Creates an input stream core that will send the filter result to a target input core (which must match the types in the filter)
static CREATE_INPUT_STREAM:     Lazy<RwLock<HashMap<FilterHandle, CreateInputStreamFn>>>    = Lazy::new(|| RwLock::new(HashMap::new()));

/// Function that returns the stream ID of a target subprogram
static STREAM_ID_FOR_TARGET:    Lazy<RwLock<HashMap<FilterHandle, StreamIdForTargetFn>>>    = Lazy::new(|| RwLock::new(HashMap::new()));

/// Maps filter handles to the stream ID of the source
static SOURCE_STREAM_ID:        Lazy<RwLock<HashMap<FilterHandle, StreamId>>>               = Lazy::new(|| RwLock::new(HashMap::new()));

// TODO: filter handles are shareable out of necessity, so we can send stream sources and targets to other programs, but they currently will be invalid after being sent

///
/// A filter is a way to convert from a stream of one message type to another, and a filter
/// handle references a predefined filter.
///
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
#[cfg_attr(feature="serde_support", derive(Serialize, Deserialize))]
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
            let scene_core          = target_input_core.lock().unwrap().scene_core().ok_or(ConnectionError::TargetNotInScene)?;

            let source_input_stream = InputStream::<TSourceMessage>::new(sending_program, &scene_core, buffer_size);
            source_input_stream.allow_thread_stealing(true);
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

                    let poll_result = poll_fn(|context| {
                        // Send the item to the core
                        let (pending_item, waker) = {
                            if let Some(target_input_core) = target_input_core.upgrade() {
                                let mut input_core = target_input_core.lock().unwrap();

                                if let Some(item_to_send) = item.take() {
                                    match input_core.send(sending_program, item_to_send) {
                                        Ok(waker)   => (None, waker),
                                        Err(item)   => {
                                            if input_core.is_closed() {
                                                // Cannot send any more data as the core is closed
                                                return Poll::Ready(Err(()));
                                            } else {
                                                // Core has no slots, so wait until it does
                                                input_core.wake_when_slots_available(context);
                                                (Some(item), None)
                                            }
                                        },
                                    }
                                } else {
                                    // Somehow the item has already been sent
                                    (None, None)
                                }
                            } else {
                                // Target core has been released, so we can no longer send any messages
                                return Poll::Ready(Err(()));
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
                            Poll::Ready(Ok(()))
                        }
                    }).await;

                    // Stop waiting for input if the target input stream errors out
                    if poll_result.is_err() {
                        break;
                    }
                }
            };

            Ok((run_filter.boxed(), source_core))
        }));

        mem::drop(create_input_stream);

        // Store the stream ID functions
        let mut stream_id_for_target = STREAM_ID_FOR_TARGET.write().unwrap();
        stream_id_for_target.insert(handle, Box::new(|maybe_target_program| {
            if let Some(target_program) = maybe_target_program {
                StreamId::with_message_type::<TTargetStream::Item>().for_target(target_program)
            } else {
                StreamId::with_message_type::<TTargetStream::Item>()
            }
        }));

        mem::drop(stream_id_for_target);

        SOURCE_STREAM_ID.write().unwrap().insert(handle, StreamId::with_message_type::<TSourceMessage>());

        handle
    }

    ///
    /// Creates a filter that converts between two message types that implements `From`
    ///
    /// This will cache the filter handle for specific message types so this won't allocate additional filters every time it's called
    ///
    pub fn conversion_filter<TSourceMessage, TTargetMessage>() -> FilterHandle
    where
        TSourceMessage: 'static + SceneMessage + Into<TTargetMessage>,
        TTargetMessage: 'static + SceneMessage,
    {
        use std::mem;
        static EXISTING_FILTERS: Lazy<RwLock<HashMap<(TypeId, TypeId), FilterHandle>>> = Lazy::new(|| RwLock::new(HashMap::new()));

        // We cache the filter handle so that if more than one thing wants the same conversion we don't allocate another one
        let conversion_type = (TypeId::of::<TSourceMessage>(), TypeId::of::<TTargetMessage>());

        // Try to fetch the existing filter if there is one
        let existing_filters = EXISTING_FILTERS.read().unwrap();
        if let Some(existing) = existing_filters.get(&conversion_type) {
            *existing
        } else {
            // Create a new filter and cache it
            mem::drop(existing_filters);
            let mut existing_filters = EXISTING_FILTERS.write().unwrap();

            let new_filter = Self::for_filter(|input| input.map(|source_message: TSourceMessage| source_message.into()));
            existing_filters.insert(conversion_type, new_filter);

            new_filter
        }
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
    /// Chains this filter with a following filter.
    ///
    /// This generates an input stream that first applies this filter, and then sends its results through another filter. `next_filter` must have an input type
    /// that matches the output type of this filter.
    ///
    pub (crate) fn chain_filters(&self, scene_core: &Arc<Mutex<SceneCore>>, sending_program: SubProgramId, next_filter: FilterHandle, target_input_core: Arc<dyn Send + Sync + Any>) -> Result<Arc<dyn Send + Sync + Any>, ConnectionError> {
        // Send to the target from the filter that follows this one
        let following_filter = next_filter.create_input_stream_core(scene_core, sending_program, target_input_core)?;

        // Receive from the source and send to the target
        self.create_input_stream_core(scene_core, sending_program, following_filter)
    }

    ///
    /// Returns the stream ID for the source of this filter
    ///
    pub (crate) fn source_stream_id_any(&self) -> Result<StreamId, ConnectionError> {
        let source_stream_id = SOURCE_STREAM_ID.read().unwrap();
        let source_stream_id = source_stream_id.get(self).ok_or(ConnectionError::FilterHandleNotFound)?;

        Ok(source_stream_id.clone())
    }

    ///
    /// Returns the stream ID for the target of this filter
    ///
    pub (crate) fn target_stream_id_any(&self) -> Result<StreamId, ConnectionError> {
        let stream_id_for_target    = STREAM_ID_FOR_TARGET.read().unwrap();
        let create_stream_id        = stream_id_for_target.get(self).ok_or(ConnectionError::FilterHandleNotFound)?;

        Ok(create_stream_id(None))
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
