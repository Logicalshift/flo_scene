use crate::module::*;

use flo_scene::guest::*;

use futures::prelude::*;
use futures::channel::mpsc;

use std::sync::*;

///
/// Creates the streams for sending actions to a WASM module and receiving the results
///
pub fn create_module_streams(module: Arc<Mutex<WasmModule>>, runtime_id: GuestRuntimeHandle) -> (mpsc::Sender<GuestAction>, impl 'static + Send + Unpin + Stream<Item=GuestResult>) {
    // Create the sender/receiver
    let (action_sender, action_receiver) = mpsc::channel(32);

    // We gather the receiver values into chunks to process as many as possible at once
    let action_receiver = action_receiver.ready_chunks(64);

    // Poll the runtime to make sure that it's in an idle condition
    let initial_results     = module.lock().unwrap().poll_awake(runtime_id);
    let stopped             = false;
    let poll_immediately    = false;

    // Create the result stream; the runtime is run by awaiting on this
    let result_stream = stream::unfold((module, runtime_id, action_receiver, stopped, poll_immediately), |(module, runtime_id, action_receiver, stopped, poll_immediately)| async move {
        let mut action_receiver = action_receiver;

        if stopped {
            // Most recent poll result indicated we have run out of actions (we have to wait to stop the stream as we want the results to be processed)
            return None;
        }

        let maybe_actions = if poll_immediately {
            // The guest indicated it wanted an immediate callback without waiting (so we do so once all of the results have been processed)
            Some(vec![])
        } else {
            // The guest is idle, so we wait until some external action wakes it up
            action_receiver.next().await
        };

        // Process the actions in the guest
        if let Some(actions) = maybe_actions {
            // Process the actions into the runtime
            actions.into_iter().for_each(|action| module.lock().unwrap().process(runtime_id, action));

            // Poll for the next set of results
            let next_actions = module.lock().unwrap().poll_awake(runtime_id);

            // Check if the runtime has stopped or if we need to poll immediately the next time through
            let mut stopped             = stopped;
            let mut poll_immediately    = false;

            for action in next_actions.iter() {
                match action {
                    GuestResult::Stopped            => { stopped = true;}
                    GuestResult::ContinuePolling    => { poll_immediately = true; }
                    _                               => { }
                }
            }

            // Convert to a stream
            let next_actions = stream::iter(next_actions);
            Some((next_actions, (module, runtime_id, action_receiver, stopped, poll_immediately)))
        } else {
            // The actions have finished
            None
        }
    }).flatten();

    // Chain the initial results with the extra result stream
    let result_stream = stream::iter(initial_results).chain(result_stream);

    // Result is the stream we just built
    (action_sender, Box::pin(result_stream))
}
