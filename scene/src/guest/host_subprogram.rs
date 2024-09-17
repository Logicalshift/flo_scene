use super::poll_action::*;
use super::poll_result::*;
use super::guest_message::*;
use crate::host::*;

use futures::prelude::*;
use futures::channel::mpsc;

///
/// Runs a guest subprogram as a subprogram in a scene
///
/// The result stream here should supply messages only for the subprogram that should be run here.
///
/// The guest program should generate the supplied message type, it's an error if it does not.
///
pub async fn run_host_subprogram<TMessageType>(input_stream: InputStream<TMessageType>, context: SceneContext, encoder: impl 'static + Send + GuestMessageEncoder, actions: mpsc::Sender<GuestAction>, results: impl 'static + Send + Unpin + Stream<Item=GuestResult>) 
where
    TMessageType: 'static + GuestSceneMessage + SceneMessage
{
    let mut actions = actions;
    let mut results = results;

    let guest_program_handle;
    let guest_stream_id;

    // Setup phase: we get the program handle and the input stream handle for the guest program
    loop {
        if let Some(msg) = results.next().await {
            match msg {
                GuestResult::CreateSubprogram(program_id, program_handle, stream_id) => {
                    // TODO: program_id does not need to match here but maybe we should check/warn if it does not
                    if Some(program_id) != context.current_program_id() {
                        // Program IDs do not match: log warning (consider error)
                    }

                    guest_program_handle    = program_handle;
                    guest_stream_id         = stream_id;
                    break;
                }

                unexpected => {
                    // Unexpected message
                    // TODO: log/soft error instead of panicking
                    panic!("Unexpected guest message: {:?}", unexpected);
                }
            }
        } else {
            // Guest program failed to start
            // TODO: log/soft error instead of panicking
            panic!("Guest program failed to start");
        }
    }

    // Guest program has started: perform 'pre-flight' checks
    if guest_stream_id != TMessageType::stream_id() {
        // The guest program must generate the same stream ID as the host
        // TODO: log/soft error instead of panicking
        panic!("Was expecting a guest program generating message type {:?}, but got {:?}", TMessageType::stream_id(), guest_stream_id);
    }

    // Main loop: relay messages and connect to sinks
    future::select(
        Box::pin(async move {
            use GuestResult::*;

            // Loop 1: handle the results from the guest program
            while let Some(result) = results.next().await {
                match result {
                    Stopped => { 
                        // Guest has entirely stopped
                        break;
                    }

                    CreateSubprogram(_id, _handle, _stream_id) => {

                    }

                    EndedSubprogram(program_handle) => {
                        // Program that we're running has entirely stopped 
                        if program_handle == guest_program_handle {
                            break;
                        }
                    }

                    Ready(handle) => {
                        // Indicate we're ready to receive mroe input
                    }

                    Connect(sink_handle, stream_target) => { 
                        // TODO: connect to a sink on the source side
                    }

                    Send(sink_handle, encoded_bytes) => {
                        // TODO: send to an existing connected sink handle
                    }

                    Disconnect(sink_handle) => {
                        // TODO: remove a sink handle
                    }

                    ContinuePolling => { 
                        // Nothing to do, should be handled by the stream
                    }
                }
            } 
        }),

        Box::pin(async move {
            // Loop 2: read from the input stream
            let mut input_stream = input_stream;
            while let Some(input) = input_stream.next().await {
                // TODO: send as an action if the input stream is ready
            }
        })
    ).await;
}
