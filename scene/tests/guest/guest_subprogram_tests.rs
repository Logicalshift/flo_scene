use flo_scene::*;
use flo_scene::programs::*;
use flo_scene::guest::*;

use futures::prelude::*;

use serde::*;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SimpleTestMessage {
    value: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SimpleResponseMessage {
    value: String,
}

impl SceneMessage for SimpleTestMessage {
    fn message_type_name() -> String {
        "flo_scene_tests::guest_subprogram_tests::SimpleTestMessage".into()
    }
}

impl SceneMessage for SimpleResponseMessage {
    fn message_type_name() -> String {
        "flo_scene_tests::guest_subprogram_tests::SimpleResponseMessage".into()
    }
}

#[test]
fn run_basic_guest_subprogram() {
    let scene = Scene::default();

    let guest_subprogram_id     = SubProgramId::called("Guest subprogram");
    let sender_subprogram_id    = SubProgramId::called("Sender subprogram");
    let test_subprogram_id      = SubProgramId::called("Test subprogram");

    // Start a guest runtime that mirrors messages
    let guest_runtime = GuestRuntime::with_default_subprogram(guest_subprogram_id, GuestJsonEncoder, move |input_stream: GuestInputStream<SimpleTestMessage>, context| async move {
        // Send responses to the defualt target for the scene
        let mut response = context.send::<SimpleResponseMessage>(()).unwrap();

        let mut input_stream = input_stream;
        while let Some(msg) = input_stream.next().await {
            println!("Received message: {:?}", msg);

            response.send(SimpleResponseMessage { value: msg.value }).await.unwrap();

            println!("Sent message");
        }
    });

    // Run the guest in the scene, using the JSON encoder
    let (sender, receiver) = guest_runtime.as_streams();
    scene.add_subprogram(guest_subprogram_id, move |input: InputStream<SimpleTestMessage>, context| run_host_subprogram(input, context, GuestJsonEncoder, sender, receiver), 20);

    // Run another program to send messages to the first one
    scene.add_subprogram(sender_subprogram_id, move |_input: InputStream<()>, context| async move {
        let mut test_messages = context.send(guest_subprogram_id).unwrap();

        test_messages.send(SimpleTestMessage { value: "Hello".into() }).await.unwrap();
        test_messages.send(SimpleTestMessage { value: "Goodbyte".into() }).await.unwrap();
    }, 0);

    // Connect the programs
    scene.connect_programs(guest_subprogram_id, test_subprogram_id, StreamId::with_message_type::<SimpleResponseMessage>()).unwrap();

    TestBuilder::new()
        .expect_message(|msg: SimpleResponseMessage| { Ok(()) })
        .expect_message(|msg: SimpleResponseMessage| { Ok(()) })
        .run_in_scene(&scene, test_subprogram_id);
}
