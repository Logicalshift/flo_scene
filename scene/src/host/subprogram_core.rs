use crate::host::output_sink::*;
use crate::host::process_core::*;
use crate::host::scene_core::*;
use crate::host::scene_message::*;
use crate::host::stream_id::*;
use crate::host::stream_target::*;
use crate::host::subprogram_id::*;

use futures::prelude::*;

use std::any::*;
use std::collections::*;
use std::sync::*;
use std::sync::atomic::{AtomicUsize, Ordering};

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

    /// The handle of this subprogram (index into the subprograms list)
    pub (super) subprogram_handle: usize,

    /// The source of the last message that this subprogram received via its input stream
    pub (super) last_message_source: Option<SubProgramId>,

    /// The handle of the processes that this subprogram is running on (empty if the program has finished)
    pub (super) process_ids: Vec<ProcessHandle>,

    /// The output sink cores for the outputs of this sub-program
    pub (super) outputs: HashMap<StreamId, Arc<dyn Send + Sync + Any>>,

    /// The number of outputs left after the last time that the list was purged
    pub (super) output_high_water: usize,

    /// The name of the expected input type of this program
    pub (super) expected_input_type_name: &'static str,

    /// The ID assigned to the next command that this subprogram will launch (shared with any commands launched by this program)
    pub (super) next_command_sequence: Arc<AtomicUsize>,

    /// The 'parent' program for this subprogram (this subprogram will end if this parent program ends)
    pub (super) parent_program: Option<SubProgramId>,

    /// The 'child' programs for this subprogram (these programs will also end if this program ends)
    pub (super) child_programs: HashSet<SubProgramId>,

    /// Data associated with this subprogram
    pub (super) data: HashMap<TypeId, Box<dyn 'static + Send + Any>>,
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
    /// Retrieves the output streams and output sinks for this subprogram
    ///
    pub (crate) fn output_streams<'a>(&'a self) -> impl 'a + Iterator<Item=(&'a StreamId, &'a Arc<dyn Send + Sync + Any>)> {
        self.outputs.iter()
    }

    ///
    /// Attempts to reconnect any output sinks that are marked as disconnected
    ///
    pub (crate) fn reconnect_disconnected_outputs(program_core: &Arc<Mutex<SubProgramCore>>, scene_core: &Arc<Mutex<SceneCore>>, reconnect_stream_id: &StreamId) {
        // Get the disconnected output sinks that match this
        let (output_sink_cores, program_id) = {
            let core = program_core.lock().unwrap();

            // Only the disconnected output sinks are reconnected here (sinks that are already attached are reconnected via the connectome's invalidated streams)
            let output_sink_cores = core.outputs
                .iter()
                .filter(|(output_stream_id, _)| reconnect_stream_id.message_type() == output_stream_id.message_type())
                .filter(|(output_stream_id, output_sink_core)| matches!(output_stream_id.active_target_for_output_sink(output_sink_core), Ok(StreamTarget::Any)))
                .map(|(stream_id, output_sink_core)| (stream_id.clone(), output_sink_core.clone()))
                .collect::<Vec<_>>();

            let program_id = core.id;

            (output_sink_cores, program_id)
        };

        // Reconnect the outputs to their existing targets
        let scene_updates = SceneCore::reconnect_output_sinks(scene_core, program_id, output_sink_cores);

        // Notify the subscribers of any connections that have changed
        SceneCore::send_scene_updates(scene_core, scene_updates);
    }

    ///
    /// Releases the unused output sinks if many have been allocated since this was last done
    ///
    /// (The reason for returning them here is so they can be dropped outside of the subprogram lock)
    ///
    pub (crate) fn release_stale_output_sinks(&mut self) -> Vec<Arc<dyn Send + Sync + Any>> {
        const NUM_NEW_SINKS_BEFORE_RELEASE: usize = 10;

        if self.outputs.len() > self.output_high_water + NUM_NEW_SINKS_BEFORE_RELEASE {
            self.release_all_unused_output_sinks()
        } else {
            vec![]
        }
    }

    ///
    /// Finds the output sinks in this core which are not being used by anything and returns them
    ///
    /// (The reason for returning them here is so they can be dropped outside of the subprogram lock)
    ///
    pub (crate) fn release_all_unused_output_sinks(&mut self) -> Vec<Arc<dyn Send + Sync + Any>> {
        let mut unused_output_sinks = vec![];

        // Iterate through all of the outputs stored in this core
        for stream_id in self.outputs.keys().cloned().collect::<Vec<_>>() {
            if let Some(output) = self.outputs.get(&stream_id) {
                // Remove outputs with a strong_count of 1 (ie, which are only referenced internally)
                if Arc::strong_count(output) == 1 {
                    unused_output_sinks.push(self.outputs.remove(&stream_id).unwrap());
                }
            }
        }

        // Update the 'high water' mark for the output sinks for this subprogram
        self.output_high_water = self.outputs.len();

        unused_output_sinks
    }

    ///
    /// Creates a new subprogram ID for a task launched by this program
    ///
    pub (crate) fn new_task_id(&mut self) -> SubProgramId {
        let sequence_number = self.next_command_sequence.fetch_add(1, Ordering::Relaxed);

        self.id.with_command_id(sequence_number)
    }

    ///
    /// Waits for any background processes to finish
    ///
    pub (crate) fn wait_for_background_processes<'a>(subprogram_core: &'a Arc<Mutex<SubProgramCore>>, scene_core: &'a Arc<Mutex<SceneCore>>) -> impl 'a + Send + Future<Output=()> {
        future::poll_fn(move |ctxt| {
            use futures::task::{Poll};

            // Check if the processes are all finished
            let mut scene_core  = scene_core.lock().unwrap();
            let subprogram_core = subprogram_core.lock().unwrap();

            if subprogram_core.process_ids.len() > 1 {
                // More than one process remaining for this subprogram (the first process is always the program itself, except when it's dead)
                // Reawaken this whenever a process has finished stopping
                scene_core.on_stop.push(ctxt.waker().clone());

                Poll::Pending
            } else {
                // No more background processes are waiting
                Poll::Ready(())
            }
        })
    }

    ///
    /// Closes down this subprogram (shutting down any child programs)
    ///
    pub (crate) fn closedown(subprogram_core: &Arc<Mutex<SubProgramCore>>, scene_core: &Arc<Mutex<SceneCore>>) {
        use std::mem;

        // Fetch the child programs from the core that's shutting down
        let (parent_program, child_programs, our_program_id) = {
            let mut subprogram_core = subprogram_core.lock().unwrap();

            if subprogram_core.child_programs.is_empty() {
                (subprogram_core.parent_program.take(), HashSet::new(), subprogram_core.id)
            } else {
                let mut child_programs = HashSet::new();
                mem::swap(&mut child_programs, &mut subprogram_core.child_programs);

                (subprogram_core.parent_program.take(), child_programs, subprogram_core.id)
            }
        };

        // Close down the child programs
        for child_program_id in child_programs.into_iter() {
            SceneCore::close_subprogram(scene_core, child_program_id);
        }

        // Remove this program from the parent program if there is one
        if let Some(parent_program) = parent_program {
            SceneCore::detach_child_program(scene_core, parent_program, our_program_id);
        }
    }
}
