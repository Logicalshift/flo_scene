use crate::connect_result::*;
use crate::error::*;
use crate::filter::*;
use crate::output_sink::*;
use crate::input_stream::*;
use crate::process_core::*;
use crate::programs::*;
use crate::scene::*;
use crate::scene_message::*;
use crate::stream_id::*;
use crate::stream_source::*;
use crate::stream_target::*;
use crate::subprogram_core::*;
use crate::subprogram_id::*;
use crate::thread_stealer::*;

use futures::prelude::*;
use futures::future::{poll_fn};
use futures::task::{Poll, Waker, Context, waker, ArcWake};
use futures::channel::mpsc;

use std::any::*;
use std::collections::*;
use std::sync::*;
use std::sync::atomic::{AtomicUsize};

///
/// Used to wake up anything polling a scene core when a subprogram is ready
///
pub (crate) struct SceneCoreWaker {
    /// The core that should be woken when this subprogram is ready to run
    core: Weak<Mutex<SceneCore>>,

    /// The subprogram that is woken by this waker
    process_id: usize,
}

///
/// The scene core is used to store the shared state for all scenes
///
pub (crate) struct SceneCore {
    /// The sub-programs that are active in this scene
    sub_programs: Vec<Option<Arc<Mutex<SubProgramCore>>>>,

    /// The message types where the 'initialise' routine has been called
    initialised_message_types: HashSet<TypeId>,

    /// The input stream cores for each sub-program, along with their stream type
    sub_program_inputs: Vec<Option<(StreamId, Arc<dyn Send + Sync + Any>, SubProgramId)>>,

    /// The next free sub-program
    next_subprogram: usize,

    /// Maps subprogram IDs to indexes in the subprogram list
    program_indexes: HashMap<SubProgramId, usize>,

    /// The futures that are running in this core
    processes: Vec<Option<SceneProcess>>,

    /// The next free process in this core
    next_process: usize,

    /// The processes that have been woken up since the core was last polled
    awake_processes: VecDeque<usize>,

    /// Wakers for the futures that are being used to run the scene (can be multiple if the scene is scheduled across a thread pool)
    thread_wakers: Vec<Option<Waker>>,

    /// The connections to assign between programs. More specific sources override less specific sources.
    connections: HashMap<(StreamSource, StreamId), StreamTarget>,

    /// Filters that can convert between a output stream type and an input stream type
    filter_conversions: HashMap<(StreamId, StreamId), FilterHandle>,

    /// Maps source streams to target streams where the source stream should be filtered to the target stream before connecting (this 
    /// is used for streams with an 'Any' target where there's no existing connection)
    filtered_targets: HashMap<StreamId, Vec<StreamId>>,

    /// True if this scene is stopped and shouldn't be run any more
    stopped: bool,

    /// True if the next time the scene becomes entirely idle, we should notify one of the waiting streams
    notify_when_idle: bool,

    /// Streams to send on when the input streams and core all become idle (borrowed when we're waiting for them to send)
    when_idle: Vec<Option<mpsc::Sender<()>>>,

    /// An output core where status updates are sent
    updates: Option<(SubProgramId, Arc<Mutex<OutputSinkCore<SceneUpdate>>>)>,
}

impl SceneCore {
    ///
    /// Creates an empty scene core
    ///
    pub fn new() -> SceneCore {
        SceneCore {
            sub_programs:               vec![],
            sub_program_inputs:         vec![],
            initialised_message_types:  HashSet::new(),
            next_subprogram:            0,
            processes:                  vec![],
            next_process:               0,
            program_indexes:            HashMap::new(),
            awake_processes:            VecDeque::new(),
            connections:                HashMap::new(),
            filter_conversions:         HashMap::new(),
            filtered_targets:           HashMap::new(),
            thread_wakers:              vec![],
            stopped:                    false,
            notify_when_idle:           false,
            when_idle:                  vec![],
            updates:                    None,
        }
    }

    ///
    /// If a message type has not been initialised in a core, calls the initialisation function
    ///
    #[inline]
    pub (crate) fn initialise_message_type(core: &Arc<Mutex<SceneCore>>, message_type: StreamId) {
        // If the message type is not yet initialised, mark it as such and then call the initialisation function
        // TODO: if there are multiple threads dealing with the scene, the message type might be used 'uninitialised' for a brief while on other threads
        let type_id             = message_type.message_type();
        let needs_initialising  = {
            let mut core = core.lock().unwrap();

            if !core.initialised_message_types.contains(&type_id) {
                // Not initialised
                core.initialised_message_types.insert(type_id);

                true
            } else {
                // Already initialised
                false
            }
        };

        if needs_initialising {
            // Create a fake scene object for the 'initialise' routine (same core)
            let fake_scene = Scene::with_core(core);

            // Initialise the message type
            message_type.initialise_in_scene(&fake_scene).ok();
        }
    }

    ///
    /// Adds a program to the list being run by this scene
    ///
    pub fn start_subprogram<TMessage>(scene_core: &Arc<Mutex<SceneCore>>, program_id: SubProgramId, program: impl 'static + Send + Future<Output=()>, input_core: Arc<Mutex<InputStreamCore<TMessage>>>) -> Arc<Mutex<SubProgramCore>>
    where
        TMessage: 'static + SceneMessage,
    {
        use std::mem;

        Self::initialise_message_type(scene_core, StreamId::with_message_type::<TMessage>());

        let (subprogram, waker) = {
            let start_core      = Arc::downgrade(scene_core);
            let process_core    = Arc::downgrade(scene_core);
            let mut core        = scene_core.lock().unwrap();

            // It's an error if something tries to start an extra copy of an existing program without stopping the original first
            if let Some(existing_index) = core.program_indexes.get(&program_id) {
                if let Some(Some(_)) = core.sub_programs.get(*existing_index) {
                    // TODO: this should be a 'soft' error instead of a panic, or alternatively we can close the streams of the existing program and replace it with the new one
                    panic!("Cannot start two copies of the same program in the same scene: not starting an extra copy of {:?}", program_id);
                }
            }

            // next_subprogram should always indicate the handle we'll use for the new program (it should be either a None entry in the list or sub_programs.len())
            let handle = core.next_subprogram;

            // Create a place to send updates on the program's progress
            let update_sink = core.updates.as_ref().map(|(pid, sink_core)| OutputSink::attach(*pid, Arc::clone(sink_core), scene_core));

            // Start a process to run this subprogram
            let (process_handle, waker) = core.start_process(async move {
                // Notify that the program is starting
                if let Some(core) = start_core.upgrade() {
                    // We use a background process to start because we might be blocking the program that reads the updates here
                    SceneCore::send_scene_updates(&core, vec![SceneUpdate::Started(program_id, StreamId::with_message_type::<TMessage>())]);
                }
                mem::drop(start_core);

                // Wait for the program to run
                program.await;

                // Notify that the program has finished
                if let Some(mut update_sink) = update_sink {
                    update_sink.send(SceneUpdate::Stopped(program_id)).await.ok();
                }

                // Close down the subprogram before finishing
                if let Some(process_core) = process_core.upgrade() {
                    let mut core = process_core.lock().unwrap();

                    // Take the subprogram and input core out of the scene
                    let old_sub_program     = core.sub_programs[handle].take();
                    let old_input_core      = core.sub_program_inputs[handle].take();
                    core.next_subprogram    = core.next_subprogram.min(handle);

                    // Drop in order: first release the core lock, then drop the subprograms (which may re-take it)
                    mem::drop(core);

                    if let Some(old_sub_program) = &old_sub_program {
                        old_sub_program.lock().unwrap().process_id = None;
                    }

                    mem::drop(old_input_core);
                    mem::drop(old_sub_program);

                    // Core might be idle now the program has finished
                    SceneCore::check_if_idle(&process_core);
                }
            });

            // Create the sub-program data
            let subprogram = SubProgramCore {
                id:                         program_id,
                process_id:                 Some(process_handle),
                last_message_source:        None,
                input_stream_id:            StreamId::with_message_type::<TMessage>(),
                outputs:                    HashMap::new(),
                output_high_water:          0,
                expected_input_type_name:   type_name::<TMessage>(),
                next_command_sequence:      Arc::new(AtomicUsize::new(0)),
            };

            // Allocate space for the program
            while core.sub_programs.len() <= handle {
                core.sub_programs.push(None);
                core.sub_program_inputs.push(None);
            }
            debug_assert!(core.sub_programs[handle].is_none());

            // Store the program details
            let subprogram                  = Arc::new(Mutex::new(subprogram));
            core.sub_programs[handle]       = Some(Arc::clone(&subprogram));
            core.sub_program_inputs[handle] = Some((StreamId::with_message_type::<TMessage>(), input_core, program_id));
            core.program_indexes.insert(program_id, handle);

            // Update the 'next_subprogram' value to an empty slot
            while core.next_subprogram < core.sub_programs.len() && core.sub_programs[core.next_subprogram].is_some() {
                core.next_subprogram += 1;
            }

            (subprogram, waker)
        };

        // Safe to wake the waker once the core lock is released
        if let Some(waker) = waker {
            waker.wake();
        }

        // If there are any pending connections that can be connected to this subprogram, reconnect them here
        Self::reconnect_subprogram(scene_core, program_id);

        // Result is the subprogram
        subprogram
    }

    ///
    /// Creates a 'stream update' input stream that is independent of any running program
    ///
    /// This can be read from as a secondary input stream for the control program
    ///
    pub (crate) fn send_updates_to_stream(core: &Arc<Mutex<SceneCore>>, fake_program_id: SubProgramId) -> InputStream<SceneUpdate> {
        // Create a new input stream with a fake program ID
        let input_stream    = InputStream::new(fake_program_id, core, 0);
        let output_stream   = OutputSinkCore::new(OutputSinkTarget::Input(Arc::downgrade(&input_stream.core())));
        let output_stream   = Arc::new(Mutex::new(output_stream));

        // Set the output to send to this stream
        core.lock().unwrap().set_update_core(fake_program_id, output_stream);

        // Return this stream so it can be read from
        input_stream
    }

    ///
    /// Retrieves the InputStreamCore for a particular stream target (an error if the target either doesn't exist or does not accept this input stream type)
    ///
    pub (crate) fn get_target_input(&mut self, target: &SubProgramId, stream_id: &StreamId) -> Result<Arc<dyn Send + Sync + Any>, ConnectionError> {
        // Fetch the sub-program handle (or return an error if it doesn't exist)
        let expected_message_type   = stream_id.input_stream_core_type();
        let handle                  = *self.program_indexes.get(target).ok_or(ConnectionError::TargetNotInScene)?;

        // The message type must match the expected type
        let target_input = self.sub_program_inputs[handle].as_ref().ok_or(ConnectionError::TargetNotAvailable)?;

        if (*target_input.1).type_id() != expected_message_type {
            // The target doesn't have the expected message type. If there's a conversion filter, we can still return the target type
            if self.filter_conversions.contains_key(&(stream_id.as_message_type(), target_input.0.as_message_type())) {
                // The input stream doesn't match the output but there's a filter to convert between them available (only direct conversions are supported, so a chain of filters won't be followed)
                Ok(Arc::clone(&target_input.1))
            } else {
                // Can't use this stream as it doesn't match stream_id
                let stream_type     = stream_id.message_type_name();
                let program_type    = self.sub_programs[handle].as_ref().unwrap().lock().unwrap().expected_input_type_name.to_string();

                Err(ConnectionError::WrongInputType(SourceStreamMessageType(stream_type), TargetInputMessageType(program_type)))
            }
        } else {
            Ok(Arc::clone(&target_input.1))
        }
    }

    ///
    /// Sends a set of updates to the update stream (if there is one set for this core)
    ///
    pub (crate) fn send_scene_updates(scene_core: &Arc<Mutex<SceneCore>>, updates: Vec<SceneUpdate>) {
        use std::mem;

        if updates.is_empty() {
            return;
        }

        let core = scene_core.lock().unwrap();

        if let Some((pid, update_core)) = core.updates.as_ref() {
            let mut update_sink = OutputSink::attach(*pid, Arc::clone(update_core), scene_core);
            mem::drop(core);

            for update in updates {
                update_sink.send_immediate(update).ok();
            }
        }
    }

    ///
    /// Adds or updates a program connection in this core
    ///
    pub (crate) fn connect_programs(core: &Arc<Mutex<SceneCore>>, source: StreamSource, target: StreamTarget, stream_id: StreamId) -> Result<ConnectionResult, ConnectionError> {
        // Make sure the target stream ID type  is initialised
        Self::initialise_message_type(core, stream_id.clone());

        // Check source/target filter streams
        match (&source, &target) {
            (StreamSource::Filtered(source_filter), StreamTarget::Filtered(target_filter, _)) => {
                let source_stream_id = source_filter.source_stream_id_any()?;
                if source_stream_id.as_message_type() != stream_id.as_message_type() {
                    return Err(ConnectionError::FilterSourceInputMustMatchStream);
                }

                let middle_stream_id = source_filter.target_stream_id_any()?;
                let target_stream_id = target_filter.source_stream_id_any()?;
                if middle_stream_id != target_stream_id {
                    return Err(ConnectionError::FilterTargetInputMustMatchStream);
                }
            },

            (StreamSource::Filtered(source_filter), _) => {
                let source_stream_id = source_filter.source_stream_id_any()?;
                if source_stream_id.as_message_type() != stream_id.as_message_type() {
                    return Err(ConnectionError::FilterSourceInputMustMatchStream);
                }
            },

            (_, StreamTarget::Filtered(target_filter, _)) => {
                let target_stream_id = target_filter.source_stream_id_any()?;
                if target_stream_id.as_message_type() != stream_id.as_message_type() {
                    return Err(ConnectionError::FilterTargetInputMustMatchStream);
                }
            },

            _ => { 
                // Not filtered
            }
        }

        // Certain combinations of source and target can be expressed in a more 'standard' way
        let (source, target) = match (source, target) {
            (StreamSource::Filtered(filter), StreamTarget::Program(program)) => {
                // Connecting a filter source to a program is the same as connecting anything to the filter
                (StreamSource::All, StreamTarget::Filtered(filter, program))
            },

            (source, target) => (source, target),
        };

        // Call finish_connecting_programs to determine the result of the connection
        let result = SceneCore::finish_connecting_programs(core, source.clone(), target.clone(), stream_id.clone());

        // If successful and the target is a filter, connect the specific stream for the target as well as the 'all' stream
        if result.is_ok() {
            // Reconnect any streams that specifically target the filter as well
            if let StreamTarget::Filtered(_, target_program_id) = target {
                if stream_id.target_program().is_none() {
                    SceneCore::finish_connecting_programs(core, source.clone(), target.clone(), stream_id.for_target(target_program_id)).ok();
                }
            }

            // Reconnect any streams that are affected by an 'any' source filter: that is, anything in filter_conversions that targets the affected stream
            let filter_conversions = core.lock().unwrap().filter_conversions
                .keys()
                .filter(|(_, target)| target.message_type() == stream_id.message_type())
                .cloned()
                .collect::<Vec<_>>();
            let subprograms = if filter_conversions.is_empty() { 
                vec![] 
            } else {
                core.lock().unwrap().sub_programs.iter().flatten().cloned().collect()
            };

            // filter_conversions is the list of every stream that uses this message type and might be affected by the source filter
            for (convert_from, _) in filter_conversions {
                // Check for subprograms that write the to the stream affected by this connection
                for subprogram in subprograms.iter() {
                    // If the subprogram sends a message that can be converted by this filter, reconnect it
                    if subprogram.lock().unwrap().has_output_sink(&convert_from) {
                        let waker = SubProgramCore::reconnect_disconnected_outputs(subprogram, core, &convert_from);

                        if let Some(waker) = waker {
                            waker.wake();
                        }
                    }
                }
            }
        }

        // Send an update if there's an error
        if let Err(err) = &result {
            let update = SceneUpdate::FailedConnection(err.clone(), source, target, stream_id);
            SceneCore::send_scene_updates(core, vec![update]);
        }

        result
    }

    ///
    /// Finishes a program connection, sending updates if successful
    ///
    #[allow(clippy::type_complexity)]   // Creating a type for reconnect_subprogram just looks super goofy and is a lifetime nightmare
    fn finish_connecting_programs(core: &Arc<Mutex<SceneCore>>, source: StreamSource, target: StreamTarget, stream_id: StreamId) -> Result<ConnectionResult, ConnectionError> {
        // If the source is a filter source, then add to the list of available filter programs
        if let StreamSource::Filtered(source_filter) = &source {
            // Get the source and target streams IDs, with no target
            let source_stream = source_filter.source_stream_id_any()?;
            let target_stream = source_filter.target_stream_id_any()?;

            // Store this filter handle as a possible conversion for a mismatched input
            let mut core = core.lock().unwrap();
            core.filter_conversions.insert((source_stream.clone(), target_stream.clone()), *source_filter);

            match target {
                StreamTarget::None | StreamTarget::Program(_) | StreamTarget::Filtered(_, _) => { 
                    // Nothing to do
                }

                StreamTarget::Any => { 
                    // Add this as a possible target for an output stream of the specified type. This stream becomes the lowest priority stream if there are several choices
                    // for making a connection
                    let possible_targets = core.filtered_targets.entry(source_stream.clone())
                        .or_insert_with(|| vec![]);
                    possible_targets.retain(|old_target| old_target != &target_stream);
                    possible_targets.push(target_stream);

                    // This replaces the 'all' connection for this stream type, if there is one
                    core.connections.remove(&(StreamSource::All, source_stream));
                }
            }
        }

        // Create a function to reconnect a subprogram
        let reconnect_subprogram: Box<dyn Fn(&Arc<Mutex<SubProgramCore>>) -> Option<Waker>> = match &target {
            StreamTarget::None                  => Box::new(|sub_program| sub_program.lock().unwrap().discard_output_from(&stream_id)),
            StreamTarget::Any                   => {
                if let StreamSource::Filtered(source_filter) = &source {
                    let source_stream = source_filter.source_stream_id_any()?;

                    Box::new(move |sub_program| SubProgramCore::reconnect_disconnected_outputs(sub_program, core, &source_stream))
                } else {
                    Box::new(|sub_program| sub_program.lock().unwrap().disconnect_output_sink(&stream_id))
                }
            },

            StreamTarget::Program(subprogid)    => {
                let mut core        = core.lock().unwrap();
                let stream_id       = &stream_id;
                let target_input    = core.get_target_input(subprogid, stream_id);

                match target_input {
                    Ok(target_input)                            => Box::new(move |sub_program| sub_program.lock().unwrap().reconnect_output_sinks(&target_input, stream_id, false)),
                    Err(ConnectionError::TargetNotInScene)      => Box::new(move |sub_program| sub_program.lock().unwrap().disconnect_output_sink(&stream_id)),
                    Err(ConnectionError::WrongInputType(_, _))  => Box::new(move |sub_program| sub_program.lock().unwrap().disconnect_output_sink(&stream_id)),
                    Err(err)                                    => { return Err(err); },
                }
            },

            StreamTarget::Filtered(filter_handle, subprogid) => {
                let core            = core.clone();
                let input_stream_id = filter_handle.target_stream_id(*subprogid)?;
                let target_input    = core.lock().unwrap().get_target_input(subprogid, &input_stream_id)?;
                let stream_id       = &stream_id;

                Box::new(move |sub_program| {
                    if sub_program.lock().unwrap().has_output_sink(stream_id) {
                        let sub_program_id  = sub_program.lock().unwrap().id;
                        let input           = filter_handle.create_input_stream_core(&core, sub_program_id, Arc::clone(&target_input));

                        if let Ok(input) = input {
                            sub_program.lock().unwrap().reconnect_output_sinks(&input, stream_id, true)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
            },
        };

        // TODO: pause the inputs of all the sub-programs matching the source, so the update is atomic?
        // TODO: if there's a filtered connection we really should wait for the filter program to stop before starting new input to avoid situations where some values can arrive out-of-order

        let sub_programs = {
            let mut core = core.lock().unwrap();

            // Store the connection
            core.connections.insert((source.clone(), stream_id.clone()), target.clone());

            // Fetch the sub-programs to update
            core.sub_programs.clone()
        };

        // Update the existing connections
        let mut scene_updates = vec![];
        let target_program_id = target.target_sub_program();

        for sub_program in sub_programs.iter().flatten() {
            // Update the streams of the subprogram
            let sub_program_id = sub_program.lock().unwrap().id;

            if source.matches_subprogram(&sub_program_id) && sub_program.lock().unwrap().has_output_sink(&stream_id) {
                // Reconnect the program
                let waker = reconnect_subprogram(sub_program);

                if let Some(target_program_id) = target_program_id {
                    scene_updates.push(SceneUpdate::Connected(sub_program_id, target_program_id, stream_id.clone()));
                } else {
                    scene_updates.push(SceneUpdate::Disconnected(sub_program_id, stream_id.clone()));
                }

                // Wake the input stream
                if let Some(waker) = waker {
                    waker.wake();
                }
            }
        }

        // Send the updates on how the connections have changed
        SceneCore::send_scene_updates(core, scene_updates);

        if target_program_id.is_some() {
            // TODO: determine if the target program can accept connections of this type
            Ok(ConnectionResult::Ready)
        } else {
            Ok(ConnectionResult::TargetNotReady)
        }
    }

    ///
    /// Creates an InputStreamCore that reads the input type of a filter, then chains the output through another filter to generate an output
    ///
    fn filtered_chain_input_for_program<TSourceMessageType>(core: &Arc<Mutex<SceneCore>>, source_program: SubProgramId, initial_filter: FilterHandle, second_filter: FilterHandle, target_program: SubProgramId) -> Result<Arc<Mutex<InputStreamCore<TSourceMessageType>>>, ConnectionError> 
    where
        TSourceMessageType: 'static + SceneMessage,
    {
        // Fetch the input core for the target program
        let target_input_core = {
            let core            = core.lock().unwrap();

            let target_index    = core.program_indexes.get(&target_program).ok_or(ConnectionError::TargetNotInScene)?;
            let target_core     = core.sub_program_inputs.get(*target_index).ok_or(ConnectionError::TargetNotInScene)?.as_ref().ok_or(ConnectionError::TargetNotInScene)?;

            Arc::clone(&target_core.1)
        };

        // Create an input stream core to use with it by chaining the two filters
        initial_filter.chain_filters(core, source_program, second_filter, target_input_core)
            .and_then(|input_core| input_core
                .downcast::<Mutex<InputStreamCore<TSourceMessageType>>>()
                .map_err(|_| ConnectionError::FilterInputDoesNotMatch))
    }

    ///
    /// Creates an InputStreamCore that reads the input type of a filter, and outputs to the input core of a program with the output type of the filter (if the types all match up)
    ///
    fn filtered_input_for_program<TSourceMessageType>(core: &Arc<Mutex<SceneCore>>, source_program: SubProgramId, filter_handle: FilterHandle, target_program: SubProgramId) -> Result<Arc<Mutex<InputStreamCore<TSourceMessageType>>>, ConnectionError> 
    where
        TSourceMessageType: 'static + SceneMessage,
    {
        // Fetch the input core for the target program
        let target_input_core = {
            let core            = core.lock().unwrap();

            let target_index    = core.program_indexes.get(&target_program).ok_or(ConnectionError::TargetNotInScene)?;
            let target_core     = core.sub_program_inputs.get(*target_index).ok_or(ConnectionError::TargetNotInScene)?.as_ref().ok_or(ConnectionError::TargetNotInScene)?;

            Arc::clone(&target_core.1)
        };

        let source_stream_id    = StreamId::with_message_type::<TSourceMessageType>();
        let filter_input        = filter_handle.source_stream_id_any()?;

        // If a source filter is applied to the output of the program, then the filter input might not match the source stream ID (we'll need to apply a conversion filter)
        if filter_input != source_stream_id {
            // The filter needs further mapping to change the source stream to its input
            let initial_filter = core.lock().unwrap().filter_conversions.get(&(source_stream_id, filter_input)).copied();

            if let Some(initial_filter) = initial_filter {
                // Chain the filter we found to the filter that was requested
                initial_filter.chain_filters(core, source_program, filter_handle, target_input_core)
                    .and_then(|input_core| input_core
                        .downcast::<Mutex<InputStreamCore<TSourceMessageType>>>()
                        .map_err(|_| ConnectionError::FilterInputDoesNotMatch))
            } else {
                // Filter not defined (in general if we reach here it should have been defined)
                Err(ConnectionError::FilterMappingMissing)
            }
        } else {
            // Create an input stream core to use with it
            filter_handle.create_input_stream_core(core, source_program, target_input_core)
                .and_then(|input_core| input_core
                    .downcast::<Mutex<InputStreamCore<TSourceMessageType>>>()
                    .map_err(|_| ConnectionError::FilterInputDoesNotMatch))
        }
    }

    ///
    /// Checks for a filter that can map between a source output and a target input, and generates appropriate filtered input if one exists
    ///
    pub (crate) fn filter_source_for_program<TSourceMessageType>(scene_core: &Arc<Mutex<SceneCore>>, source_program: SubProgramId, source_id: &StreamId, target_id: &StreamId, target_program: SubProgramId) -> Result<Arc<Mutex<InputStreamCore<TSourceMessageType>>>, ConnectionError>
    where
        TSourceMessageType: 'static + SceneMessage,
    {
        use std::mem;

        // Strip out any target program from the source and target
        let source_id = source_id.as_message_type();
        let target_id = target_id.as_message_type();

        // Look up a direct filter between the source and target programs
        let filter = {
            let core    = scene_core.lock().unwrap();
            let filter  = core.filter_conversions.get(&(source_id.clone(), target_id.clone())).copied();

            filter
        };

        if let Some(filter) = filter {
            // There's a simple conversion between the output of the source and the input of the target
            Self::filtered_input_for_program(scene_core, source_program, filter, target_program)
        } else {
            // TODO: this is a little dense, might be good to split this up/cut things down a bit

            // Search for a source filter that can map from the source_id to an input supported by the stream (we'll need to use a chained filter to do this)
            let core = scene_core.lock().unwrap();
            if let Some(source_filters) = core.filtered_targets.get(&source_id) {
                // Try to find a source filter that matches a connection for the target program
                let filtered_connection = source_filters.iter()
                    .find_map(|filter_output_stream| {
                        // Use this filter if the target program has a connection that accepts its output
                        if let Some(connection) = core.connections.get(&(StreamSource::Program(source_program), filter_output_stream.for_target(target_program))) {
                            // There's a filter we can use from this specific program
                            Some((connection.clone(), filter_output_stream.clone()))
                        } else if let Some(connection) = core.connections.get(&(StreamSource::All, filter_output_stream.for_target(target_program))) {
                            // There's a filter we can use for any connection to this program
                            Some((connection.clone(), filter_output_stream.clone()))
                        } else {
                            None
                        }
                    });

                // filtered_connection will contain a connection if there's a filter we can apply to the source that will connect to the target
                if let Some((connection, filter_output_stream_id)) = filtered_connection {
                    // Get the filter conversion for the source stream
                    let input_filter = core.filter_conversions.get(&(source_id.clone(), filter_output_stream_id)).copied();

                    // Get the filter conversion and 'true' target for the connection
                    let (output_filter, target_program) = match connection {
                        StreamTarget::Any | StreamTarget::None      => (None, None),
                        StreamTarget::Program(program_id)           => (None, Some(program_id)),
                        StreamTarget::Filtered(filter, program_id)  => (Some(filter), Some(program_id)),
                    };

                    mem::drop(core);

                    // Generate the connection, if we can - chain if there's both an output and an input filter, or use a direct connection if there's just an input filter
                    // If the input filter was not found, then that's usually a bug as it should exist for the filtered connection to be non-None (we'll indicate a bad input type as if there's no conversion)
                    match (input_filter, output_filter, target_program) {
                        (_, _, None) => Err(ConnectionError::WrongInputType(SourceStreamMessageType(source_id.message_type_name()), TargetInputMessageType(target_id.message_type_name()))),

                        (Some(input_filter), None, Some(target_program))                => Self::filtered_input_for_program(scene_core, source_program, input_filter, target_program),
                        (Some(input_filter), Some(output_filter), Some(target_program)) => Self::filtered_chain_input_for_program(scene_core, source_program, input_filter, output_filter, target_program),

                        _ => Err(ConnectionError::WrongInputType(SourceStreamMessageType(source_id.message_type_name()), TargetInputMessageType(target_id.message_type_name()))),
                    }
                } else {
                    // There's no way to map this stream
                    Err(ConnectionError::WrongInputType(SourceStreamMessageType(source_id.message_type_name()), TargetInputMessageType(target_id.message_type_name())))
                }
            } else {
                // There are no source filters for this stream type
                Err(ConnectionError::WrongInputType(SourceStreamMessageType(source_id.message_type_name()), TargetInputMessageType(target_id.message_type_name())))
            }
        }
    }

    ///
    /// If a stream can be mapped by a filter, this will return the stream ID of the target of that filter
    ///
    pub (crate) fn filter_mapped_target(&self, source_stream_id: &StreamId) -> Option<StreamTarget> {
        if let Some(possible_target_stream_ids) = self.filtered_targets.get(source_stream_id) {
            // Search for a connection that can accept a connection of this type
            for target_stream_id in possible_target_stream_ids {
                match self.connections.get(&(StreamSource::All, target_stream_id.clone())) {
                    None                        |
                    Some(StreamTarget::None)    | 
                    Some(StreamTarget::Any)     => { /* No connection for this stream type */ },
                    Some(target)                => {
                        // There exists a connection for this stream type
                        return Some(target.clone());
                    }
                }
            }

            None
        } else {
            None
        }
    }

    ///
    /// Returns the 'mapped' StreamTarget for a connection. This is the actual target that a program should be sent to: for example if the `target` is passed
    /// in as 'Any' and there's a connection specified for that target, this will return that connection.
    ///
    /// This can still return 'Any' (indicating that output should be held until a connection is specified), or 'None' (indicating that output should be
    /// immediately discarded).
    ///
    /// The result of this function should not be mapped further, as it will point at the actual program that is the target if there 
    /// is one.
    ///
    pub (crate) fn mapped_target_for_connection(&self, source: &StreamSource, target: &StreamTarget, stream_id: &StreamId) -> Result<StreamTarget, ConnectionError> {
        let mapped_target = match target {
            StreamTarget::None | StreamTarget::Any => {
                if let Some(source_specific_target) = self.connections.get(&(source.clone(), stream_id.clone())) {
                    // If there's a specific mapping for this stream ID from this source, use that for preference
                    source_specific_target.clone()
                } else if let Some(general_target) = self.connections.get(&(StreamSource::All, stream_id.clone())) {
                    // Otherwise, if there's a general connection for all streams, use that
                    general_target.clone()
                } else if let Some(filter_mapped_target) = self.filter_mapped_target(stream_id) {
                    // If there's no way to directly connect a stream, then see if there's a filter that can be used to make the connection instead 
                    filter_mapped_target
                } else if let StreamTarget::Any = target {
                    // The 'any' stream target can use the default target for this stream ID
                    stream_id.default_target()
                } else {
                    // This stream has no target at the moment (StreamTarget::None should discard its output and StreamTarget::Any should wait for a stream to become available)
                    target.clone()
                }
            }

            StreamTarget::Program(program_id) => {
                if let Some(overridden_target) = self.connections.get(&(source.clone(), stream_id.for_target(program_id))) {
                    // There's a connection that overrides the specific connection from this source to this target
                    overridden_target.clone()
                } else if let Some(overridden_target) = self.connections.get(&(StreamSource::All, stream_id.for_target(program_id))) {
                    // All streams targeting this program are overidden by a different connection
                    overridden_target.clone()
                } else {
                    // Use the program specified by the target
                    StreamTarget::Program(*program_id)
                }
            }

            StreamTarget::Filtered(filter_handle, program_id) => {
                let filter_output = filter_handle.target_stream_id_any()?;

                if let Some(filtered_redirect) = self.connections.get(&(source.clone(), filter_output.for_target(program_id))) {
                    // The type that is output by the filter has an overridden connection
                    filtered_redirect.clone()
                } else if let Some(filtered_redirect) = self.connections.get(&(StreamSource::All, filter_output.for_target(program_id))) {
                    // The type that is output by the filter has an overridden connection
                    filtered_redirect.clone()
                } else {
                    // Just send directly to the target program
                    StreamTarget::Program(*program_id)
                }
            }
        };

        Ok(mapped_target)
    }

    ///
    /// Returns the output sink target configured for a particular stream
    ///
    pub (crate) fn sink_for_target<TMessageType>(scene_core: &Arc<Mutex<SceneCore>>, source: &SubProgramId, target: StreamTarget) -> Result<OutputSinkTarget<TMessageType>, ConnectionError>
    where
        TMessageType: 'static + SceneMessage,
    {
        use std::mem;

        // Make sure that the message type is ready to use
        Self::initialise_message_type(scene_core, StreamId::with_message_type::<TMessageType>());

        // Get the filter we're using as the output for the current stream
        let output_filter = match &target {
            StreamTarget::Filtered(filter, _)   => Some(*filter),
            _                                   => None,
        };

        // Map the target to get the real target
        let core            = scene_core.lock().unwrap();
        let mapped_target   = core.mapped_target_for_connection(&source.into(), &target, &StreamId::with_message_type::<TMessageType>())?;

        let output_sink_target = match (output_filter, mapped_target) {
            (_, StreamTarget::None) => OutputSinkTarget::<TMessageType>::Discard,
            (_, StreamTarget::Any)  => OutputSinkTarget::Disconnected,

            (None, StreamTarget::Program(target_program_id)) => {
                // Fetch the input for the target program
                let target_program_handle   = core.program_indexes.get(&target_program_id);
                let target_program_handle   = if let Some(target_program_handle) = target_program_handle { target_program_handle } else { return Ok(OutputSinkTarget::Disconnected); };
                let target_program_input    = core.sub_program_inputs.get(*target_program_handle).ok_or(ConnectionError::TargetNotInScene)?.clone().ok_or(ConnectionError::TargetNotInScene)?;
                mem::drop(core);

                // Connect directly to the output if it matches the source stream type, otherwise apply an input filter to convert the type
                let target_program_input = target_program_input.1.downcast::<Mutex<InputStreamCore<TMessageType>>>()
                    .or_else(|_| Self::filter_source_for_program::<TMessageType>(scene_core, *source, &StreamId::with_message_type::<TMessageType>(), &target_program_input.0, target_program_id));

                // If the target program is running but doesn't support 
                match target_program_input {
                    Ok(target_program_input)                    => OutputSinkTarget::Input(Arc::downgrade(&target_program_input)),
                    Err(ConnectionError::WrongInputType(_, _))  => OutputSinkTarget::Disconnected,      // The filter can be added later, so using a currently invalid type is allowed, creates a disconnected stream
                    Err(ConnectionError::TargetNotInScene)      => OutputSinkTarget::Disconnected,      // The target can be started later on, so we start disconnected
                    Err(err)                                    => { return Err(err); }
                }
                
            }

            (None, StreamTarget::Filtered(input_filter, target_program_id)) => {
                // Connect the core using the input filter
                mem::drop(core);
                match Self::filtered_input_for_program(scene_core, *source, input_filter, target_program_id) {
                    Ok(filtered_input_core)                         => OutputSinkTarget::CloseWhenDropped(Arc::downgrade(&filtered_input_core)),
                    Err(ConnectionError::WrongInputType(_, _))      => OutputSinkTarget::Disconnected,
                    Err(ConnectionError::TargetNotInScene)          => OutputSinkTarget::Disconnected,
                    Err(err)                                        => { return Err(err); }
                }
            }

            (Some(output_filter), StreamTarget::Program(target_program_id)) => {
                // Filter the output
                mem::drop(core);
                match Self::filtered_input_for_program(scene_core, *source, output_filter, target_program_id) {
                    Ok(filtered_output_core)                        => OutputSinkTarget::CloseWhenDropped(Arc::downgrade(&filtered_output_core)),
                    Err(ConnectionError::WrongInputType(_, _))      => OutputSinkTarget::Disconnected,
                    Err(ConnectionError::TargetNotInScene)          => OutputSinkTarget::Disconnected,
                    Err(err)                                        => { return Err(err); }
                }
            }

            (Some(output_filter), StreamTarget::Filtered(input_filter, target_program_id)) => {
                // Source filters can't be used when both sides are filtered as we need to chain the source and the target
                mem::drop(core);
                match Self::filtered_chain_input_for_program(scene_core, *source, output_filter, input_filter, target_program_id) {
                    Ok(filtered_core)                               => OutputSinkTarget::CloseWhenDropped(Arc::downgrade(&filtered_core)),
                    Err(ConnectionError::WrongInputType(_, _))      => OutputSinkTarget::Disconnected,
                    Err(ConnectionError::TargetNotInScene)          => OutputSinkTarget::Disconnected,
                    Err(err)                                        => { return Err(err); }
                }
            }
        };

        Ok(output_sink_target)
    }

    ///
    /// Starts a new process running in this scene
    ///
    pub (crate) fn start_process(&mut self, process: impl 'static + Send + Future<Output=()>) -> (ProcessHandle, Option<Waker>) {
        // Assign a process ID to this process
        let process_id = self.next_process;
        while self.processes.len() <= process_id {
            self.processes.push(None);
        }

        // Update the next process ID
        self.next_process += 1;
        while self.next_process < self.processes.len() && self.processes[self.next_process].is_some() {
            self.next_process += 1;
        }

        // Store the new process
        let new_process = SceneProcess {
            future:                 SceneProcessFuture::Waiting(process.boxed()),
            is_awake:               true,
            unpark_when_waiting:    vec![],
        };
        self.processes[process_id] = Some(new_process);

        // Mark as awake
        self.awake_processes.push_back(process_id);

        // The caller should call the waker once the core has been locked again (which is why we don't call it ourselves here)
        let mut waker   = None;
        for maybe_waker in self.thread_wakers.iter_mut() {
            waker = maybe_waker.take();
            if waker.is_some() {
                break;
            }
        }

        (ProcessHandle(process_id), waker)
    }

    ///
    /// Retrieves a list of the currently running subprogram IDs
    ///
    pub (crate) fn get_running_subprograms(&self) -> Vec<SubProgramId> {
        self.program_indexes.keys()
            .copied()
            .collect()
    }

    ///
    /// Retrieves the subprogram core for an ID if it exists
    ///
    pub (crate) fn get_sub_program(&self, sub_program_id: SubProgramId) -> Option<Arc<Mutex<SubProgramCore>>> {
        let handle = self.program_indexes.get(&sub_program_id)?;

        if let Some(subprogram) = self.sub_programs.get(*handle) {
            subprogram.clone()
        } else {
            None
        }
    }

    ///
    /// Retrieves the input stream core for a subprogram, if it exists
    ///
    pub (crate) fn get_input_stream_core(&self, sub_program_id: SubProgramId) -> Option<Arc<dyn Send + Sync + Any>> {
        let handle = self.program_indexes.get(&sub_program_id)?;

        if let Some(input_stream_core) = self.sub_program_inputs.get(*handle) {
            input_stream_core.as_ref().map(|core| &core.1).cloned()
        } else {
            None
        }
    }

    ///
    /// Stops this scene, returning the wakers that need to be invoked to finish stopping it
    ///
    pub (crate) fn stop(&mut self) -> Vec<Waker> {
        self.stopped = true;

        self.thread_wakers
            .iter_mut()
            .filter_map(|waker| waker.take())
            .collect()
    }

    ///
    /// Sets the output sink core to use with the updates stream
    ///
    pub (crate) fn set_update_core(&mut self, program_id: SubProgramId, core: Arc<Mutex<OutputSinkCore<SceneUpdate>>>) {
        self.updates = Some((program_id, core));
    }

    ///
    /// Polls the process for the specified program on this thread
    ///
    pub (crate) fn steal_thread_for_program<TMessage>(core: &Arc<Mutex<SceneCore>>, program_id: SubProgramId) -> Result<(), SceneSendError<TMessage>> {
        // Fetch the program whose process we're going to run
        let subprogram = {
            let core    = core.lock().unwrap();

            core.program_indexes.get(&program_id)
                .and_then(|idx| core.sub_programs.get(*idx).cloned())
                .unwrap_or(None)
        };

        let subprogram = subprogram.ok_or(SceneSendError::TargetProgramEndedBeforeReady)?;

        // Try to fetch the future for the process (back to the scene core again here)
        let (process_id, mut process_future) = {
            // Loop until the process future is available on this thread
            loop {
                // We lock both the core and the subprogram here so that the process cannot end before we get the future
                let mut core    = core.lock().unwrap();
                let process_id  = subprogram.lock().unwrap().process_id.ok_or(SceneSendError::TargetProgramEndedBeforeReady)?;

                let process     = core.processes.get_mut(process_id.0)
                    .map(|process| process.as_mut())
                    .unwrap_or(None)
                    .ok_or(SceneSendError::TargetProgramEndedBeforeReady)?;

                // If the process is available, return it to the rest of the thread, in order to run it here
                match process.future.take() {
                    Some(future)    => { break (process_id.0, future); }
                    None            => {
                        if process.future.is_running_on_this_thread() {
                            // Error if we can't steal the thread because the process is already running on this thread
                            return Err(SceneSendError::CannotReEnterTargetProgram);
                        }

                        // This future is already running on another thread, so we cannot steal it 
                        return Ok(());
                    }
                }
            }
        };

        // Poll the future (reawaken the core later on)
        let scene_waker = waker(Arc::new(SceneCoreWaker::with_core(core, process_id)));
        let poll_result = poll_thread_steal(process_future.as_mut(), Some(scene_waker));

        // Return the future to the core/finish it
        let waker = {
            let mut scene_core = core.lock().unwrap();

            if poll_result.is_ready() {
                // Process was finished: free it up for the future 
                scene_core.processes[process_id]    = None;
                scene_core.next_process             = process_id.min(scene_core.next_process);
                scene_core.awake_processes.retain(|pid| pid != &process_id);

                None
            } else {
                // Process still running: return the future so that it'll actually run
                let process = scene_core.processes[process_id].as_mut().unwrap();
                process.future = SceneProcessFuture::Waiting(process_future);

                // Wake any threads that were waiting for this process
                process.unpark_when_waiting.drain(..).for_each(|thread| thread.unpark());

                // If the future has woken up since the poll finished, then re-awaken the scene using a scene waker
                if scene_core.processes[process_id].as_mut().unwrap().is_awake {
                    Some(waker(Arc::new(SceneCoreWaker::with_core(core, process_id))))
                } else {
                    None
                }
            }
        };

        // We need to reawaken the core if the process turns out to be awake again
        if let Some(waker) = waker {
            waker.wake()
        }

        Ok(())
    }

    ///
    /// Adds a sender to be notified whenever the core is idle
    ///
    pub (crate) fn send_idle_notifications_to(core: &Arc<Mutex<SceneCore>>, notifier: mpsc::Sender<()>) {
        let mut core = core.lock().unwrap();

        core.when_idle.push(Some(notifier));
    }

    ///
    /// The core will send a notification next time it's idle
    ///
    pub (crate) fn notify_on_next_idle(core: &Arc<Mutex<SceneCore>>) {
        let mut core = core.lock().unwrap();

        core.notify_when_idle = true;
    }

    ///
    /// Checks if the core needs to signal that it's idle, and does so if necessary
    ///
    pub (crate) fn check_if_idle(core: &Arc<Mutex<SceneCore>>) -> bool {
        use std::mem;

        // Fetch the active input streams from the core (or stop, if we're not notifying)
        let sub_program_inputs = {
            let core = core.lock().unwrap();

            if !core.notify_when_idle {
                // Give up quickly if the core is not waiting for a notification
                return false;
            }

            core.sub_program_inputs
                .iter()
                .flatten()
                .cloned()
                .collect::<Vec<_>>()
        };

        // Check the inputs to see if they are idle
        let all_inputs_idle = sub_program_inputs.iter()
            .all(|(stream_id, input_stream, _program_id)| stream_id.is_idle(input_stream) == Ok(true));

        let all_processes_idle = if !all_inputs_idle {
            // If the inputs are not idle, then the core is not idle (we'll just assume that the processes are asleep)
            // (The scene is not idle until all the processes are asleep and all subprograms are ready to process their next input)
            false
        } else {
            // If the inputs are all idle, then all the processes must be asleep and waiting for more input as well
            core.lock().unwrap()
                .processes.iter()
                .all(|process| if let Some(process) = process {
                    !process.is_awake && process.future.is_waiting()
                } else {
                    true
                })
        };

        // If all inputs are idle and the core is still in 'notification' mode, notify the waiting messages
        if all_inputs_idle && all_processes_idle {
            let mut locked_core = core.lock().unwrap();

            if !locked_core.notify_when_idle {
                // Some other thread has presumably notified
                return false;
            }

            // We're going to notify, so prevent other threads from reaching this point
            locked_core.notify_when_idle = false;

            // Get the notifiers from the core
            let mut notifiers = locked_core.when_idle.iter_mut()
                .enumerate()
                .map(|(idx, notifier)| (idx, notifier.clone()))
                .collect::<Vec<_>>();

            // Unlock the core (sending to the notifiers might call a waker, which might in turn re-lock the core)
            mem::drop(locked_core);

            // Unlock the core (we need to be able to wake it up again without deadlocking)
            for (_idx, notifier) in notifiers.iter_mut() {
                let is_sent = if let Some(notifier) = notifier {
                    // Try to send to this notifier immediately
                    notifier.try_send(())
                } else {
                    // Notifier is disconnected, or a notification is being sent on another thread
                    Ok(())
                };

                if let Err(send_error) = is_sent {
                    if send_error.is_disconnected() {
                        // Don't try to send to this stream any more
                        // TODO: if we ever need to create/dispose many of these, it'll make more sense to have a function to remove them
                        *notifier = None;
                    } else if send_error.is_full() {
                        // Something is already processing a notification
                        // Not notifying it again
                        // TODO: could try sending in a process (possibly want to gather all the senders in one place)
                    }
                }
            }

            // Return the notifiers to the core
            let mut locked_core = core.lock().unwrap();

            for (idx, notifier) in notifiers.drain(..) {
                if notifier.is_none() {
                    // This notifier was disconnected (or is still disconnected)
                    locked_core.when_idle[idx] = None;
                }
            }

            // Indicate to the caller that a notification occurred
            true
        } else {
            false
        }
    }

    ///
    /// After a program has started, finds any connections that are targetting it and remakes them
    ///
    pub fn reconnect_subprogram(scene_core: &Arc<Mutex<SceneCore>>, subprogram_id: SubProgramId) {
        // Get the subprograms and active connections from the core
        let (subprograms, connections) = {
            let core = scene_core.lock().unwrap();

            let subprograms = core.sub_programs.iter()
                .flatten()
                .cloned()
                .collect::<Vec<_>>();
            let connections = core.connections.clone();

            (subprograms, connections)
        };

        // For each subprogram, find any outputs that might connect to our target, and reconnect them
        for subprogram in subprograms {
            let core = subprogram.lock().unwrap();

            let source_id = core.id;
            if source_id == subprogram_id {
                // Don't connect the target program
                continue;
            }

            let mut targets_to_reconnect = vec![];
            for (stream_id, output_sink_core) in core.outputs.iter() {
                // Skip this connection if it does not target us
                if let Some(target) = stream_id.target_program() {
                    // Souce stream is targeting a specific program
                    let connection = connections.get(&(target.into(), stream_id.clone()));
                    if let Some(connection) = connection {
                        if connection.target_sub_program() == Some(subprogram_id) {
                            // This connection matches our subprogram (may be redirected from a different target)
                        } else {
                            // This connection does not match our stream
                            continue;
                        }
                    } else if target == subprogram_id {
                        // Direct connection to this subprogram
                    } else {
                        // Connection to somewhere else
                        continue;
                    }
                } else {
                    let any_connection = connections.get(&(StreamSource::All, stream_id.clone()));

                    if let Some(connection) = any_connection {
                        if connection.target_sub_program() == Some(subprogram_id) {
                            // This is an 'any' connection that targets this program
                        } else {
                            continue;
                        }
                    } else {
                        // This stream has no target
                        continue;
                    }
                }

                // This target is needs reconnecting as it targets our program, we'll do this once we've released the lock
                targets_to_reconnect.push((stream_id.clone(), output_sink_core.clone()));
            }

            // Reconnect any targets
            for (stream_id, output_sink_core) in targets_to_reconnect.into_iter() {
                // Try to connect this core to this program
                if let Ok(Some(waker)) = stream_id.reconnect_output_sink(scene_core, &output_sink_core, source_id, StreamTarget::Program(subprogram_id)) {
                    // Errors are ignored, they'll leave the sink's state alone
                    waker.wake();
                }
            }
        }
    }
}

impl SceneCoreWaker {
    ///
    /// Creates a waker for a scene core
    ///
    pub fn with_core(core: &Arc<Mutex<SceneCore>>, process_id: usize) -> Self {
        Self {
            core:       Arc::downgrade(core),
            process_id: process_id,
        }
    }
}

impl ArcWake for SceneCoreWaker {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        // If the scene is still running, fetch the core
        let process_id  = arc_self.process_id;
        let core        = if let Some(core) = arc_self.core.upgrade() { core } else { return; };

        // Fetch a waker from the core to wake up a thread
        let waker = {
            let mut core    = core.lock().unwrap();

            // Pick a waker from the core
            let mut waker   = None;
            for maybe_waker in core.thread_wakers.iter_mut() {
                waker = maybe_waker.take();
                if waker.is_some() {
                    break;
                }
            }

            // Retrieve the program from the core
            let process = core.processes.get_mut(process_id);

            if let Some(Some(process)) = process {
                // Add the process to the awake list, if it's not there already
                process.is_awake = true;

                if !core.awake_processes.contains(&process_id) {
                    core.awake_processes.push_back(process_id);
                }
            }

            waker
        };

        // Wake up a polling routine (which should in turn poll the program)
        if let Some(waker) = waker {
            waker.wake()
        }
    }
}

///
/// Runs the programs attached to a scene core
///
pub (crate) fn run_core(core: &Arc<Mutex<SceneCore>>) -> impl Future<Output=()> {
    use std::mem;

    let unlocked_core = Arc::clone(core);

    // Choose a location to store the waker for this core instance
    let waker_idx;
    {
        let mut core = unlocked_core.lock().unwrap();

        waker_idx = core.thread_wakers.len();
        core.thread_wakers.push(None);
    }

    poll_fn(move |ctxt| {
        loop {
            // Fetch a program to poll from the core: if all the programs are complete, then stop
            let (next_process, next_process_idx) = {
                // Acquire the core
                let mut core = unlocked_core.lock().unwrap();

                if core.stopped {
                    // The scene always stops running immediately when 'stopped' is true
                    return Poll::Ready(());
                }

                if core.next_subprogram == 0 && core.sub_programs.iter().all(|program| program.is_none()) && core.awake_processes.is_empty() {
                    // The scene is finished when there are no running programs left in it
                    return Poll::Ready(());
                }

                // Read the index of an awake program to poll (or return pending if there are no pending programs)
                let next_process_idx = core.awake_processes.pop_front();
                let next_process_idx = if let Some(next_process_idx) = next_process_idx { 
                    next_process_idx 
                } else {
                    // Store a waker for this thread
                    let waker = ctxt.waker().clone();
                    core.thread_wakers[waker_idx] = Some(waker);

                    // Get the core to check if the scene has become idle (if requested)
                    mem::drop(core);
                    if SceneCore::check_if_idle(&unlocked_core) {
                        // The core has queued an idle request: continue to evaluate it
                        // The core is idle if all the input streams are waiting and have no messages in them, plus the idle request flag is set
                        continue;
                    }

                    // Wait for a subprogram to wake us
                    return Poll::Pending;
                };

                if let Some(Some(next_process)) = core.processes.get_mut(next_process_idx) {
                    if next_process.is_awake && next_process.future.is_waiting() {
                        // Process is awake: we say it's asleep again at the start of the polling process
                        next_process.is_awake = false;

                        // We borrow the future while we poll it to take ownership of it (it gets put back once we're done)
                        (next_process.future.take(), next_process_idx)
                    } else {
                        // Process is not awake (eg, because it's being polled by another thread), so don't wake up
                        (None, next_process_idx)
                    }
                } else {
                    // Process has been killed so can't be woken up
                    (None, next_process_idx)
                }
            };

            if let Some(next_process) = next_process {
                // The next process is awake and ready to poll: create a waker to reawaken it when we're done
                let process_waker       = waker(Arc::new(SceneCoreWaker::with_core(&unlocked_core, next_process_idx)));
                let mut process_context = Context::from_waker(&process_waker);

                // Poll the process in the new context
                let mut next_process    = next_process;
                let poll_result         = next_process.poll_unpin(&mut process_context);

                if poll_result.is_pending() {
                    // Put the process back into the pending list
                    let mut core        = unlocked_core.lock().unwrap();
                    let process_data    = core.processes[next_process_idx].as_mut().expect("Process should not go away while we're polling it");

                    process_data.future = SceneProcessFuture::Waiting(next_process);
                    process_data.unpark_when_waiting.drain(..).for_each(|thread| thread.unpark());

                    if process_data.is_awake {
                        // Possible re-awoken while polling, so make sure the process is still in the pending list so it gets polled again
                        if !core.awake_processes.contains(&next_process_idx) {
                            core.awake_processes.push_back(next_process_idx);
                        }
                    }
                } else {
                    // This process has been terminated: remove it from the list
                    let mut core = unlocked_core.lock().unwrap();

                    core.processes[next_process_idx] = None;
                    core.next_process = core.next_process.min(next_process_idx);
                }
            }
        }
    })
}
