use crate::OutputSinkTarget;
use crate::input_stream::*;
use crate::stream_id::*;
use crate::subprogram_id::*;

use futures::prelude::*;
use futures::future::{BoxFuture, poll_fn};
use futures::task::{Poll, Waker, Context, waker, ArcWake};

use std::any::*;
use std::collections::*;
use std::sync::*;

///
/// Data that's stored for an individual program
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

    /// The input streams for each sub-program
    sub_program_inputs: Vec<Option<Arc<Mutex<dyn Send + Sync + Any>>>>,

    /// The next free sub-program
    next_subprogram: usize,

    /// Maps subprogram IDs to indexes in the subprogram list
    program_indexes: HashMap<SubProgramId, usize>,

    /// The programs that have been woken up since the core was last polled
    awake_programs: VecDeque<usize>,

    /// Wakers for the futures that are being used to run the scene (can be multiple if the scene is scheduled across a thread pool)
    thread_wakers: Vec<Option<Waker>>,
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
}
