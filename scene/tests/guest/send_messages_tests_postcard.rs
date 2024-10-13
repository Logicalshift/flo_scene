use flo_scene::*;
use flo_scene::guest::*;

use futures::prelude::*;

use serde::*;
use postcard;

use std::sync::*;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SimpleTestMessage {
    value: String,
}

impl SceneMessage for SimpleTestMessage {
    fn message_type_name() -> String {
        "flo_scene_tests::send_message_tests::SimpleTestMessage".into()
    }
}

#[test]
pub fn send_postcard_message_to_runtime() {
    // The results from the guest (we're not doing any isolation stuff so we can share variables this way)
    let received = Arc::new(Mutex::new(vec![]));
    let woken    = Arc::new(Mutex::new(false));

    // Create a runtime that receives messages using the postcard encoder
    let encoder         = GuestPostcardEncoder;
    let messages        = Arc::clone(&received);
    let awake           = Arc::clone(&woken);
    let guest_runtime   = GuestRuntime::with_default_subprogram(SubProgramId::new(), encoder, move |input_stream: GuestInputStream<SimpleTestMessage>, _context| async move {
        (*awake.lock().unwrap()) = true;

        let mut input_stream = input_stream;
        while let Some(msg) = input_stream.next().await {
            messages.lock().unwrap().push(msg);
        }
    });

    // Initially shouldn't be woken up
    assert!(*woken.lock().unwrap() == false);

    // Poll once to make the loop start waiting (we can send messages before this point: want to test that we'll wake the thread up again)
    let result = guest_runtime.poll_awake();
    assert!(*woken.lock().unwrap() == true);
    assert!(result.contains(&GuestResult::Ready(GuestSubProgramHandle::default())));

    // Enqueue a message for the runtime (the default subprogram always has the same handle)
    let data = postcard::to_stdvec(&SimpleTestMessage { value: "Test".into() }).unwrap();
    guest_runtime.send_message(GuestSubProgramHandle::default(), data);

    // Polling the runtime once should clear the pending message
    let result = guest_runtime.poll_awake();

    // Message should have been received and properly decoded
    let received = received.lock().unwrap();
    assert!(received.len() == 1, "{:?}", received);
    assert!(received[0] == SimpleTestMessage { value: "Test".into() }, "{:?}", received);
    assert!(result.contains(&GuestResult::Ready(GuestSubProgramHandle::default())));

    // Program isn't doing anything so it doesn't get more ready
    let result = guest_runtime.poll_awake();
    assert!(!result.contains(&GuestResult::Ready(GuestSubProgramHandle::default())));
}

#[test]
pub fn receive_message_from_runtime() {
    // Create a runtime that sends a message to the host
    let encoder         = GuestPostcardEncoder;
    let guest_runtime   = GuestRuntime::with_default_subprogram(SubProgramId::new(), encoder, move |_input_stream: GuestInputStream<SimpleTestMessage>, context| async move {
        // Send the message to the default target
        let mut message_sink = context.send::<SimpleTestMessage>(()).unwrap();
        message_sink.send(SimpleTestMessage { value: "From remote".into() }).await.unwrap();
    });

    // We now need to send the expected responses back for the sink that was just opened up

    // The runtime will request a connection
    let connect_result      = guest_runtime.poll_awake();
    let mut connect_request = connect_result.into_iter().filter(|msg| matches!(msg, GuestResult::Connect(_, _))).collect::<Vec<_>>();

    assert!(connect_request.len() == 1);

    let (sink_handle, stream_target) = if let GuestResult::Connect(handle, stream_target) = connect_request.pop().unwrap() { 
        (handle, stream_target)
    } else {
        unreachable!()
    };

    assert!(stream_target == HostStreamTarget::Any(HostStreamId::for_message::<SimpleTestMessage>()));

    // We need to send a connection back
    guest_runtime.sink_ready(sink_handle);

    // Guest should now send us the first message
    let send_result = guest_runtime.poll_awake();
    let mut send_result = send_result.into_iter().filter(|msg| matches!(msg, GuestResult::Send(_, _))).collect::<Vec<_>>();

    assert!(send_result.len() != 2, "{:?}", send_result);
    assert!(send_result.len() == 1, "{:?}", send_result);

    let (send_sink_handle, data) = if let GuestResult::Send(handle, data) = send_result.pop().unwrap() { 
        (handle, data)
    } else {
        unreachable!()
    };

    assert!(send_sink_handle == sink_handle);
    let decoded = GuestPostcardEncoder.decode::<SimpleTestMessage>(data);
    assert!(decoded.value == "From remote");
}

#[test]
pub fn receive_several_messages_from_runtime() {
    // Create a runtime that sends a message to the host
    let encoder         = GuestPostcardEncoder;
    let guest_runtime   = GuestRuntime::with_default_subprogram(SubProgramId::new(), encoder, move |_input_stream: GuestInputStream<SimpleTestMessage>, context| async move {
        // Send the message to the default target
        let mut message_sink = context.send::<SimpleTestMessage>(()).unwrap();
        message_sink.send(SimpleTestMessage { value: "From remote".into() }).await.unwrap();
        message_sink.send(SimpleTestMessage { value: "Another message".into() }).await.unwrap();
    });

    // We now need to send the expected responses back for the sink that was just opened up

    // The runtime will request a connection
    let connect_result      = guest_runtime.poll_awake();
    let mut connect_request = connect_result.into_iter().filter(|msg| matches!(msg, GuestResult::Connect(_, _))).collect::<Vec<_>>();

    assert!(connect_request.len() == 1);

    let (sink_handle, stream_target) = if let GuestResult::Connect(handle, stream_target) = connect_request.pop().unwrap() { 
        (handle, stream_target)
    } else {
        unreachable!()
    };

    assert!(stream_target == HostStreamTarget::Any(HostStreamId::for_message::<SimpleTestMessage>()));

    // We need to send a connection back
    guest_runtime.sink_ready(sink_handle);

    // Guest should now send us the first message (second won't arrive until we re-signal that we're ready to receive it)
    let send_result = guest_runtime.poll_awake();
    let mut send_result = send_result.into_iter().filter(|msg| matches!(msg, GuestResult::Send(_, _))).collect::<Vec<_>>();

    assert!(send_result.len() != 2, "{:?}", send_result);
    assert!(send_result.len() == 1, "{:?}", send_result);

    let (send_sink_handle, data) = if let GuestResult::Send(handle, data) = send_result.pop().unwrap() { 
        (handle, data)
    } else {
        unreachable!()
    };

    assert!(send_sink_handle == sink_handle);
    let decoded = GuestPostcardEncoder.decode::<SimpleTestMessage>(data);
    assert!(decoded.value == "From remote");

    // Indicating 'ready' again should trigger the second message
    guest_runtime.sink_ready(sink_handle);

    let send_result = guest_runtime.poll_awake();
    let mut send_result = send_result.into_iter().filter(|msg| matches!(msg, GuestResult::Send(_, _))).collect::<Vec<_>>();

    assert!(send_result.len() == 1);

    let (send_sink_handle, data) = if let GuestResult::Send(handle, data) = send_result.pop().unwrap() { 
        (handle, data)
    } else {
        unreachable!()
    };

    assert!(send_sink_handle == sink_handle);
    let decoded = GuestPostcardEncoder.decode::<SimpleTestMessage>(data);
    assert!(decoded.value == "Another message");
}
