use flo_scene::*;
use flo_scene::programs::*;

use futures::prelude::*;
use futures::stream;
use futures::channel::mpsc;
use futures::channel::oneshot;

use serde::*;

#[derive(Serialize, Deserialize)]
struct TestMessage(String);

impl SceneMessage for TestMessage { }

impl TestMessage {
    fn assert(&self, msg: &str) -> Result<(), String> {
        if msg == self.0 {
            Ok(())
        } else {
            Err(format!("Expected '{}' but got '{}'", msg, self.0))
        }
    }
}

#[test]
fn send_stream_to_output_sink() {
    // Create a scene and a stream of messages we want to send
    let scene   = Scene::default();
    let stream  = stream::iter(vec![TestMessage("1".to_string()), TestMessage("2".to_string()), TestMessage("3".to_string())]);       

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
        .expect_message(|msg: TestMessage| msg.assert("1"))
        .expect_message(|msg: TestMessage| msg.assert("2"))
        .expect_message(|msg: TestMessage| msg.assert("3"))
        .run_in_scene(&scene, test_subprogram);
}

#[test]
fn stop_when_parent_stops() {
    // Create a scene and a MPSC channel
    let scene           = Scene::default();
    let (send, recv)    = mpsc::channel::<TestMessage>(1);

    // Also need a way to signal that the streaming subprogram has stopped (so we know the stream itself should be shutting down)
    let (send_stopped, recv_stopped) = oneshot::channel::<()>();

    // We'll use one program to stream results, and another to send them
    let stream_subprogram   = SubProgramId::new();
    let send_subprogram     = SubProgramId::new();
    let test_subprogram     = SubProgramId::new();

    // Stream subprogram just sends the 'recv' stream
    scene.add_subprogram(stream_subprogram, move |input: InputStream<()>, context| async move {
        // Send the stream to an output of this program
        context.send(()).unwrap().send_stream(recv);

        // Read from the input to keep this program running
        let mut input = input;
        while let Some(_) = input.next().await {
        }

        // Indicate that this program is shutting down
        send_stopped.send(()).unwrap();
    }, 0);

    scene.add_subprogram(send_subprogram, move |input, context| async move {
        // Receive control messages
        context.send_message(SceneControl::Subscribe(send_subprogram.into())).await.unwrap();

        // Send the first message
        let mut send = send;
        send.send(TestMessage("Initial message".to_string())).await.unwrap();

        // Stop the stream program (and wait for it to stop)
        context.send_message(SceneControl::Close(stream_subprogram)).await.unwrap();
        recv_stopped.await.unwrap();

        // Wait for the control program to indicate the stream subprogram has stopped
        let mut input = input;
        while let Some(input) = input.next().await {
            if let SceneUpdate::Stopped(program_id) = input {
                if program_id == stream_subprogram {
                    // Stream subprogram has stopped
                    break;
                }
            }
        }

        // Indicate the test program is stopped
        context.send_message(TestMessage("Stopped test program".to_string())).await.unwrap();

        // Wait for another 'stopped' message indicating that the spawned stream program has stopped
        // (Should always stop after the stream program, as it's when the stream program returns that the spawned program is shut down)
        while let Some(input) = input.next().await {
            if let SceneUpdate::Stopped(_) = input {
                break;
            }
        }

        context.send_message(TestMessage("Stopped streaming program".to_string())).await.unwrap();

        // The stream should be shut down
        if let Err(_) = send.send(TestMessage("Should no longer be being sent".to_string())).await {
            context.send_message(TestMessage("Refusing more messages".to_string())).await.unwrap();
        } else {
            context.send_message(TestMessage("Should not have been able to send this message".to_string())).await.unwrap();
        }
    }, 200);

    scene.connect_programs((), test_subprogram, StreamId::with_message_type::<TestMessage>()).unwrap();

    TestBuilder::new()
        .expect_message(|msg: TestMessage| msg.assert("Initial message"))
        .expect_message(|msg: TestMessage| msg.assert("Stopped test program"))
        .expect_message(|msg: TestMessage| msg.assert("Stopped streaming program"))
        .expect_message(|msg: TestMessage| msg.assert("Refusing more messages"))
        .run_in_scene(&scene, test_subprogram);
}
