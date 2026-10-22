use crate::host::connect_result::*;
use crate::host::connectome::*;
use crate::host::error::*;
use crate::host::filter::*;
use crate::host::output_sink::*;
use crate::host::input_stream::*;
use crate::host::process_core::*;
use crate::host::programs::*;
use crate::host::scene::*;
use crate::host::scene_message::*;
use crate::host::stream_id::*;
use crate::host::stream_source::*;
use crate::host::stream_target::*;
use crate::host::subprogram_core::*;
use crate::host::subprogram_id::*;
use crate::host::thread_stealer::*;

use futures::prelude::*;
use futures::future::{poll_fn};
use futures::task::{Poll, Waker, Context, waker, ArcWake};
use futures::channel::mpsc;

use std::any::*;
use std::collections::*;
use std::sync::*;
use std::sync::atomic::{AtomicUsize};
use std::panic::{catch_unwind, UnwindSafe};

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
/// The possible states of a scene process
///
enum SceneProcessState {
    Free,
    Running(SceneProcess),
    Aborted,
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
    processes: Vec<SceneProcessState>,

    /// The next free process in this core
    next_process: usize,

    /// The processes that have been woken up since the core was last polled
    awake_processes: VecDeque<usize>,

    /// Wakers for the futures that are being used to run the scene (can be multiple if the scene is scheduled across a thread pool)
    thread_wakers: Vec<Option<Waker>>,

    /// Wakers for when a process stops
    pub (super) on_stop: Vec<Waker>,

    /// The connectome, which defines how programs are connected to one another
    connectome: Connectome,

    /// True if this scene is stopped and shouldn't be run any more
    stopped: bool,

    /// True if the next time the scene becomes entirely idle, we should notify one of the waiting streams
    notify_when_idle: bool,

    /// The current idle count (idle notifications include this)
    idle_count: usize,

    /// Streams to send on when the input streams and core all become idle (borrowed when we're waiting for them to send)
    when_idle: Vec<Option<mpsc::Sender<usize>>>,

    /// An output core where status updates are sent
    updates: Option<(SubProgramId, Arc<Mutex<OutputSinkCore<SceneUpdate>>>)>,

    /// Panics that the scene has received, in the order they were received
    pub (super) active_panics: Vec<Box<dyn 'static + Send + Any>>,
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
            connectome:                 Connectome::default(),
            thread_wakers:              vec![],
            on_stop:                    vec![],
            stopped:                    false,
            notify_when_idle:           false,
            idle_count:                 0,
            when_idle:                  vec![],
            updates:                    None,
            active_panics:              vec![],
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

        let (subprogram, waker, invalidated_streams) = {
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

                // Wait for the program to run
                program.await;

                // Wait for the background tasks to stop before the whole task is finished
                if let Some(scene_core) = start_core.upgrade() {
                    let sub_program_core = { scene_core.lock().unwrap().sub_programs[handle].clone() };

                    if let Some(subprogram_core) = sub_program_core {
                        SubProgramCore::wait_for_background_processes(&subprogram_core, &scene_core).await;
                    }
                }
                mem::drop(start_core);

                // Notify that the program has finished
                if let Some(mut update_sink) = update_sink {
                    update_sink.send(SceneUpdate::Stopped(program_id)).await.ok();
                }

                // Close down the subprogram before finishing
                if let Some(scene_core) = process_core.upgrade() {
                    let mut core = scene_core.lock().unwrap();

                    // Take the subprogram and input core out of the scene
                    let old_sub_program     = core.sub_programs[handle].take();
                    let old_input_core      = core.sub_program_inputs[handle].take();
                    core.program_indexes.remove(&program_id);
                    core.next_subprogram    = core.next_subprogram.min(handle);

                    // Remove from the connectome and retrieve the invalidated streams
                    let invalidated_streams = core.connectome.remove_subprogram(program_id);

                    // Drop in order: first release the core lock, then drop the subprograms (which may re-take it)
                    mem::drop(core);

                    if let Some(old_sub_program) = &old_sub_program {
                        old_sub_program.lock().unwrap().process_ids = vec![];
                        SubProgramCore::closedown(old_sub_program, &scene_core);
                    }

                    mem::drop(old_input_core);
                    mem::drop(old_sub_program);

                    // Reconnect any invalidated streams
                    Self::reconnect_invalidated_streams(&scene_core, invalidated_streams);

                    // Core might be idle now the program has finished
                    SceneCore::check_if_idle(&scene_core);
                }
            });

            // Create the sub-program data
            let subprogram = SubProgramCore {
                id:                         program_id,
                subprogram_handle:          handle,
                process_ids:                vec![process_handle],
                last_message_source:        None,
                input_stream_id:            StreamId::with_message_type::<TMessage>(),
                outputs:                    HashMap::new(),
                output_high_water:          0,
                expected_input_type_name:   type_name::<TMessage>(),
                next_command_sequence:      Arc::new(AtomicUsize::new(0)),
                parent_program:             None,
                child_programs:             HashSet::new(),
                data:                       HashMap::new(),
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

            // Add to the connectome and get the invalidated streams
            let invalidated = core.connectome.add_subprogram(program_id, StreamId::with_message_type::<TMessage>());

            (subprogram, waker, invalidated)
        };

        // Safe to wake the waker once the core lock is released
        if let Some(waker) = waker {
            waker.wake();
        }

        // If there are any pending connections that can be connected to this subprogram, reconnect them here
        Self::reconnect_invalidated_streams(scene_core, invalidated_streams);

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
            // Can't use this stream as it doesn't match stream_id
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
    /// Ensures that the source and target of a filter are initialised
    ///
    fn initialise_filter(core: &Arc<Mutex<SceneCore>>, filter_handle: FilterHandle) {
        if let Ok(source_stream) = filter_handle.source_stream_id_any() {
            Self::initialise_message_type(core, source_stream);
        }

        if let Ok(target_stream) = filter_handle.target_stream_id_any() {
            Self::initialise_message_type(core, target_stream);
        }
    }

    ///
    /// Adds or updates a program connection in this core
    ///
    pub (crate) fn connect_programs(core: &Arc<Mutex<SceneCore>>, source: StreamSource, target: StreamTarget, stream_id: StreamId) -> Result<ConnectionResult, ConnectionError> {
        // Make sure the target stream ID type  is initialised
        Self::initialise_message_type(core, stream_id.clone());

        if let StreamSource::Filtered(filter) = &source {
            Self::initialise_filter(core, filter.clone());
        }

        if let StreamTarget::Filtered(filter, _) = &target {
            Self::initialise_filter(core, filter.clone());
        }

        // Call the connectome to perform the connection
        let invalidated_streams = core.lock().unwrap().connectome.connect(&source, &target, stream_id.clone());

        let invalidated_streams = match invalidated_streams {
            Ok(streams) => streams,
            Err(err)    => {
                let update = SceneUpdate::FailedConnection(err.clone(), source, target, stream_id);
                SceneCore::send_scene_updates(core, vec![update]);
                return Err(err);
            }
        };

        // Reconnect any affected streams
        Self::reconnect_invalidated_streams(core, invalidated_streams);

        // Reconnect all subprograms with this kind of output sink
        // TODO: this iterates all streams in all programs which isn't great performance-wise
        let subprograms = core.lock().unwrap().sub_programs.iter().flatten().cloned().collect::<Vec<_>>();
        for subprogram in subprograms {
            if subprogram.lock().unwrap().output_streams().any(|(output_stream, _)| output_stream.message_type() == stream_id.message_type()) {
                SubProgramCore::reconnect_disconnected_outputs(&subprogram, core, &stream_id);
            }
        }

        // TODO: generate the other connection results (generates TargetNotReady if the connection doesn't have a target)
        Ok(ConnectionResult::Ready)
    }

    ///
    /// Invalidate and reconnect any streams affected by invalidation
    ///
    fn reconnect_invalidated_streams(scene_core: &Arc<Mutex<SceneCore>>, invalidated: InvalidatedStreams) {
        // Connect any disconnected sinks in the affected programs/streams
        let mut scene_updates   = vec![];

        for (source_program, stream) in invalidated.iter() {
            let Some(program)       = scene_core.lock().unwrap().get_sub_program(*source_program) else { continue };

            // Copy the output sink cores that reference this program
            let output_sink_cores   = program.lock().unwrap().outputs
                .iter()
                .filter(|(program_stream, _core)| stream.message_type() == program_stream.message_type())
                .map(|(program_stream, core)| (program_stream.clone(), core.clone()))
                .collect::<Vec<_>>();

            let updates = Self::reconnect_output_sinks(scene_core, *source_program, output_sink_cores);
            scene_updates.extend(updates);
        }

        // Notify the subscribers of any connections that have changed
        SceneCore::send_scene_updates(scene_core, scene_updates);
    }

    ///
    /// Reconnects a set of output sinks belonging to a source program to the targets indicated by the connectome
    ///
    /// Returns the wakers that need to be woken once the reconnection is complete, and the scene updates indicating any connections
    /// that have changed as a result of the reconnection (the caller should send these using `send_scene_updates()`)
    ///
    pub (crate) fn reconnect_output_sinks(scene_core: &Arc<Mutex<SceneCore>>, source_program: SubProgramId, output_sink_cores: Vec<(StreamId, Arc<dyn Send + Sync + Any>)>) -> Vec<SceneUpdate> {
        let mut wakers          = vec![];
        let mut scene_updates   = vec![];

        for (program_stream, core) in output_sink_cores {
            // Get the active target for this sink
            let old_program = program_stream.active_program_for_output_sink(&core);
            let target      = Ok::<_, ConnectionError>(program_stream.requested_target());

            // Try to reconnect it
            if let Ok(target) = target {
                let waker = program_stream.reconnect_output_sink(scene_core, &core, source_program, target);

                if let Ok(waker) = waker {
                    wakers.extend(waker);

                    // Generate an update if the target of the connection changed
                    let new_program = program_stream.active_program_for_output_sink(&core);

                    if new_program != old_program {
                        if let Some(new_program) = new_program {
                            scene_updates.push(SceneUpdate::Connected(source_program, new_program, program_stream.clone()));
                        } else {
                            scene_updates.push(SceneUpdate::Disconnected(source_program, program_stream.clone()));
                        }
                    }
                }
            }
        }

        for waker in wakers {
            waker.wake()
        }

        scene_updates
    }

    ///
    /// Retrieves the target for an output sink on a route
    ///
    fn sink_target_for_route<TMessageType: SceneMessage>(&mut self, source_program_id: SubProgramId, route: ConnectionRoute) -> Result<(OutputSinkTarget<TMessageType>, Option<Waker>), ConnectionError> {
        match route.endpoint() {
            ConnectionRoute::Filter(_, _)   |       // Filters can't be an endpoint, we treat them like 'None' if they show up because of a bug
            ConnectionRoute::None           => {
                // Output sink is disconnected
                Ok((OutputSinkTarget::Discard, None))
            }

            ConnectionRoute::Hold => {
                // Disconnect the output for this sink
                Ok((OutputSinkTarget::Disconnected, None))
            },

            ConnectionRoute::DeliverTo(target_program) => {
                // The target stream will be the output of the last filter
                let filters = route.filters().collect::<Vec<_>>();
                let stream  = filters
                    .last().map(|last_filter| last_filter.target_stream_id_any())
                    .unwrap_or_else(|| Ok(StreamId::with_message_type::<TMessageType>()))?;

                // Start with the input for the target program
                let mut target_input        = self.get_target_input(&target_program, &stream)?;
                let mut close_when_dropped  = false;

                // Apply filters in reverse to get the core to connect
                let mut wakers = vec![];

                for filter in filters.into_iter().rev() {
                    let (input, waker)  = filter.create_input_stream_core(self, source_program_id, target_input)?;
                    target_input        = input;
                    close_when_dropped  = true;

                    wakers.extend(waker);
                }

                // Create a waker from those returned by the filter
                let waker = if wakers.is_empty() {
                    None
                } else if wakers.len() == 1 {
                    wakers.pop()
                } else {
                    struct WakeAll(Mutex<Vec<Waker>>);

                    impl ArcWake for WakeAll {
                        fn wake_by_ref(arc_self: &Arc<Self>) {
                            for waker in arc_self.0.lock().unwrap().drain(..) {
                                waker.wake()
                            }
                        }
                    }

                    Some(waker(Arc::new(WakeAll(Mutex::new(wakers)))))
                };

                let input_core = target_input.downcast().unwrap();
                if close_when_dropped {
                    Ok((OutputSinkTarget::CloseWhenDropped(Arc::downgrade(&input_core)), waker))
                } else {
                    Ok((OutputSinkTarget::Input(Arc::downgrade(&input_core)), waker))
                }
            },
        }
    }

    ///
    /// Returns the output sink target configured for a particular stream
    ///
    pub (crate) fn sink_for_target<TMessageType>(&mut self, source: &SubProgramId, target: StreamTarget) -> Result<(OutputSinkTarget<TMessageType>, Option<Waker>), ConnectionError>
    where
        TMessageType: 'static + SceneMessage,
    {
        let stream  = StreamId::with_message_type::<TMessageType>();
        let route   = self.connectome.route_for(&stream, *source, &target);

        match route.and_then(|route| self.sink_target_for_route(*source, route)) {
            Ok((target, waker))                         => Ok((target, waker)),
            Err(ConnectionError::WrongInputType(_, _))  => Ok((OutputSinkTarget::Disconnected, None)),
            Err(ConnectionError::TargetNotInScene)      => Ok((OutputSinkTarget::Disconnected, None)),
            Err(other)                                  => Err(other)
        }
    }

    ///
    /// Starts a new process running in this scene
    ///
    pub (crate) fn start_process(&mut self, process: impl 'static + Send + Future<Output=()>) -> (ProcessHandle, Option<Waker>) {
        // Assign a process ID to this process (first free process, or an entirely new slot)
        let mut process_id = self.next_process;
        while process_id < self.processes.len() && !self.processes[process_id].is_free() { process_id += 1; }
        while self.processes.len() <= process_id {
            self.processes.push(SceneProcessState::Free);
        }

        // Update the next process ID
        self.next_process += 1;
        while self.next_process < self.processes.len() && !self.processes[self.next_process].is_free() {
            self.next_process += 1;
        }

        // Store the new process
        let new_process = SceneProcess {
            future:                 SceneProcessFuture::Waiting(process.boxed()),
            is_awake:               true,
            unpark_when_waiting:    vec![],
        };
        self.processes[process_id] = SceneProcessState::Running(new_process);

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
                let process_id  = subprogram.lock().unwrap().process_ids.get(0).copied().ok_or(SceneSendError::TargetProgramEndedBeforeReady)?;

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
                scene_core.processes[process_id]    = SceneProcessState::Free;
                scene_core.next_process             = process_id.min(scene_core.next_process);
                scene_core.awake_processes.retain(|pid| pid != &process_id);

                None
            } else {
                // Process still running: return the future so that it'll actually run
                let process = scene_core.processes[process_id].as_mut();
                if let Some(process) = process {
                    process.future = SceneProcessFuture::Waiting(process_future);

                    // Wake any threads that were waiting for this process
                    process.unpark_when_waiting.drain(..).for_each(|thread| thread.unpark());

                    // If the future has woken up since the poll finished, then re-awaken the scene using a scene waker
                    if scene_core.processes[process_id].as_mut().map(|process| process.is_awake) == Some(true) {
                        Some(waker(Arc::new(SceneCoreWaker::with_core(core, process_id))))
                    } else {
                        None
                    }
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
    pub (crate) fn send_idle_notifications_to(core: &Arc<Mutex<SceneCore>>, notifier: mpsc::Sender<usize>) {
        let mut core = core.lock().unwrap();

        core.when_idle.push(Some(notifier));
    }

    ///
    /// The core will send a notification next time it's idle
    ///
    /// The returned value is the idle count: notifications should be sent with this set to a higher value
    ///
    pub (crate) fn notify_on_next_idle(core: &Arc<Mutex<SceneCore>>) -> usize {
        let mut core        = core.lock().unwrap();
        let idle_count      = core.idle_count;

        // Specify that we should notify when idle, and increase the idle count to avoid a potential race where an 'old' idle notification might interfere with this one
        core.notify_when_idle   = true;
        core.idle_count         += 1;

        idle_count
    }

    ///
    /// Checks if the core needs to signal that it's idle, and does so if necessary
    ///
    pub (crate) fn check_if_idle(core: &Arc<Mutex<SceneCore>>) -> bool {
        use std::mem;

        // Fetch the active input streams from the core (or stop, if we're not notifying)
        let idle_count;
        let sub_program_inputs = {
            let mut core = core.lock().unwrap();

            if !core.notify_when_idle {
                // Give up quickly if the core is not waiting for a notification
                return false;
            }

            // Increase the idle count so that future notifications have a different value
            core.idle_count += 1;
            idle_count = core.idle_count;

            core.sub_program_inputs
                .iter()
                .flatten()
                .cloned()
                .collect::<Vec<_>>()
        };

        // Check the inputs to see if they are idle or if everything can be unblocked
        enum InputState {
            Idle,
            CanBeUnblocked(Vec<(StreamId, Arc<dyn Any + Send + Sync>)>),
            Busy
        }

        let inputs_state = sub_program_inputs.iter()
            .fold(InputState::Idle, |state, (stream_id, input_stream, _program_id)| {
                match state {
                    InputState::Idle => {
                        if stream_id.is_idle(input_stream) == Ok(true) {
                            InputState::Idle
                        } else if stream_id.can_be_unblocked(input_stream) == Ok(true) {
                            InputState::CanBeUnblocked(vec![(stream_id.clone(), input_stream.clone())])
                        } else {
                            InputState::Busy
                        }
                    }
                    InputState::CanBeUnblocked(mut streams) => {
                        if stream_id.is_idle(input_stream) == Ok(true) {
                            InputState::CanBeUnblocked(streams)
                        } else if stream_id.can_be_unblocked(input_stream) == Ok(true) {
                            streams.push((stream_id.clone(), input_stream.clone()));
                            InputState::CanBeUnblocked(streams)
                        } else {
                            InputState::Busy
                        }
                    }
                    InputState::Busy => InputState::Busy,
                }
            });

        let all_inputs_idle = matches!(inputs_state, InputState::Idle);

        if let InputState::CanBeUnblocked(streams) = inputs_state {
            // All streams can be unblocked to get to an idle state (ie, whatever situation they were waiting on has not resolved itself)
            // (Would be 'Busy' if any of the input streams were still working on things)
            for (stream_id, input_core) in streams {
                stream_id.unblock(&input_core).ok();
            }
        }

        let all_processes_idle = if !all_inputs_idle {
            // If the inputs are not idle, then the core is not idle (we'll just assume that the processes are asleep)
            // (The scene is not idle until all the processes are asleep and all subprograms are ready to process their next input)
            false
        } else {
            // If the inputs are all idle, then all the processes must be asleep and waiting for more input as well
            core.lock().unwrap()
                .processes.iter()
                .all(|process| if let SceneProcessState::Running(process) = process {
                    !process.is_awake && process.future.is_waiting()
                } else {
                    true
                })
        };

        // If all inputs are idle and the core is still in 'notification' mode, notify the waiting messages
        if all_inputs_idle && all_processes_idle {
            let mut locked_core = core.lock().unwrap();

            if locked_core.idle_count != idle_count {
                // Something else has notified or requested an idle notification, so this thread is no longer checking for a 'live' idle message
                //
                // Guards against a couple of races:
                //  * inputs might have become populated again since the check above as the core is not locked
                //  * the idle notification might have triggered a new notification requests before all the threads have finished checking for idleness so we get a 'stale' notification
                return false;
            }

            if !locked_core.notify_when_idle {
                // Some other thread has presumably notified
                return false;
            }

            // Claim the notification, and also note the idle count that we'll send to the listeners
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
                    notifier.try_send(idle_count)
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
            let mut to_discard  = vec![];

            for (idx, notifier) in notifiers.drain(..) {
                if notifier.is_none() {
                    // This notifier was disconnected (or is still disconnected)
                    to_discard.push(locked_core.when_idle[idx].take());
                }
            }

            // When we drop a sender, it might try to wake the future, which can try to re-lock the core, so we drop them all after releasing the core
            mem::drop(locked_core);
            mem::drop(to_discard);

            // Indicate to the caller that a notification occurred
            true
        } else {
            false
        }
    }

    ///
    /// Shuts down the input stream for a subprogram (which generally should cause the program itself to stop)
    ///
    pub (crate) fn close_subprogram(scene_core: &Arc<Mutex<SceneCore>>, program_id: SubProgramId) {
        let waker = {
            let program     = scene_core.lock().unwrap().get_sub_program(program_id);
            let input_core  = scene_core.lock().unwrap().get_input_stream_core(program_id);

            if let (Some(program), Some(input_core)) = (program, input_core) {
                let input_stream_id = program.lock().unwrap().input_stream_id();

                input_stream_id.close_input(&input_core)
            } else {
                Ok(None)
            }
        };

        if let Ok(Some(waker)) = waker {
            waker.wake()
        }
    }

    ///
    /// Attaches a child program to a parent program
    ///
    pub (crate) fn attach_child_program(scene_core: &Arc<Mutex<SceneCore>>, parent: SubProgramId, child: SubProgramId) {
        if parent == child {
            return;
        }

        // Fetch the parent and child subprogram cores from the scene core
        let (parent_core, child_core) = {
            let scene_core = scene_core.lock().unwrap();

            let parent_idx  = scene_core.program_indexes.get(&parent);
            let child_idx   = scene_core.program_indexes.get(&child);

            let parent_core = parent_idx.and_then(|idx| scene_core.sub_programs[*idx].clone());
            let child_core  = child_idx.and_then(|idx| scene_core.sub_programs[*idx].clone());

            (parent_core, child_core)
        };

        // Child subprogram is shut down if the parent program doesn't exist at this point
        let Some(parent_core)   = parent_core else { Self::close_subprogram(scene_core, child); return };
        let Some(child_core)    = child_core else { return };

        // Start by replacing the parent on the child core (can't really safely lock both)
        let old_parent = {
            let mut child_core = child_core.lock().unwrap();
            child_core.parent_program.take()
        };

        if let Some(old_parent) = old_parent {
            // Remove the child from the old parent
            let old_parent_core = {
                let scene_core = scene_core.lock().unwrap();

                let old_parent_idx = scene_core.program_indexes.get(&old_parent);
                old_parent_idx.and_then(|idx| scene_core.sub_programs[*idx].clone())
            };

            if let Some(old_parent_core) = old_parent_core {
                let mut old_parent_core = old_parent_core.lock().unwrap();
                old_parent_core.child_programs.remove(&child);
            }
        }

        // Add as child/parent relationship
        {
            let mut parent_core = parent_core.lock().unwrap();
            parent_core.child_programs.insert(child);
        }

        {
            let mut child_core = child_core.lock().unwrap();
            child_core.parent_program = Some(parent);
        }

        // TODO: potential race condition if the parent shuts down between fetching the core and setting the child subprogram
    }

    ///
    /// Removes the specified program from the list of child programs referenced by the parent program
    ///
    /// (This leaves the parent program value intact for the child program)
    ///
    pub (crate) fn detach_child_program(scene_core: &Arc<Mutex<SceneCore>>, parent_program: SubProgramId, child_program: SubProgramId) {
        // Fetch the core for the parent program from the scene
        let parent_core = {
            let scene_core  = scene_core.lock().unwrap();
            let parent_idx  = scene_core.program_indexes.get(&parent_program);
            let parent_core = parent_idx.and_then(|idx| scene_core.sub_programs[*idx].clone());

            parent_core
        };

        let Some(parent_core) = parent_core else { return; };

        // Remove the child program from the list of subprograms that are children of the parent program
        parent_core.lock().unwrap().child_programs.remove(&child_program);
    }

    ///
    /// Unceremoniously abort the specified subprogram (dropping its future without giving it
    /// a chance to continue, used in the event of an error or other critical failure)
    ///
    pub (crate) fn abort_subprogram(scene_core: &Arc<Mutex<SceneCore>>, program: &Arc<Mutex<SubProgramCore>>) {
        use std::mem;

        let mut core    = scene_core.lock().unwrap();
        let mut program = program.lock().unwrap();

        let program_id = *program.program_id();

        // Abort every process the program is running immediately
        let mut aborted_processes = vec![];
        for handle in program.process_ids.drain(..) {
            core.next_process        = core.next_process.min(handle.0);
            aborted_processes.push(mem::replace(&mut core.processes[handle.0], SceneProcessState::Aborted));
            core.awake_processes.retain(|old_handle| old_handle != &handle.0);
        }

        // Take the subprogram and input core out of the scene
        let subprogram_handle   = program.subprogram_handle;
        let old_sub_program     = core.sub_programs[subprogram_handle].take();
        let old_input_core      = core.sub_program_inputs[subprogram_handle].take();
        core.program_indexes.remove(&program_id);
        core.next_subprogram    = core.next_subprogram.min(subprogram_handle);

        // Drop in order: first release the core lock, then drop the subprograms and processes (which may re-take it)
        drop(program);
        drop(core);

        drop(aborted_processes);

        if let Some(old_sub_program) = &old_sub_program {
            old_sub_program.lock().unwrap().process_ids = vec![];
            SubProgramCore::closedown(old_sub_program, &scene_core);
        }

        drop(old_input_core);
        drop(old_sub_program);

        // Notify that the program has stopped
        SceneCore::send_scene_updates(&scene_core, vec![SceneUpdate::Stopped(program_id)]);

        // Core might be idle now the program has finished
        SceneCore::check_if_idle(&scene_core);
    }

    ///
    /// Performs the specified action and catches any panics that might result
    ///
    /// Returns Some(result) if the action was successful.
    ///
    /// If the action panics, the program will be marked as aborted (if this happens while
    /// polling the program, returning Pending will not result in another poll)
    ///
    #[inline]
    pub (crate) fn catch_panics<TResult>(scene_core: &Arc<Mutex<SceneCore>>, program_core: &Arc<Mutex<SubProgramCore>>, action: impl UnwindSafe + FnOnce() -> TResult) -> Option<TResult> {
        match catch_unwind(move || action()) {
            Ok(result) => Some(result),
            Err(panic) => {
                // Program is immediately aborted if the action panics
                Self::abort_subprogram(scene_core, program_core);

                // Mark the core as panicked
                Self::panic_scene(scene_core, program_core, panic);

                None
            },
        }
    }

    ///
    /// Panics the current scene
    ///
    pub (crate) fn panic_scene(scene_core: &Arc<Mutex<SceneCore>>, program_core: &Arc<Mutex<SubProgramCore>>, panic: Box<dyn 'static + Any + Send>) {
        // Try soft-panicking the core (which just sends the 'panic' message and lets the error program do what it will with the information)
        if let Err(still_panicking) = Self::soft_panic_scene(scene_core, program_core, panic) {
            // Hard panic the core instead
            Self::hard_panic_scene(scene_core, program_core, still_panicking);
        }
    }

    ///
    /// 'Soft' panics a scene by sending a message and allowing the scene to tidy itself up
    ///
    /// Returns false if the scene cannot be soft-panicked (eg, because there's no error program
    /// running, the error program itself is panicked, the message can't be queued, etc)
    ///
    pub (crate) fn soft_panic_scene(scene_core: &Arc<Mutex<SceneCore>>, program_core: &Arc<Mutex<SubProgramCore>>, panic: Box<dyn 'static + Any + Send>) -> Result<(), Box<dyn 'static + Any + Send>>  {
        // Send the panic from the program that generated it
        let program_id = {
            let program_core = program_core.lock().unwrap();
            *program_core.program_id()
        };

        // If the core is already panicked, then hard panic
        if !scene_core.lock().unwrap().active_panics.is_empty() {
            return Err(panic);
        }

        // Fetch the input sink for the program
        let Ok((error_target, waker)) = scene_core.lock().unwrap().sink_for_target::<Error>(&program_id, StreamTarget::Any) else { return Err(panic); };
        if let Some(waker) = waker { waker.wake(); }

        match &error_target {
            OutputSinkTarget::Disconnected  |
            OutputSinkTarget::Discard       => { 
                // Disconnected error stream: switch to hard panicking
                Err(panic)
            }

            OutputSinkTarget::Input(input)              |
            OutputSinkTarget::CloseWhenDropped(input)   |
            OutputSinkTarget::FixedInput(input)         => {
                // Try to queue up the message urgently
                if let Some(input) = input.upgrade() {
                    // Lock the stream (deal with the case where it's the stream itself that's panicked by hard panicking)
                    let Ok(mut input_lock) = input.lock() else { return Err(panic) };

                    // Send the error with overfill (we're not blocking here)
                    let panic_message = if let Some(panic_message) = panic.downcast_ref::<&'static str>() {
                        panic_message.to_string()
                    } else if let Some(panic_message) = panic.downcast_ref::<String>() {
                        panic_message.clone()
                    } else {
                        "panicked".to_string()
                    };

                    let message = Error::Panic { source: program_id, message: panic_message.into() };
                    let waker   = input_lock.send_with_overfill(program_id, message);
                    drop(input_lock);

                    // Trigger the wake action to ensure the message is delivered
                    match waker {
                        Ok(Some(waker)) => { waker.wake(); },
                        Ok(None)        => { }
                        Err(_)          => { return Err(panic); }
                    }

                    // Store as the active panic in the core
                    {
                        let mut scene_core = scene_core.lock().unwrap();

                        if scene_core.active_panics.is_empty() {
                            scene_core.active_panics.push(panic);
                        } else {
                            // Late hard-panic (we've sent the message but another panic has won the race to be first)
                            return Err(panic);
                        }
                    }

                    Ok(())
                } else {
                    // Input stream exists but is dead
                    Err(panic)
                }
            }
        }
    }

    ///
    /// Hard-panics the scene because the error handler isn't present
    ///
    pub (crate) fn hard_panic_scene(scene_core: &Arc<Mutex<SceneCore>>, _program_core: &Arc<Mutex<SubProgramCore>>, panic: Box<dyn 'static + Any + Send>) {
        let mut scene_core = scene_core.lock().unwrap();

        // Add to the active panics
        scene_core.active_panics.push(panic);

        // Tell the core to stop immediately
        let wakers = scene_core.stop();

        // Wake up anything that needs to be awoken to stop the core
        wakers.into_iter().for_each(|waker| waker.wake());
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

            if let Some(SceneProcessState::Running(process)) = process {
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

impl SceneProcessState {
    #[inline]
    fn is_free(&self) -> bool {
        match self {
            SceneProcessState::Free         => true,
            SceneProcessState::Running(_)   => false,
            SceneProcessState::Aborted      => false,
        }
    }

    #[inline]
    fn as_mut(&mut self) -> Option<&mut SceneProcess> {
        match self {
            SceneProcessState::Free                     => None,
            SceneProcessState::Running(scene_process)   => Some(scene_process),
            SceneProcessState::Aborted                  => None,
        }
    }
}

///
/// Runs the programs attached to a scene core
///
pub (crate) fn run_core(core: &Arc<Mutex<SceneCore>>) -> impl Future<Output=()> + use<> {
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

                if let Some(SceneProcessState::Running(next_process)) = core.processes.get_mut(next_process_idx) {
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
                    let process_data    = &mut core.processes[next_process_idx];

                    if let SceneProcessState::Running(process_data) = process_data {
                        process_data.future = SceneProcessFuture::Waiting(next_process);
                        process_data.unpark_when_waiting.drain(..).for_each(|thread| thread.unpark());

                        if process_data.is_awake {
                            // Possible re-awoken while polling, so make sure the process is still in the pending list so it gets polled again
                            if !core.awake_processes.contains(&next_process_idx) {
                                core.awake_processes.push_back(next_process_idx);
                            }
                        }
                    } else {
                        // This process has terminated abnormally
                        *process_data       = SceneProcessState::Free;
                        core.next_process   = core.next_process.min(next_process_idx);

                        let on_stop = core.on_stop.drain(..).collect::<Vec<_>>();
                        drop(core);

                        // Wake up anything that wants to be notified when a process stops
                        for stop_notification in on_stop {
                            stop_notification.wake();
                        }
                    }
                } else {
                    // This process has been terminated: remove it from the list
                    let on_stop = {
                        let mut core = unlocked_core.lock().unwrap();

                        core.processes[next_process_idx] = SceneProcessState::Free;
                        core.next_process = core.next_process.min(next_process_idx);

                        core.on_stop.drain(..).collect::<Vec<_>>()
                    };

                    // Wake up anything that wants to be notified when a process stops
                    for stop_notification in on_stop {
                        stop_notification.wake();
                    }
                }
            }
        }
    })
}
