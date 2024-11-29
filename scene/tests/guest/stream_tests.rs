use flo_scene::*;
use flo_scene::guest::*;
use flo_scene::programs::*;

use futures::prelude::*;
use futures::executor;
use futures::channel::mpsc;

use serde::*;

use std::sync::*;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SimpleTestMessage {
    value: String,
}

impl SceneMessage for SimpleTestMessage {
    fn message_type_name() -> String {
        "flo_scene_tests::stream_tests::SimpleTestMessage".into()
    }
}

#[test]
pub fn send_postcard_message_to_runtime_using_stream() {
    // The results from the guest (we're not doing any isolation stuff so we can share variables this way)
    let received = Arc::new(Mutex::new(vec![]));
    let woken    = Arc::new(Mutex::new(false));

    // Create a guest runtime
    let messages        = Arc::clone(&received);
    let awake           = Arc::clone(&woken);
    let guest_runtime   = GuestRuntime::with_default_subprogram(SubProgramId::new(), move |input_stream: GuestInputStream<SimpleTestMessage>, _context| async move {
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
        let data = postcard::to_stdvec(&SimpleTestMessage { value: "Test".into() }).unwrap();

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

#[test]
fn send_query_response_from_guest() {
    let scene = Scene::default();

    let guest_subprogram_id     = SubProgramId::called("Guest subprogram");
    let receiver_subprogram_id  = SubProgramId::called("Receiver subprogram");
    let test_subprogram_id      = SubProgramId::called("Test subprogram");

    // Start a guest runtime that mirrors messages
    let guest_runtime = GuestRuntime::with_default_subprogram(guest_subprogram_id, move |_: GuestInputStream<SimpleTestMessage>, context| async move {
        // Send responses to the defualt target for the scene
        let mut response = context.send::<QueryResponse<String>>(()).unwrap();
        let (send, recv) = mpsc::channel(0);

        response.send(QueryResponse::with_stream(recv)).await.unwrap();

        let mut send = send;
        println!("Send: Hello");
        send.send("Hello".into()).await.unwrap();
        println!("Send: Goodbyte");
        send.send("Goodbyte".into()).await.unwrap();
        println!("Finished");
    });

    // Run a receiver that receives the query from the guest and sends it on as test messages
    scene.add_subprogram(receiver_subprogram_id, move |input: InputStream<QueryResponse<String>>, context| async move {
        let mut input           = input;
        let mut query_response  = input.next().await.unwrap();
        let mut test_messages   = context.send(test_subprogram_id).unwrap();

        while let Some(msg) = query_response.next().await {
            println!("Received {:?}", msg);
            test_messages.send(SimpleTestMessage { value: msg }).await.unwrap();
        }

        println!("Response closed");
        test_messages.send(SimpleTestMessage { value: "Finished".into() }).await.unwrap();
    }, 0);

    // Run the guest in the scene, using the JSON encoder
    let (sender, receiver) = guest_runtime.as_streams();
    scene.add_subprogram(guest_subprogram_id, move |input: InputStream<SimpleTestMessage>, context| run_host_subprogram(input, context, sender, receiver), 20);

    // Connect the programs
    scene.connect_programs(receiver_subprogram_id, test_subprogram_id, StreamId::with_message_type::<SimpleTestMessage>()).unwrap();
    scene.connect_programs(guest_subprogram_id, receiver_subprogram_id, StreamId::with_message_type::<QueryResponse<String>>()).unwrap();

    TestBuilder::new()
        .expect_message(|msg: SimpleTestMessage| { if msg.value == "Hello" { Ok(()) } else { Err(format!("Value is {} (should be Hello)", msg.value)) } })
        .expect_message(|msg: SimpleTestMessage| { if msg.value == "Goodbyte" { Ok(()) } else { Err(format!("Value is {} (should be Goodbyte)", msg.value)) } })
        .expect_message(|msg: SimpleTestMessage| { if msg.value == "Finished" { Ok(()) } else { Err(format!("Value is {} (should be Finished)", msg.value)) } })
        .run_in_scene(&scene, test_subprogram_id);
}

#[test]
fn send_query_response_from_host() {
    let scene = Scene::default();

    let guest_subprogram_id     = SubProgramId::called("Guest subprogram");
    let sender_subprogram_id    = SubProgramId::called("Receiver subprogram");
    let test_subprogram_id      = SubProgramId::called("Test subprogram");

    // Start a guest runtime that mirrors messages
    let guest_runtime = GuestRuntime::with_default_subprogram(guest_subprogram_id, move |input: GuestInputStream<QueryResponse<String>>, context| async move {
        println!("Waiting for input");

        let mut input           = input;
        let mut query_response  = input.next().await.unwrap();
        let mut test_messages   = context.send(test_subprogram_id).unwrap();

        println!("Receiving from query");
        while let Some(msg) = query_response.next().await {
            println!("Received {:?}", msg);
            test_messages.send(SimpleTestMessage { value: msg }).await.unwrap();
        }

        test_messages.send(SimpleTestMessage { value: "Finished".into() }).await.unwrap();
    });

    // Run a receiver that receives the query from the guest and sends it on as test messages
    scene.add_subprogram(sender_subprogram_id, move |_: InputStream<()>, context| async move {
        // Send responses to the default target for the scene
        let mut response = context.send::<QueryResponse<String>>(()).unwrap();
        let (send, recv) = mpsc::channel(0);

        println!("  Sending query");
        response.send(QueryResponse::with_stream(recv)).await.map_err(|err| err.map(|_| ())).unwrap();

        let mut send = send;
        println!("  Sending first message");
        send.send("Hello".into()).await.unwrap();
        println!("  Sending second message");
        send.send("Goodbyte".into()).await.unwrap();
        println!("  Closing stream");
    }, 0);

    // Run the guest in the scene, using the JSON encoder
    let (sender, receiver) = guest_runtime.as_streams();
    scene.add_subprogram(guest_subprogram_id, move |input: InputStream<QueryResponse<String>>, context| run_host_subprogram(input, context, sender, receiver), 20);

    // Connect the programs
    scene.connect_programs(guest_subprogram_id, test_subprogram_id, StreamId::with_message_type::<SimpleTestMessage>()).unwrap();
    scene.connect_programs(sender_subprogram_id, guest_subprogram_id, StreamId::with_message_type::<QueryResponse<String>>()).unwrap();

    TestBuilder::new()
        .expect_message(|msg: SimpleTestMessage| { if msg.value == "Hello" { Ok(()) } else { Err(format!("Value is {} (should be Hello)", msg.value)) } })
        .expect_message(|msg: SimpleTestMessage| { if msg.value == "Goodbyte" { Ok(()) } else { Err(format!("Value is {} (should be Goodbyte)", msg.value)) } })
        .expect_message(|msg: SimpleTestMessage| { if msg.value == "Finished" { Ok(()) } else { Err(format!("Value is {} (should be Finished)", msg.value)) } })
        .run_in_scene(&scene, test_subprogram_id);
}
