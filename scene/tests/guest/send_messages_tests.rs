use flo_scene::*;
use flo_scene::guest::*;

use futures::prelude::*;

use serde::*;
use serde_json;

use std::sync::*;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SimpleTestMessage {
    value: String,
}

impl SceneMessage for SimpleTestMessage { }
impl GuestSceneMessage for SimpleTestMessage { }

#[test]
pub fn send_json_message_to_runtime() {
    // The results from the guest (we're not doing any isolation stuff so we can share variables this way)
    let received = Arc::new(Mutex::new(vec![]));

    // Create a runtime that receives messages using the JSON encoder
    let encoder         = GuestJsonEncoder;
    let messages        = Arc::clone(&received);
    let guest_runtime   = GuestRuntime::with_default_subprogram(encoder, move |input_stream: GuestInputStream<SimpleTestMessage>, _context| async move {
        let mut input_stream = input_stream;
        while let Some(msg) = input_stream.next().await {
            messages.lock().unwrap().push(msg);
        }
    });

    // Enqueue a message for the runtime (the default subprogram always has the same handle)
    let data = SimpleTestMessage { value: "Test".into() }.serialize(serde_json::value::Serializer).unwrap();
    let data = data.to_string().into_bytes();
    guest_runtime.send_message(GuestSubProgramHandle::default(), data);

    // Polling the runtime once should clear the pending message
    let _result = guest_runtime.poll_awake();

    // Message should have been received
    let received = received.lock().unwrap();
    assert!(received.len() == 1, "{:?}", received);
    assert!(received[0] == SimpleTestMessage { value: "Test".into() }, "{:?}", received);
}
