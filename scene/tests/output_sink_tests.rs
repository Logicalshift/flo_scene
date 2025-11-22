use flo_scene::*;
use flo_scene::programs::*;

use futures::prelude::*;
use futures::stream;

use serde::*;

#[derive(Serialize, Deserialize)]
struct TestMessage;

impl SceneMessage for TestMessage { }

#[test]
fn send_stream_to_output_sink() {
    // Create a scene and a stream of messages we want to send
    let scene   = Scene::default();
    let stream  = stream::iter(vec![TestMessage, TestMessage, TestMessage]);       

    // Send the stream in a subprogram
    let stream_subprogram   = SubProgramId::new();
    let test_subprogram     = SubProgramId::new();

    scene.add_subprogram(stream_subprogram, move |input: InputStream<()>, context| async move {
        // Send the stream to an output of this program
        context.send(()).unwrap().send_stream(stream);

        // Read from the input to keep this program running
        let mut input = input;
        while let Some(_) = input.next().await {
        }
    }, 0);

    scene.connect_programs((), test_subprogram, StreamId::with_message_type::<TestMessage>()).unwrap();

    TestBuilder::new()
        .expect_message(|_msg: TestMessage| Ok(()))
        .expect_message(|_msg: TestMessage| Ok(()))
        .expect_message(|_msg: TestMessage| Ok(()))
        .run_in_scene(&scene, test_subprogram);
}
