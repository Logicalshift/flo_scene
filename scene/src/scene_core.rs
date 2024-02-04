use crate::error::*;
use crate::filter::*;
use crate::output_sink::*;
use crate::input_stream::*;
use crate::process_core::*;
use crate::programs::*;
use crate::scene::*;
use crate::scene_context::*;
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
    sub_program_inputs: Vec<Option<(StreamId, Arc<dyn Send + Sync + Any>)>>,

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
    pub fn start_subprogram<TMessage>(scene_core: &Arc<Mutex<SceneCore>>, program_id: SubProgramId, program: impl 'static + Send + Future<Output=()>, input_core: Arc<Mutex<InputStreamCore<TMessage>>>) -> Arc<Mutex<SubProgramCore>>
    where
        TMessage: 'static + SceneMessage,
    {
        use std::mem;

        Self::initialise_message_type::<TMessage>(scene_core);

        let (subprogram, waker) = {
            let start_core      = Arc::downgrade(scene_core);
            let process_core    = Arc::downgrade(scene_core);
            let mut core        = scene_core.lock().unwrap();

            // next_subprogram should always indicate the handle we'll use for the new program (it should be either a None entry in the list or sub_programs.len())
            let handle = core.next_subprogram;

            // Create a place to send updates on the program's progress
            let update_sink = core.updates.as_ref().map(|(pid, sink_core)| OutputSink::attach(*pid, Arc::clone(sink_core), scene_core));

            // Start a process to run this subprogram
            let (process_handle, waker) = core.start_process(async move {
                // Notify that the program is starting
                if let Some(core) = start_core.upgrade() {
                    // We use a background process to start because we might be blocking the program that reads the updates here
                    SceneCore::send_scene_updates(&core, vec![SceneUpdate::Started(program_id)]);
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
                }
            });

            // Create the sub-program data
            let subprogram = SubProgramCore {
                id:                         program_id,
                process_id:                 Some(process_handle),
                input_stream_id:            StreamId::with_message_type::<TMessage>(),
                outputs:                    HashMap::new(),
                output_high_water:          0,
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
            core.sub_program_inputs[handle] = Some((StreamId::with_message_type::<TMessage>(), input_core));
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
            let context     = SceneContext::new(core, &subprogram);
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

        if (*(*target_input).1).type_id() != expected_message_type {
            let stream_type     = stream_id.message_type_name();
            let program_type    = self.sub_programs[handle].as_ref().unwrap().lock().unwrap().expected_input_type_name.to_string();

            Err(ConnectionError::WrongInputType(SourceStreamMessageType(stream_type), TargetInputMessageType(program_type)))
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

        let mut core    = scene_core.lock().unwrap();
        let mut waker   = None;
        if let Some((pid, update_core)) = core.updates.as_ref() {
            let mut update_sink = OutputSink::attach(*pid, Arc::clone(update_core), scene_core);

            let (_, process_waker) = core.start_process(async move {
                for update in updates {
                    update_sink.send(update).await.ok();
                }
            });

            waker = process_waker;
        }

        mem::drop(core);
        if let Some(waker) = waker {
            waker.wake()
        }
    }

    ///
    /// Adds or updates a program connection in this core
    ///
    pub (crate) fn connect_programs(core: &Arc<Mutex<SceneCore>>, source: StreamSource, target: StreamTarget, stream_id: StreamId) -> Result<(), ConnectionError> {
        // Call finish_connecting_programs to generate the result
        let result = SceneCore::finish_connecting_programs(core, source.clone(), target.clone(), stream_id.clone());

        // If successful and the target is a filter, connect the specific stream for the target as well as the 'all' stream
        if result.is_ok() {
            if let StreamTarget::Filtered(_, target_program_id) = target {
                if stream_id.target_program().is_none() {
                    SceneCore::finish_connecting_programs(core, source.clone(), target.clone(), stream_id.for_target(target_program_id)).ok();
                }
            }
        }

        // Send an update if there's an error
        if let Err(err) = &result {
            let update      = SceneUpdate::FailedConnection(err.clone(), source, target, stream_id);
            SceneCore::send_scene_updates(core, vec![update]);
        }

        result
    }

    ///
    /// Finishes a program connection, sending updates if successful
    ///
    #[allow(clippy::type_complexity)]   // Creating a type for reconnect_subprogram just looks super goofy and is a lifetime nightmare
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

        for sub_program in sub_programs.iter().flatten() {
            // Update the streams of the subprogram
            let sub_program_id = sub_program.lock().unwrap().id;

            if source.matches_subprogram(&sub_program_id) {
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
            let target_core     = core.sub_program_inputs.get(*target_index).ok_or(ConnectionError::TargetNotInScene)?.as_ref().ok_or(ConnectionError::TargetNotInScene)?;

            Arc::clone(&target_core.1)
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
                            let target_program_input    = target_program_input.1.downcast::<Mutex<InputStreamCore<TMessageType>>>()
                                .map_err(move |_| ConnectionError::WrongInputType(SourceStreamMessageType(type_name::<TMessageType>().to_string()), TargetInputMessageType(target_input_type)))?;

                            Ok(OutputSinkTarget::Input(Arc::downgrade(&target_program_input)))
                        },
                        StreamTarget::Filtered(filter_handle, target_program_id) => {
                            // Create a stream that is processed through a filter (note that this creates a process that will need to be terminated by closing the input stream)
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
                let (filter, target_program_id) = core.connections.get(&(source.into(), StreamId::with_message_type::<TMessageType>().for_target(&target_program_id)))
                    .or_else(|| core.connections.get(&(StreamSource::All, StreamId::with_message_type::<TMessageType>().for_target(&target_program_id))))
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
                    let target_program_input    = target_program_input.1.downcast::<Mutex<InputStreamCore<TMessageType>>>()
                        .map_err(move |_| ConnectionError::WrongInputType(SourceStreamMessageType(type_name::<TMessageType>().to_string()), TargetInputMessageType(target_input_type)))?;

                    Ok(OutputSinkTarget::Input(Arc::downgrade(&target_program_input)))
                }
            },

            StreamTarget::Filtered(filter_handle, target_program_id) => {
                // The connections can define a redirect stream by using a StreamId target
                let core                = scene_core.lock().unwrap();
                let target_program_id   = core.connections.get(&(source.into(), StreamId::with_message_type::<TMessageType>().for_target(&target_program_id)))
                    .or_else(|| core.connections.get(&(StreamSource::All, StreamId::with_message_type::<TMessageType>().for_target(&target_program_id))))
                    .and_then(|target| {
                        match target {
                            StreamTarget::Program(program_id)       => Some(*program_id),
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
    pub (crate) fn steal_thread_for_program(core: &Arc<Mutex<SceneCore>>, program_id: SubProgramId) -> Result<(), SceneSendError> {
        use std::mem;
        use std::thread;

        // Fetch the program whose process we're going to run
        let subprogram = {
            let core    = core.lock().unwrap();

            core.program_indexes.get(&program_id)
                .and_then(|idx| core.sub_programs.get(*idx).cloned())
                .unwrap_or(None)
        };

        let subprogram = subprogram.ok_or(SceneSendError::TargetProgramEnded)?;

        // Try to fetch the future for the process (back to the scene core again here)
        let (process_id, mut process_future) = {
            // Loop until the process future is available on this thread
            loop {
                // We lock both the core and the subprogram here so that the process cannot end before we get the future
                let mut core    = core.lock().unwrap();
                let process_id  = subprogram.lock().unwrap().process_id.ok_or(SceneSendError::TargetProgramEnded)?;

                let process     = core.processes.get_mut(process_id.0)
                    .map(|process| process.as_mut())
                    .unwrap_or(None)
                    .ok_or(SceneSendError::TargetProgramEnded)?;

                // If the process is available, return it to the rest of the thread, in order to run it here
                match process.future.take() {
                    Some(future)    => { break (process_id.0, future); }
                    None            => {
                        if process.future.is_running_on_this_thread() {
                            // Error if we can't steal the thread because the process is already running on this thread
                            return Err(SceneSendError::CannotReEnterTargetProgram);
                        }

                        // Release the core and park the thread until the process stops running on the target thread
                        process.unpark_when_waiting.push(thread::current());
                        mem::drop(core);
                        thread::park();
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
            .all(|(stream_id, input_stream)| stream_id.is_idle(input_stream) == Ok(true));

        // If all inputs are idle and the core is still in 'notification' mode, notify the waiting messages
        if all_inputs_idle {
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
                .map(|(idx, notifier)| (idx, notifier.take()))
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
                        // TODO: if we ever need to create/dispose many of these, it'll make more sense to 
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
                if let Some(notifier) = notifier {
                    locked_core.when_idle[idx] = Some(notifier);
                }
            }

            // Indicate to the caller that a notification occurred
            true
        } else {
            false
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
