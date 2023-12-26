use crate::{SubProgramId};

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