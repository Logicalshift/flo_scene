use crate::error::*;
use crate::filter::*;
use crate::output_sink::*;
use crate::input_stream::*;
use crate::programs::*;
use crate::scene::*;
use crate::scene_context::*;
use crate::scene_message::*;
use crate::stream_id::*;
use crate::stream_source::*;
use crate::stream_target::*;
use crate::subprogram_id::*;

use futures::prelude::*;
use futures::future::{BoxFuture, poll_fn};
use futures::task::{Poll, Waker, Context, waker, ArcWake};

use std::any::*;
use std::collections::*;
use std::sync::*;

///
/// A handle of a process running in a scene
///
/// (A process is just a future, a scene is essentially run as a set of concurrent futures that can be modified as needed)
///
#[derive(Copy, Clone, PartialEq, Eq)]
pub (crate) struct ProcessHandle(usize);

///
/// Data that's stored for an individual program.
///
/// Note that the scene core must be locked before the subprogram core, if the scene core needs to be locked.
///
pub (crate) struct SubProgramCore {
    /// The stream ID of the input stream to this subprogram
    input_stream_id: StreamId,

    /// The ID of this program
    id: SubProgramId,

    /// The output sink targets for this sub-program
    outputs: HashMap<StreamId, Arc<dyn Send + Sync + Any>>,

    /// The name of the expected input type of this program
    expected_input_type_name: &'static str,
}

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
/// Data associated with a process in a scene
///
struct SceneProcess {
    /// The future for this process (can be None while it's being polled by another thread)
    future: Option<BoxFuture<'static, ()>>,

    /// Set to true if this process has been woken up
    is_awake: bool,
}

///
/// The scene core is used to store the shared state for all scenes
///
pub (crate) struct SceneCore {
    /// The sub-programs that are active in this scene
    sub_programs: Vec<Option<Arc<Mutex<SubProgramCore>>>>,

    /// The message types where the 'initialise' routine has been called
    initialised_message_types: HashSet<TypeId>,

    /// The input stream cores for each sub-program
    sub_program_inputs: Vec<Option<Arc<dyn Send + Sync + Any>>>,

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

    /// True if this scene is stopped and shouldn't be run any more
    stopped: bool,

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
            thread_wakers:              vec![],
            stopped:                    false,
            updates:                    None,
        }
    }

    ///
    /// If a message type has not been initialised in a core, calls the initialisation function
    ///
    #[inline]
    pub (crate) fn initialise_message_type<TMessageType>(core: &Arc<Mutex<SceneCore>>)
    where
        TMessageType: 'static + SceneMessage,
    {
        // If the message type is not yet initialised, mark it as such and then call the initialisation function
        // TODO: if there are multiple threads dealing with the scene, the message type might be used 'uninitialised' for a brief while on other threads
        let type_id             = TypeId::of::<TMessageType>();
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
            TMessageType::initialise(&fake_scene);
        }
    }

    ///
    /// Adds a program to the list being run by this scene
    ///
    pub fn start_subprogram<TMessage>(core: &Arc<Mutex<SceneCore>>, program_id: SubProgramId, program: impl 'static + Send + Future<Output=()>, input_core: Arc<Mutex<InputStreamCore<TMessage>>>) -> Arc<Mutex<SubProgramCore>>
    where
        TMessage: 'static + SceneMessage,
    {
        use std::mem;

        Self::initialise_message_type::<TMessage>(core);

        let (subprogram, waker) = {
            let start_core      = Arc::downgrade(core);
            let process_core    = Arc::downgrade(&core);
            let mut core        = core.lock().unwrap();

            // next_subprogram should always indicate the handle we'll use for the new program (it should be either a None entry in the list or sub_programs.len())
            let handle = core.next_subprogram;

            // Create a place to send updates on the program's progress
            let update_sink = core.updates.as_ref().map(|(pid, sink_core)| OutputSink::attach(*pid, Arc::clone(sink_core)));

            // Start a process to run this subprogram
            let (_process_handle, waker) = core.start_process(async move {
                // Notify that the program is starting
                if let Some(core) = start_core.upgrade() {
                    // We use a background process to start because we might be blocking the program that reads the updates here
                    core.lock().unwrap().send_scene_updates(vec![SceneUpdate::Started(program_id)]);
                }
                mem::drop(start_core);

                // Wait for the program to run
                program.await;

                // Notify that the program has finished
                if let Some(mut update_sink) = update_sink {
                    update_sink.send(SceneUpdate::Stopped(program_id)).await.ok();
                }

                // Close down the subprogram before finishing
                if let Some(core) = process_core.upgrade() {
                    let mut core = core.lock().unwrap();

                    core.sub_programs[handle]       = None;
                    core.sub_program_inputs[handle] = None;
                    core.next_subprogram            = core.next_subprogram.min(handle);
                }
            });

            // Create the sub-program data
            let subprogram = SubProgramCore {
                id:                         program_id.clone(),
                input_stream_id:            StreamId::with_message_type::<TMessage>(),
                outputs:                    HashMap::new(),
                expected_input_type_name:   type_name::<TMessage>(),
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
            core.sub_program_inputs[handle] = Some(input_core);
            core.program_indexes.insert(program_id.clone(), handle);

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

        // Result is the subprogram
        subprogram
    }

    ///
    /// Creates the 'scene update' stream for a particular program
    ///
    pub (crate) fn set_scene_update_from(core: &Arc<Mutex<SceneCore>>, source: SubProgramId) {
        // Get the subprogram for the stream
        let subprogram = {
            let core                = core.lock().unwrap();
            let subprogram_handle   = core.program_indexes.get(&source).copied();

            if let Some(subprogram_handle) = subprogram_handle {
                if let Some(subprogram) = core.sub_programs.get(subprogram_handle) {
                    subprogram.as_ref().cloned()
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some(subprogram) = subprogram {
            // Create a context for that subprogram
            let context     = SceneContext::new(&core, &subprogram);
            let update_sink = context.send::<SceneUpdate>(StreamTarget::None).unwrap();

            core.lock().unwrap().set_update_core(source, update_sink.core());
        }
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

        if (**target_input).type_id() != expected_message_type {
            let stream_type     = stream_id.message_type_name();
            let program_type    = self.sub_programs[handle].as_ref().unwrap().lock().unwrap().expected_input_type_name.to_string();

            Err(ConnectionError::WrongInputType(SourceStreamMessageType(stream_type), TargetInputMessageType(program_type)))
        } else {
            Ok(Arc::clone(target_input))
        }
    }

    ///
    /// Sends a set of updates to the update stream (if there is one set for this core)
    ///
    pub (crate) fn send_scene_updates(&mut self, updates: Vec<SceneUpdate>) {
        if updates.len() == 0 {
            return;
        }

        if let Some((pid, update_core)) = self.updates.as_ref() {
            let mut update_sink = OutputSink::attach(*pid, Arc::clone(update_core));

            self.start_process(async move {
                for update in updates {
                    update_sink.send(update).await.ok();
                }
            });
        }
    }

    ///
    /// Adds or updates a program connection in this core
    ///
    pub (crate) fn connect_programs(core: &Arc<Mutex<SceneCore>>, source: StreamSource, target: StreamTarget, stream_id: StreamId) -> Result<(), ConnectionError> {
        // Retrieve the result
        let result = SceneCore::finish_connecting_programs(core, source.clone(), target.clone(), stream_id.clone());

        // Send an update if there's an error
        if let Err(err) = &result {
            let update      = SceneUpdate::FailedConnection(err.clone(), source, target, stream_id);
            let mut core    = core.lock().unwrap();

            core.send_scene_updates(vec![update]);
        }

        result
    }

    ///
    /// Finishes a program connection, sending updates if successful
    ///
    fn finish_connecting_programs(core: &Arc<Mutex<SceneCore>>, source: StreamSource, target: StreamTarget, stream_id: StreamId) -> Result<(), ConnectionError> {
        // Create a function to reconnect a subprogram
        let reconnect_subprogram: Box<dyn Fn(&Arc<Mutex<SubProgramCore>>) -> Option<Waker>> = match &target {
            StreamTarget::None                  => Box::new(|sub_program| sub_program.lock().unwrap().discard_output_from(&stream_id)),
            StreamTarget::Any                   => Box::new(|sub_program| sub_program.lock().unwrap().disconnect_output_sink(&stream_id)),

            StreamTarget::Program(subprogid)    => {
                let mut core        = core.lock().unwrap();
                let stream_id       = &stream_id;
                let target_input    = core.get_target_input(subprogid, stream_id)?;

                Box::new(move |sub_program| sub_program.lock().unwrap().reconnect_output_sinks(&target_input, stream_id, false))
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

        for maybe_sub_program in sub_programs.iter() {
            if let Some(sub_program) = maybe_sub_program {
                // Update the streams of the subprogram
                let sub_program_id = sub_program.lock().unwrap().id;

                if source.matches_subprogram(&sub_program_id) {
                    // Reconnect the program
                    let waker = reconnect_subprogram(&sub_program);

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
        }

        // Send the updates on how the connections have changed
        core.lock().unwrap().send_scene_updates(scene_updates);

        Ok(())
    }

    ///
    /// Creates an InputStreamCore that reads the input type of a filter, and outputs to the input core of a program with the output type of the filter (if the types all match up)
    ///
    fn filtered_input_for_program(core: &Arc<Mutex<SceneCore>>, source_program: SubProgramId, filter_handle: FilterHandle, target_program: SubProgramId) -> Result<Arc<dyn Send + Sync + Any>, ConnectionError> {
        // Fetch the input core for the target program
        let target_input_core = {
            let core            = core.lock().unwrap();

            let target_index    = core.program_indexes.get(&target_program).ok_or(ConnectionError::TargetNotInScene)?;
            let target_core     = (&core.sub_program_inputs.get(*target_index)).ok_or(ConnectionError::TargetNotInScene)?.as_ref().ok_or(ConnectionError::TargetNotInScene)?;

            Arc::clone(target_core)
        };

        // Create an input stream core to use with it
        filter_handle.create_input_stream_core(core, source_program, target_input_core)
    }

    ///
    /// Returns the output sink target configured for a particular stream
    ///
    pub (crate) fn sink_for_target<TMessageType>(scene_core: &Arc<Mutex<SceneCore>>, source: &SubProgramId, target: StreamTarget) -> Result<OutputSinkTarget<TMessageType>, ConnectionError>
    where
        TMessageType: 'static + SceneMessage,
    {
        use std::mem;

        Self::initialise_message_type::<TMessageType>(scene_core);

        match target {
            StreamTarget::None  |
            StreamTarget::Any   => {
                // Return the general stream for the message type, if there is one. 'None' connections will default to 'None', but 'Any' connections will use the default connection for the message type.
                let core                = scene_core.lock().unwrap();
                let maybe_connection    = match target {
                    StreamTarget::None  => core.connections
                        .get(&(source.into(), StreamId::with_message_type::<TMessageType>())).cloned()
                        .or_else(|| core.connections.get(&(StreamSource::All, StreamId::with_message_type::<TMessageType>())).cloned()),

                    _                   => core.connections
                        .get(&(source.into(), StreamId::with_message_type::<TMessageType>())).cloned()
                        .or_else(|| core.connections.get(&(StreamSource::All, StreamId::with_message_type::<TMessageType>())).cloned())
                        .or_else(|| Some(TMessageType::default_target()))
                };

                if let Some(connection) = maybe_connection {
                    // Stream is connected to a specific program (or is configured to discard its input)
                    match connection {
                        StreamTarget::None                          => Ok(OutputSinkTarget::Discard),
                        StreamTarget::Any                           => Ok(OutputSinkTarget::Disconnected),
                        StreamTarget::Program(target_program_id)    => {
                            // Connect the stream to the input of a specific program
                            let target_program_handle   = core.program_indexes.get(&target_program_id).ok_or(ConnectionError::TargetNotInScene)?;
                            let target_program_input    = core.sub_program_inputs.get(*target_program_handle).ok_or(ConnectionError::TargetNotInScene)?.clone().ok_or(ConnectionError::TargetNotInScene)?;
                            let target_input_type       = core.sub_programs[*target_program_handle].as_ref().unwrap().lock().unwrap().expected_input_type_name.to_string();
                            let target_program_input    = target_program_input.downcast::<Mutex<InputStreamCore<TMessageType>>>()
                                .or_else(move |_| Err(ConnectionError::WrongInputType(SourceStreamMessageType(type_name::<TMessageType>().to_string()), TargetInputMessageType(target_input_type))))?;

                            Ok(OutputSinkTarget::Input(Arc::downgrade(&target_program_input)))
                        },
                        StreamTarget::Filtered(filter_handle, target_program_id) => {
                            // Create a stream that is processed through a filter (note that this creates a process that will need to be terminated by closing the input stream)
                            let filter_handle       = filter_handle;
                            let target_program_id   = target_program_id;
                            mem::drop(core);

                            let filtered_input_core = Self::filtered_input_for_program(scene_core, *source, filter_handle, target_program_id)?;
                            let filtered_input_core = filtered_input_core.downcast::<Mutex<InputStreamCore<TMessageType>>>()
                                .or(Err(ConnectionError::FilterInputDoesNotMatch))?;

                            Ok(OutputSinkTarget::CloseWhenDropped(Arc::downgrade(&filtered_input_core)))
                        },
                    }
                } else {
                    // Stream is not connected, either use a discard or a disconnected stream
                    match target {
                        StreamTarget::None  => Ok(OutputSinkTarget::Discard),
                        StreamTarget::Any   => Ok(OutputSinkTarget::Disconnected),
                        _                   => Err(ConnectionError::UnexpectedConnectionType)
                    }
                }
            },

            StreamTarget::Program(target_program_id) => {
                // The connections can define a redirect stream by using a StreamId target
                let core                        = scene_core.lock().unwrap();
                let (filter, target_program_id) = core.connections.get(&(source.into(), StreamId::for_target::<TMessageType>(&target_program_id)))
                    .or_else(|| core.connections.get(&(StreamSource::All, StreamId::for_target::<TMessageType>(&target_program_id))))
                    .and_then(|target| {
                        match target {
                            StreamTarget::Program(program_id)           => Some((None, *program_id)),
                            StreamTarget::Filtered(filter, program_id)  => Some((Some(*filter), *program_id)),
                            _                                           => None,
                        }
                    })
                    .unwrap_or((None, target_program_id));

                if let Some(filter) = filter {
                    // Create a filtered connection to this program
                    mem::drop(core);

                    let filtered_input_core = Self::filtered_input_for_program(scene_core, *source, filter, target_program_id)?;
                    let filtered_input_core = filtered_input_core.downcast::<Mutex<InputStreamCore<TMessageType>>>()
                        .or(Err(ConnectionError::FilterInputDoesNotMatch))?;

                    Ok(OutputSinkTarget::CloseWhenDropped(Arc::downgrade(&filtered_input_core)))
                } else {
                    // Attempt to find the target stream for this specific program to create a direct connection
                    // TODO: if the program hasn't started yet, we should create a disconnected stream and connect it later on
                    let target_program_handle   = core.program_indexes.get(&target_program_id).ok_or(ConnectionError::TargetNotInScene)?;
                    let target_program_input    = core.sub_program_inputs.get(*target_program_handle).ok_or(ConnectionError::TargetNotInScene)?.clone().ok_or(ConnectionError::TargetNotInScene)?;
                    let target_input_type       = core.sub_programs[*target_program_handle].as_ref().unwrap().lock().unwrap().expected_input_type_name.to_string();
                    let target_program_input    = target_program_input.downcast::<Mutex<InputStreamCore<TMessageType>>>()
                        .or_else(move |_| Err(ConnectionError::WrongInputType(SourceStreamMessageType(type_name::<TMessageType>().to_string()), TargetInputMessageType(target_input_type))))?;

                    Ok(OutputSinkTarget::Input(Arc::downgrade(&target_program_input)))
                }
            },

            StreamTarget::Filtered(filter_handle, target_program_id) => {
                // The connections can define a redirect stream by using a StreamId target
                let core                = scene_core.lock().unwrap();
                let target_program_id   = core.connections.get(&(source.into(), StreamId::for_target::<TMessageType>(&target_program_id)))
                    .or_else(|| core.connections.get(&(StreamSource::All, StreamId::for_target::<TMessageType>(&target_program_id))))
                    .and_then(|target| {
                        match target {
                            StreamTarget::Program(program_id)       => Some(program_id.clone()),
                            StreamTarget::Filtered(_, program_id)   => Some(*program_id),
                            _                                       => None,
                        }
                    })
                    .unwrap_or(target_program_id);

                // Create a stream that is processed through a filter (note that this creates a process that will need to be terminated by closing the input stream)
                mem::drop(core);
                let filtered_input_core = Self::filtered_input_for_program(scene_core, *source, filter_handle, target_program_id)?;
                let filtered_input_core = filtered_input_core.downcast::<Mutex<InputStreamCore<TMessageType>>>()
                    .or(Err(ConnectionError::FilterInputDoesNotMatch))?;

                Ok(OutputSinkTarget::CloseWhenDropped(Arc::downgrade(&filtered_input_core)))
            }
        }
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
            future:     Some(process.boxed()),
            is_awake:   true,
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
            input_stream_core.clone()
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
    pub (crate) fn try_create_output_target<TMessageType>(&mut self, id: &StreamId, new_output_target: OutputSinkTarget<TMessageType>) -> Result<Arc<Mutex<OutputSinkCore<TMessageType>>>, Arc<Mutex<OutputSinkCore<TMessageType>>>>
    where
        TMessageType: 'static + SceneMessage,
    {
        let existing_target = self.outputs.get(id);
        if let Some(existing_target) = existing_target {
            // Return the already existing target
            let existing_target = Arc::clone(existing_target);
            let existing_target = existing_target.downcast::<Mutex<OutputSinkCore<TMessageType>>>().unwrap();

            Err(existing_target)
        } else {
            // Store a new target in the outputs
            let new_core    = OutputSinkCore::new(new_output_target);
            let new_core    = Arc::new(Mutex::new(new_core));
            let cloned_core = Arc::clone(&new_core);
            self.outputs.insert(id.clone(), cloned_core);

            // Use the new target for the output stream
            Ok(new_core)
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

impl SceneCoreWaker {
    ///
    /// Creates a waker for a scene core
    ///
    pub fn with_core(core: Arc<Mutex<SceneCore>>, process_id: usize) -> Self {
        Self {
            core:       Arc::downgrade(&core),
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
    let core = Arc::clone(core);

    // Choose a location to store the waker for this core instance
    let waker_idx;
    {
        let mut core = core.lock().unwrap();

        waker_idx = core.thread_wakers.len();
        core.thread_wakers.push(None);
    }

    poll_fn(move |ctxt| {
        loop {
            // Fetch a program to poll from the core: if all the programs are complete, then stop
            let (next_process, next_process_idx) = {
                // Acquire the core
                let mut core = core.lock().unwrap();

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

                    // Wait for a subprogram to wake us
                    return Poll::Pending;
                };

                if let Some(Some(next_process)) = core.processes.get_mut(next_process_idx) {
                    if next_process.is_awake && next_process.future.is_some() {
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
                let process_waker       = waker(Arc::new(SceneCoreWaker::with_core(Arc::clone(&core), next_process_idx)));
                let mut process_context = Context::from_waker(&process_waker);

                // Poll the process in the new context
                let mut next_process    = next_process;
                let poll_result         = next_process.poll_unpin(&mut process_context);

                if let Poll::Pending = poll_result {
                    // Put the process back into the pending list
                    let mut core        = core.lock().unwrap();
                    let process_data    = core.processes[next_process_idx].as_mut().expect("Process should not go away while we're polling it");

                    process_data.future = Some(next_process);

                    if process_data.is_awake {
                        // Possible re-awoken while polling, so make sure the process is still in the pending list so it gets polled again
                        if !core.awake_processes.contains(&next_process_idx) {
                            core.awake_processes.push_back(next_process_idx);
                        }
                    }
                } else {
                    // This process has been terminated: remove it from the list
                    let mut core = core.lock().unwrap();

                    core.processes[next_process_idx] = None;
                    core.next_process = core.next_process.min(next_process_idx);
                }
            }
        }
    })
}
