use flo_scene::*;
use flo_scene::guest::*;

use futures::prelude::*;
use futures::executor;

use serde::*;
use serde_json;

use std::sync::*;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SimpleTestMessage {
    value: String,
}

impl SceneMessage for SimpleTestMessage { }
impl GuestSceneMessage for SimpleTestMessage {
    fn stream_id() -> HostStreamId {
        HostStreamId::with_name("flo_scene_tests::send_message_tests::SimpleTestMessage")
    }
}

#[test]
pub fn send_json_message_to_runtime_using_stream() {
    // The results from the guest (we're not doing any isolation stuff so we can share variables this way)
    let received = Arc::new(Mutex::new(vec![]));
    let woken    = Arc::new(Mutex::new(false));

    // Create a runtime that receives messages using the JSON encoder
    let encoder         = GuestJsonEncoder;
    let messages        = Arc::clone(&received);
    let awake           = Arc::clone(&woken);
    let guest_runtime   = GuestRuntime::with_default_subprogram(SubProgramId::new(), encoder, move |input_stream: GuestInputStream<SimpleTestMessage>, _context| async move {
        (*awake.lock().unwrap()) = true;

        let mut input_stream = input_stream;
        if let Some(msg) = input_stream.next().await {
            println!("Received message");
            messages.lock().unwrap().push(msg);
        }
    });

    // Initially shouldn't be woken up
    assert!(*woken.lock().unwrap() == false);

    // Run as a stream, which should end once the main program finishes
    let (actions, output) = guest_runtime.as_streams();

    let mut output  = output;
    let mut actions = actions;
    executor::block_on(async {
        // Enqueue a message for the runtime (the default subprogram always has the same handle)
        let data = SimpleTestMessage { value: "Test".into() }.serialize(serde_json::value::Serializer).unwrap();
        let data = data.to_string().into_bytes();

        println!("Send action");
        actions.send(GuestAction::SendMessage(GuestSubProgramHandle::default(), data)).await.unwrap();
        println!("Sent");

        // Poll until the program finishes
        while let Some(_) = output.next().await {
        }
    });

    // Message should have been received and properly decoded
    let received = received.lock().unwrap();
    assert!(received.len() == 1, "{:?}", received);
    assert!(received[0] == SimpleTestMessage { value: "Test".into() }, "{:?}", received);
}
