use flo_scene::*;
use flo_scene::programs::*;

use futures::prelude::*;
use serde::*;

#[test]
fn auto_start_on_first_message() {
    // Define a message with an initialisation routine that starts the default subprogram
    #[derive(Serialize, Deserialize)]
    struct AutoStartMessage;

    impl SceneMessage for AutoStartMessage {
        fn initialise(scene: &Scene) {
            // When the message is initialised, create a program and redirect everything there
            scene.add_subprogram(SubProgramId::called("AutoStart"),
                |mut input_stream: InputStream<AutoStartMessage>, _context| async move {
                    while let Some(_) = input_stream.next().await { }
                }, 0);

            scene.connect_programs((), SubProgramId::called("AutoStart"), StreamId::with_message_type::<AutoStartMessage>()).unwrap();
        }
    }

    // Create a scene, but don't start the 'auto start' program
    let scene = Scene::default();

    // Try sending a message to it (should start up when it's first encountered)
    TestBuilder::new()
        .send_message(AutoStartMessage)
        .send_message(AutoStartMessage)
        .run_in_scene(&scene, SubProgramId::new());
}

#[test]
fn auto_start_on_connect() {
    // Define a message with an initialisation routine that starts the default subprogram
    #[derive(Serialize, Deserialize)]
    struct AutoStartMessage;

    impl SceneMessage for AutoStartMessage {
        fn initialise(scene: &Scene) {
            // When the message is initialised, create a program and redirect everything there
            scene.add_subprogram(SubProgramId::called("AutoStart"),
                |mut input_stream: InputStream<AutoStartMessage>, _context| async move {
                    while let Some(_) = input_stream.next().await { }
                }, 0);
        }
    }

    // Create a scene, but don't start the 'auto start' program
    let scene = Scene::default();

    // Connect the auto-start program as if it's already initialised in the stream
    scene.connect_programs((), SubProgramId::called("AutoStart"), StreamId::with_message_type::<AutoStartMessage>()).unwrap();

    // Try sending a message to it (should start up when it's first encountered)
    TestBuilder::new()
        .send_message(AutoStartMessage)
        .send_message(AutoStartMessage)
        .run_in_scene(&scene, SubProgramId::new());
}

#[test]
fn connect_before_program_start() {
    use futures::task::{Poll};

    #[derive(Serialize, Deserialize, Debug)]
    struct TestMessage;

    #[derive(Serialize, Deserialize, Debug)]
    struct ReadyMessage;

    impl SceneMessage for TestMessage { }
    impl SceneMessage for ReadyMessage { }

    // We have two programs. The sending_program sets up a connection to the receiving program before that program is started, then tries to send a message
    // to it, and only then actually starts the program
    let test_program        = SubProgramId::called("test_program");
    let sending_program     = SubProgramId::called("sending_program");
    let receiving_program   = SubProgramId::called("receiving_program");

    let scene = Scene::default();

    // Connect anything to the receving program (but the receiving program is not created yet)
    scene.connect_programs((), receiving_program, StreamId::with_message_type::<TestMessage>()).unwrap();

    // Add a subprogram that tries to send a TestMessage, then starts the receving program
    let scene2 = scene.clone();
    scene.add_subprogram(sending_program, move |_: InputStream<()>, context| async move {
        println!("Connecting to 'TestMessage'");

        // Create a future to send a message
        let mut channel         = context.send(()).unwrap();
        let mut send_message    = channel.send(TestMessage);

        // Wait until it blocks so we know it's waiting for the program to start
        println!("Waiting for send to block");
        future::poll_fn(|context| {
            match send_message.poll_unpin(context) {
                Poll::Ready(Ok(_))      => panic!("Message finished sending unexpectedly"),
                Poll::Ready(Err(err))   => panic!("Error: {:?}", err),
                Poll::Pending           => Poll::Ready(()),
            }
        }).await;

        // Start the receiving subprogram: this just receives test messages and sends 'ReadyMessage' to the test program
        println!("Starting receiving subprogram");
        scene2.add_subprogram(receiving_program, |input, context| async move {
            println!("Receiving subprogram started");

            let mut input = input;
            while let Some(message) = input.next().await {
                let _message: TestMessage = message;

                println!("Received message, sending 'Ready'");
                context.send(test_program).unwrap()
                    .send(ReadyMessage).await.unwrap();
            }
        }, 20);

        // Finish sending the message we started (should reconnect to the receiving program)
        println!("Waiting for message to finish sending");
        send_message.await.unwrap();
        println!("Message has finished sending");
    }, 0);

    // Wait for the 'ReadyMessage' to be generated
    println!("Running test");
    TestBuilder::new()
        .expect_message::<ReadyMessage>(|_| Ok(()))
        .run_in_scene(&scene, test_program);
}

#[test]
fn send_before_connect_before_program_start() {
    use futures::task::{Poll};

    #[derive(Serialize, Deserialize, Debug)]
    struct TestMessage;

    #[derive(Serialize, Deserialize, Debug)]
    struct ReadyMessage;

    impl SceneMessage for TestMessage { }
    impl SceneMessage for ReadyMessage { }

    // We have two programs. The sending_program sets up a connection to the receiving program before that program is started, then tries to send a message
    // to it, and only then actually starts the program
    let test_program        = SubProgramId::called("test_program");
    let sending_program     = SubProgramId::called("sending_program");
    let receiving_program   = SubProgramId::called("receiving_program");

    let scene = Scene::default();

    // Add a subprogram that tries to send a TestMessage, then starts the receving program
    let scene2 = scene.clone();
    scene.add_subprogram(sending_program, move |_: InputStream<()>, context| async move {
        println!("Connecting to 'TestMessage'");

        // Create a future to send a message
        let mut channel         = context.send(()).unwrap();
        let mut send_message    = channel.send(TestMessage);

        // Wait until it blocks so we know it's waiting for the program to start
        println!("Waiting for send to block");
        future::poll_fn(|context| {
            match send_message.poll_unpin(context) {
                Poll::Ready(Ok(_))      => panic!("Message finished sending unexpectedly"),
                Poll::Ready(Err(err))   => panic!("Error: {:?}", err),
                Poll::Pending           => Poll::Ready(()),
            }
        }).await;

        // Connect anything to the receving program (but the receiving program is not created yet)
        scene2.connect_programs((), receiving_program, StreamId::with_message_type::<TestMessage>()).unwrap();

        // Start the receiving subprogram: this just receives test messages and sends 'ReadyMessage' to the test program
        println!("Starting receiving subprogram");
        scene2.add_subprogram(receiving_program, |input, context| async move {
            println!("Receiving subprogram started");

            let mut input = input;
            while let Some(message) = input.next().await {
                let _message: TestMessage = message;

                println!("Received message, sending 'Ready'");
                context.send(test_program).unwrap()
                    .send(ReadyMessage).await.unwrap();
            }
        }, 20);

        // Finish sending the message we started (should reconnect to the receiving program)
        println!("Waiting for message to finish sending");
        send_message.await.unwrap();
        println!("Message has finished sending");
    }, 0);

    // Wait for the 'ReadyMessage' to be generated
    println!("Running test");
    TestBuilder::new()
        .expect_message::<ReadyMessage>(|_| Ok(()))
        .run_in_scene(&scene, test_program);
}

#[test]
fn connect_before_program_start_without_waiting() {
    #[derive(Serialize, Deserialize, Debug)]
    struct TestMessage;

    #[derive(Serialize, Deserialize, Debug)]
    struct ReadyMessage;

    impl SceneMessage for TestMessage { }
    impl SceneMessage for ReadyMessage { }

    // We have two programs. The sending_program sets up a connection to the receiving program before that program is started, then tries to send a message
    // to it, and only then actually starts the program
    let test_program        = SubProgramId::called("test_program");
    let sending_program     = SubProgramId::called("sending_program");
    let receiving_program   = SubProgramId::called("receiving_program");

    let scene = Scene::default();

    // Connect anything to the receving program (but the receiving program is not created yet)
    scene.connect_programs((), receiving_program, StreamId::with_message_type::<TestMessage>()).unwrap();

    // Add a subprogram that tries to send a TestMessage, then starts the receving program
    let scene2 = scene.clone();
    scene.add_subprogram(sending_program, move |_: InputStream<()>, context| async move {
        // Create a future to send a message
        let mut channel     = context.send(()).unwrap();
        let send_message    = channel.send(TestMessage);

        // Start the receiving subprogram: this just receives test messages and sends 'ReadyMessage' to the test program
        scene2.add_subprogram(receiving_program, |input, context| async move {
            let mut input = input;
            while let Some(message) = input.next().await {
                let _message: TestMessage = message;
                context.send(test_program).unwrap()
                    .send(ReadyMessage).await.unwrap();
            }
        }, 20);

        // Finish sending the message we started (should reconnect to the receiving program)
        send_message.await.unwrap();
    }, 0);

    // Wait for the 'ReadyMessage' to be generated
    TestBuilder::new()
        .expect_message::<ReadyMessage>(|_| Ok(()))
        .run_in_scene(&scene, test_program);
}
