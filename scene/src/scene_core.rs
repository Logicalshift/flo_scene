use crate::output_sink::*;
use crate::input_stream::*;
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
/// Data that's stored for an individual program.
///
/// Note that the scene core must be locked before the subprogram core, if the scene core needs to be locked.
///
pub (crate) struct SubProgramCore {
    /// The handle of this program within the core (index into the sub_programs list)
    handle: usize,

    /// The ID of this program
    id: SubProgramId,

    /// The pending future that represents this running subprogram
    run: BoxFuture<'static, ()>,

    /// The output sink targets for this sub-program
    outputs: HashMap<StreamId, Arc<dyn Send + Sync + Any>>,

    /// Set to true if this subprogram has been awakened since it was last polled
    awake: bool,
}

///
/// Used to wake up anything polling a scene core when a subprogram is ready
///
pub (crate) struct SceneCoreWaker {
    /// The core that should be woken when this subprogram is ready to run
    core: Weak<Mutex<SceneCore>>,

    /// The subprogram that is woken by this waker
    subprogram_handle: usize,
}

///
/// The scene core is used to store the shared state for all scenes
///
pub (crate) struct SceneCore {
    /// The sub-programs that are active in this scene
    sub_programs: Vec<Option<Arc<Mutex<SubProgramCore>>>>,

    /// The input stream cores for each sub-program
    sub_program_inputs: Vec<Option<Arc<dyn Send + Sync + Any>>>,

    /// The next free sub-program
    next_subprogram: usize,

    /// Maps subprogram IDs to indexes in the subprogram list
    program_indexes: HashMap<SubProgramId, usize>,

    /// The programs that have been woken up since the core was last polled
    awake_programs: VecDeque<usize>,

    /// Wakers for the futures that are being used to run the scene (can be multiple if the scene is scheduled across a thread pool)
    thread_wakers: Vec<Option<Waker>>,

    /// The connections to assign between programs. More specific sources override less specific sources.
    connections: HashMap<(StreamSource, StreamId), StreamTarget>,
}

impl SceneCore {
    ///
    /// Creates an empty scene core
    ///
    pub fn new() -> SceneCore {
        SceneCore {
            sub_programs:       vec![],
            sub_program_inputs: vec![],
            next_subprogram:    0,
            program_indexes:    HashMap::new(),
            awake_programs:     VecDeque::new(),
            connections:        HashMap::new(),
            thread_wakers:      vec![],
        }
    }

    ///
    /// Adds a program to the list being run by this scene
    ///
    pub fn start_subprogram<TMessage>(&mut self, program_id: SubProgramId, program: impl 'static + Send + Sync + Future<Output=()>, input_core: Arc<Mutex<InputStreamCore<TMessage>>>) -> (Arc<Mutex<SubProgramCore>>, Option<Waker>)
    where
        TMessage: 'static + Unpin + Send + Sync,
    {
        // next_subprogram should always indicate the handle we'll use for the new program (it should be either a None entry in the list or sub_programs.len())
        let handle      = self.next_subprogram;

        // Create the sub-program
        let subprogram  = SubProgramCore {
            handle:     handle,
            id:         program_id.clone(),
            run:        program.boxed(),
            outputs:    HashMap::new(),
            awake:      true,
        };

        // Allocate space for the program
        while self.sub_programs.len() <= handle {
            self.sub_programs.push(None);
            self.sub_program_inputs.push(None);
        }
        debug_assert!(self.sub_programs[handle].is_none());

        // Store the program details
        let subprogram                  = Arc::new(Mutex::new(subprogram));
        self.sub_programs[handle]       = Some(Arc::clone(&subprogram));
        self.sub_program_inputs[handle] = Some(input_core);
        self.program_indexes.insert(program_id.clone(), handle);

        self.awake_programs.push_back(handle);

        // Update the 'next_subprogram' value to an empty slot
        while self.next_subprogram < self.sub_programs.len() && self.sub_programs[self.next_subprogram].is_some() {
            self.next_subprogram += 1;
        }

        // Return a waker if one is available (we want to wake it with the core unlocked, so this is just returned)
        let mut waker   = None;
        for maybe_waker in self.thread_wakers.iter_mut() {
            waker = maybe_waker.take();
            if waker.is_some() {
                break;
            }
        }

        (subprogram, waker)
    }

    ///
    /// Retrieves the input stream for a particular stream target (an error if the target either doesn't exist or does not accept this input stream type)
    ///
    pub (crate) fn get_target_input(&mut self, target: &StreamTarget, expected_message_type: TypeId) -> Result<Arc<dyn Send + Sync + Any>, ()> {
        match target {
            StreamTarget::None  => todo!(), // Create a discard stream of the specified type
            StreamTarget::Any   => todo!(), // Create a disconnected stream of the specified type

            StreamTarget::Program(sub_program_id) => {
                // Fetch the sub-program handle (or return an error if it doesn't exist)
                let handle = *self.program_indexes.get(sub_program_id).ok_or(())?;

                // The message type must match the expected type
                let target_input = self.sub_program_inputs[handle].as_ref().ok_or(())?;

                if target_input.type_id() != expected_message_type {
                    Err(())
                } else {
                    Ok(Arc::clone(target_input))
                }
            }
        }
    }

    ///
    /// Adds or updates a program connection in this core
    ///
    pub (crate) fn connect_programs(&mut self, source: StreamSource, target: StreamTarget, stream_id: StreamId) -> Result<(), ()> {
        // Fetch the target stream (returning an error if it can't be found)
        let target_input = self.get_target_input(&target, stream_id.message_type())?;

        // TODO: pause the inputs of all the sub-programs matching the source, so the update is atomic?

        // Store the connection
        self.connections.insert((source.clone(), stream_id.clone()), target.clone());

        // Update the existing connections
        for maybe_sub_program in self.sub_programs.iter() {
            if let Some(sub_program) = maybe_sub_program {
                // Update the streams of the subprogram
                let mut sub_program = sub_program.lock().unwrap();

                if source.matches_subprogram(&sub_program.id) {
                    sub_program.reconnect_output_sinks(&target_input, &stream_id);
                }
            }
        }

        Ok(())
    }

    ///
    /// Returns the output sink target configured for a particular stream
    ///
    pub (crate) fn sink_for_target<TMessageType>(&mut self, source: &SubProgramId, target: StreamTarget) -> Option<Arc<Mutex<OutputSinkTarget<TMessageType>>>>
    where
        TMessageType: 'static + Send + Sync,
    {
        match target {
            StreamTarget::None  |
            StreamTarget::Any   => {
                // Return the general stream for the message type, if there is one
                let maybe_connection = self.connections
                    .get(&(source.into(), StreamId::with_message_type::<TMessageType>()))
                    .or_else(|| self.connections.get(&(StreamSource::All, StreamId::with_message_type::<TMessageType>())));

                if let Some(connection) = maybe_connection {
                    // Stream is connected to a specific program (or is configured to discard its input)
                    match connection {
                        StreamTarget::None                          => Some(Arc::new(Mutex::new(OutputSinkTarget::Discard))),
                        StreamTarget::Any                           => Some(Arc::new(Mutex::new(OutputSinkTarget::Disconnected))),
                        StreamTarget::Program(target_program_id)    => {
                            // Connect the stream to the input of a specific program
                            let target_program_handle   = self.program_indexes.get(&target_program_id)?;
                            let target_program_input    = self.sub_program_inputs.get(*target_program_handle)?.clone()?;
                            let target_program_input    = target_program_input.downcast::<Mutex<InputStreamCore<TMessageType>>>().ok()?;

                            Some(Arc::new(Mutex::new(OutputSinkTarget::Input(Arc::downgrade(&target_program_input)))))
                        }
                    }
                } else {
                    // Stream is not connected, either use a discard or a disconnected stream
                    match target {
                        StreamTarget::None  => Some(Arc::new(Mutex::new(OutputSinkTarget::Discard))),
                        StreamTarget::Any   => Some(Arc::new(Mutex::new(OutputSinkTarget::Disconnected))),
                        _                   => None
                    }
                }
            }

            StreamTarget::Program(target_program_id) => {
                // The connections can define a redirect stream by using a StreamId target
                let target_program_id = self.connections.get(&(source.into(), StreamId::for_target::<TMessageType>(&target_program_id)))
                    .or_else(|| self.connections.get(&(StreamSource::All, StreamId::for_target::<TMessageType>(&target_program_id))))
                    .and_then(|target| {
                        match target {
                            StreamTarget::Program(program_id)   => Some(program_id.clone()),
                            _                                   => None,
                        }
                    })
                    .unwrap_or(target_program_id);

                // Attempt to find the target stream for this specific program
                // TODO: if the program hasn't started yet, we should create a disconnected stream and connect it later on
                let target_program_handle   = self.program_indexes.get(&target_program_id)?;
                let target_program_input    = self.sub_program_inputs.get(*target_program_handle)?.clone()?;
                let target_program_input    = target_program_input.downcast::<Mutex<InputStreamCore<TMessageType>>>().ok()?;

                Some(Arc::new(Mutex::new(OutputSinkTarget::Input(Arc::downgrade(&target_program_input)))))
            }
        }
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
    /// Returns the existing output target for a stream ID, if it exists in this subprogram
    ///
    pub (crate) fn output_target<TMessageType>(&self, id: &StreamId) -> Option<Arc<Mutex<OutputSinkTarget<TMessageType>>>> 
    where
        TMessageType: 'static + Send + Sync,
    {
        // Fetch the existing target and clone it
        let existing_target = self.outputs.get(id)?;
        let existing_target = Arc::clone(existing_target);

        // Convert to the appropriate output type
        existing_target.downcast::<Mutex<OutputSinkTarget<TMessageType>>>().ok()
    }

    ///
    /// Tries to set the output target for a stream ID. Returns Ok() if the new output target was defined or Err() if there's already a valid output for this stream
    ///
    /// Panics if the stream ID doesn't match the message type and the stream already exists.
    ///
    pub (crate) fn try_create_output_target<TMessageType>(&mut self, id: &StreamId, new_output_target: Arc<Mutex<OutputSinkTarget<TMessageType>>>) -> Result<Arc<Mutex<OutputSinkTarget<TMessageType>>>, Arc<Mutex<OutputSinkTarget<TMessageType>>>>
    where
        TMessageType: 'static + Send + Sync,
    {
        let existing_target = self.outputs.get(id);
        if let Some(existing_target) = existing_target {
            // Return the already existing target
            let existing_target = Arc::clone(existing_target);
            let existing_target = existing_target.downcast::<Mutex<OutputSinkTarget<TMessageType>>>().unwrap();

            Err(existing_target)
        } else {
            // Store a new target in the outputs
            let cloned_target = Arc::clone(&new_output_target);
            self.outputs.insert(id.clone(), cloned_target);

            // Use the new target for the output stream
            Ok(new_output_target)
        }
    }

    ///
    /// Connects all of the streams that matches a particular stream ID to a new target
    ///
    pub (crate) fn reconnect_output_sinks(&mut self, target_input: &Arc<dyn Send + Sync + Any>, stream_id: &StreamId) {
        if let Some(output_sink) = self.outputs.get_mut(stream_id) {
            // This stream has an output matching the input (the stream types should always match)
            stream_id.connect_input_to_output(target_input, output_sink).expect("Input and output types do not match");
        }
    }
}

impl SceneCoreWaker {
    ///
    /// Creates a waker for a scene core
    ///
    pub fn with_core(core: Arc<Mutex<SceneCore>>, subprogram_handle: usize) -> Self {
        Self {
            core:               Arc::downgrade(&core),
            subprogram_handle:  subprogram_handle,
        }
    }
}

impl ArcWake for SceneCoreWaker {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        // If the scene is still running, fetch the core
        let subprogram_handle   = arc_self.subprogram_handle;
        let core                = if let Some(core) = arc_self.core.upgrade() { core } else { return; };

        // Fetch a waker from the core to wake up a thread
        let (waker, program) = {
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
            let program = core.sub_programs.get(subprogram_handle).cloned().unwrap_or_default();

            if program.is_some() {
                core.awake_programs.push_back(subprogram_handle);
            }

            (waker, program)
        };

        // Mark the program as awake (so it will be polled)
        if let Some(program) = program {
            let mut program = program.lock().unwrap();
            program.awake = true;
        }

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
        use std::mem;

        loop {
            // Fetch a program to poll from the core: if all the programs are complete, then stop
            let next_program = {
                // Acquire the core
                let mut core = core.lock().unwrap();

                if core.next_subprogram == 0 && core.sub_programs.iter().all(|program| program.is_none()) {
                    // The scene is finished when there are no running programs left in it
                    return Poll::Ready(());
                }

                // Read the index of an awake program to poll (or return pending if there are no pending programs)
                let next_program_idx = core.awake_programs.pop_front();
                let next_program_idx = if let Some(next_program_idx) = next_program_idx { 
                    next_program_idx 
                } else {
                    // Store a waker for this thread
                    let waker = ctxt.waker().clone();
                    core.thread_wakers[waker_idx] = Some(waker);

                    // Wait for a subprogram to wake us
                    return Poll::Pending;
                };

                core.sub_programs.get(next_program_idx).cloned()
            };

            if let Some(Some(next_program)) = next_program {
                let mut next_program = next_program.lock().unwrap();

                // Only poll the program if it's still awake
                if next_program.awake {
                    // Mark as asleep
                    next_program.awake = false;

                    // Poll the program in our own context (will wake anything that's running this core)
                    let program_waker       = waker(Arc::new(SceneCoreWaker::with_core(Arc::clone(&core), next_program.handle)));
                    let mut program_context = Context::from_waker(&program_waker);

                    // Poll the program
                    let poll_result         = next_program.run.poll_unpin(&mut program_context);

                    // Release the lock (so that it's safe to acquire the core lock if the program has finished)
                    let program_handle  = next_program.handle;
                    let program_id      = next_program.id.clone();
                    mem::drop(next_program);

                    if let Poll::Ready(_) = poll_result {
                        // Remove the program from the core when it's finished
                        let mut core = core.lock().unwrap();

                        core.sub_programs[program_handle]       = None;
                        core.sub_program_inputs[program_handle] = None;
                        core.program_indexes.remove(&program_id);

                        // Re-use this handle if a new program is started
                        core.next_subprogram = core.next_subprogram.min(program_handle);
                    }
                }
            }
        }
    })
}
