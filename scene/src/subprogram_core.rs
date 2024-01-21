use crate::output_sink::*;
use crate::scene_core::*;
use crate::scene_message::*;
use crate::stream_id::*;
use crate::subprogram_id::*;

use futures::task::{Waker};

use std::any::*;
use std::collections::*;
use std::sync::*;

///
/// Data that's stored for an individual program.
///
/// Note that the scene core must be locked before the subprogram core, if the scene core needs to be locked.
///
pub (crate) struct SubProgramCore {
    /// The stream ID of the input stream to this subprogram
    pub (super) input_stream_id: StreamId,

    /// The ID of this program
    pub (super) id: SubProgramId,

    /// The handle of the process that this subprogram is running on (or None if the program has finished)
    pub (super) process_id: Option<ProcessHandle>,

    /// The output sink targets for this sub-program
    pub (super) outputs: HashMap<StreamId, Arc<dyn Send + Sync + Any>>,

    /// The name of the expected input type of this program
    pub (super) expected_input_type_name: &'static str,
}

impl SubProgramCore {
    ///
    /// Retrieves the ID of this subprogram
    ///
    pub (crate) fn program_id(&self) -> &SubProgramId {
        &self.id
    }

    ///
    /// Retrieves the ID of the input stream for this subprogram
    ///
    pub (crate) fn input_stream_id(&self) -> StreamId {
        self.input_stream_id.clone()
    }

    ///
    /// Returns the existing output core for a stream ID, if it exists in this subprogram
    ///
    pub (crate) fn output_core<TMessageType>(&self, id: &StreamId) -> Option<Arc<Mutex<OutputSinkCore<TMessageType>>>> 
    where
        TMessageType: 'static + SceneMessage,
    {
        // Fetch the existing target and clone it
        let existing_target = self.outputs.get(id)?;
        let existing_target = Arc::clone(existing_target);

        // Convert to the appropriate output type
        existing_target.downcast::<Mutex<OutputSinkCore<TMessageType>>>().ok()
    }

    ///
    /// Tries to set the output target for a stream ID. Returns Ok() if the new output target was defined or Err() if there's already a valid output for this stream
    ///
    /// Panics if the stream ID doesn't match the message type and the stream already exists.
    ///
    #[allow(clippy::type_complexity)]   // Doesn't really have anything nameable plus really not that bad
    pub (crate) fn try_create_output_target<TMessageType>(&mut self, id: &StreamId, new_output_target: OutputSinkTarget<TMessageType>) 
        -> Result<Arc<Mutex<OutputSinkCore<TMessageType>>>, Arc<Mutex<OutputSinkCore<TMessageType>>>>
    where
        TMessageType: 'static + SceneMessage,
    {
        let existing_output_core = self.outputs.get(id);
        if let Some(existing_output_core) = existing_output_core {
            // Return the already existing target
            let existing_output_core = Arc::clone(existing_output_core);
            let existing_output_core = existing_output_core.downcast::<Mutex<OutputSinkCore<TMessageType>>>().unwrap();

            Err(existing_output_core)
        } else {
            // Store a new target in the outputs
            let new_output_core     = OutputSinkCore::new(new_output_target);
            let new_output_core     = Arc::new(Mutex::new(new_output_core));
            let cloned_output_core  = Arc::clone(&new_output_core);
            self.outputs.insert(id.clone(), cloned_output_core);

            // Use the new target for the output stream
            Ok(new_output_core)
        }
    }

    ///
    /// Returns true if this program has an output for a particular stream
    ///
    pub (crate) fn has_output_sink(&mut self, stream_id: &StreamId) -> bool {
        self.outputs.contains_key(stream_id)
    }

    ///
    /// Connects all of the streams that matches a particular stream ID to a new target
    ///
    pub (crate) fn reconnect_output_sinks(&mut self, target_input: &Arc<dyn Send + Sync + Any>, stream_id: &StreamId, close_when_dropped: bool) -> Option<Waker> {
        if let Some(output_sink) = self.outputs.get_mut(stream_id) {
            // This stream has an output matching the input (the stream types should always match)
            stream_id.connect_output_to_input(output_sink, target_input, close_when_dropped).expect("Input and output types do not match")
        } else {
            None
        }
    }

    ///
    /// Disconnects an output sink for a particular stream
    ///
    pub (crate) fn disconnect_output_sink(&mut self, stream_id: &StreamId) -> Option<Waker> {
        if let Some(output_sink) = self.outputs.get_mut(stream_id) {
            // This stream has an output matching the stream
            stream_id.disconnect_output(output_sink).expect("Stream type does not match")
        } else {
            None
        }
    }

    ///
    /// Discards any output sent to an output stream
    ///
    pub (crate) fn discard_output_from(&mut self, stream_id: &StreamId) -> Option<Waker> {
        if let Some(output_sink) = self.outputs.get_mut(stream_id) {
            // This stream has an output matching the stream
            stream_id.connect_output_to_discard(output_sink).expect("Stream type does not match")
        } else {
            None
        }
    }
}
