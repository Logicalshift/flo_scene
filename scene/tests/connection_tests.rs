use flo_scene::*;
use flo_scene::programs::*;

use futures::prelude::*;

#[test]
pub fn connect_two_subprograms() {
    // Scene with two programs that we'll connect together
    let scene           = Scene::default();
    let test_program    = SubProgramId::new();
    let program_1       = SubProgramId::new();
    let program_2       = SubProgramId::new();

    // Program_1 sends a string message
    scene.add_subprogram(program_1, |_: InputStream<()>, context| {
        async move {
            // Create a stream to send strings to the default target (will be created disconnected)
            let mut send_strings = context.send(()).unwrap();

            // Send the string to the control programs (sometimes the control program will make the connection first, sometimes it'll happen after we start to send)
            send_strings.send("Test".to_string()).await.unwrap();
        }
    }, 0);

    // Program 2 receives the message and sends it to the test program
    scene.add_subprogram(program_2, move |input: InputStream<String>, context| {
        async move {
            let mut test_program = context.send(test_program).unwrap();

            let mut input = input;
            while let Some(input) = input.next().await {
                test_program.send(input).await.unwrap();
            }
        }
    }, 0);

    // Connect the two programs
    scene.connect_programs(program_1, program_2, StreamId::with_message_type::<String>()).unwrap();

    // Check that we receive the test message
    TestBuilder::new()
        .expect_message(|_msg: String| { Ok(()) })
        .run_in_scene(&scene, test_program);
}

#[test]
pub fn connect_two_subprograms_using_filter() {
    // Scene with two programs that we'll connect together
    let scene           = Scene::default();
    let test_program    = SubProgramId::new();
    let program_1       = SubProgramId::new();
    let program_2       = SubProgramId::new();

    // TestMessage can be filtered into a string, but we don't set the filter up
    #[derive(Debug)]
    struct TestMessage;
    impl SceneMessage for TestMessage { }

    let test_string_filter = FilterHandle::for_filter(|messages| messages.map(|_: TestMessage| "Test".to_string()));

    // Program_1 sends a TestMessage
    scene.add_subprogram(program_1, |_: InputStream<()>, context| {
        async move {
            // Create a stream to TestMessages to the default target (will be created disconnected)
            let mut send_strings = context.send(()).unwrap();

            // Send the string to the control programs (sometimes the control program will make the connection first, sometimes it'll happen after we start to send)
            send_strings.send("Test".to_string()).await.unwrap();
        }
    }, 0);

    // Program 2 receives the message and sends it to the test program
    scene.add_subprogram(program_2, move |input: InputStream<String>, context| {
        async move {
            let mut test_program = context.send(test_program).unwrap();

            let mut input = input;
            while let Some(input) = input.next().await {
                test_program.send(input).await.unwrap();
            }
        }
    }, 0);

    // Connect testmessages and strings to program 2, filtering the test messages as we go
    scene.connect_programs((), program_2, StreamId::with_message_type::<String>()).unwrap();
    scene.connect_programs((), StreamTarget::Filtered(test_string_filter, program_2), StreamId::with_message_type::<TestMessage>()).unwrap();

    // Check that we receive the test message
    TestBuilder::new()
        .expect_message(|_msg: String| { Ok(()) })
        .run_in_scene(&scene, test_program);
}

#[test]
pub fn connect_two_subprograms_after_creating_stream() {
    // Scene with two programs that we'll connect together
    let scene           = Scene::default();
    let test_program    = SubProgramId::new();
    let program_1       = SubProgramId::new();
    let program_2       = SubProgramId::new();

    // Program_1 sends a string message
    scene.add_subprogram(program_1, |_: InputStream<()>, context| {
        async move {
            // Create a stream to send strings to the default target (will be created disconnected)
            let mut send_strings = context.send(()).unwrap();

            // Tell the control program to connect program1 to program2
            context.send_message(SceneControl::connect(program_1, program_2, StreamId::with_message_type::<String>())).await.unwrap();

            // Send the string to the control programs (sometimes the control program will make the connection first, sometimes it'll happen after we start to send)
            send_strings.send("Test".to_string()).await.unwrap();
        }
    }, 0);

    // Program 2 receives the message and sends it to the test program
    scene.add_subprogram(program_2, move |input: InputStream<String>, context| {
        async move {
            let mut test_program = context.send(test_program).unwrap();

            let mut input = input;
            while let Some(input) = input.next().await {
                test_program.send(input).await.unwrap();
            }
        }
    }, 0);

    // Check that we receive the test message
    TestBuilder::new()
        .expect_message(|_msg: String| { Ok(()) })
        .run_in_scene(&scene, test_program);
}

#[test]
pub fn connect_default_subprogram_after_creating_stream() {
    // Scene with two programs that we'll connect together
    let scene           = Scene::default();
    let test_program    = SubProgramId::new();
    let program_1       = SubProgramId::new();
    let program_2       = SubProgramId::new();

    // Program_1 sends a string message
    scene.add_subprogram(program_1, |_: InputStream<()>, context| {
        async move {
            // Create a stream to send strings to the default target
            let mut send_strings = context.send(()).unwrap();

            // Tell the control program to connect all strings to the target
            context.send_message(SceneControl::connect((), program_2, StreamId::with_message_type::<String>())).await.unwrap();

            // Send the string to the control programs
            send_strings.send("Test".to_string()).await.unwrap();
        }
    }, 0);

    // Program 2 receives the message and sends it to the test program
    scene.add_subprogram(program_2, move |input: InputStream<String>, context| {
        async move {
            let mut test_program = context.send(test_program).unwrap();

            let mut input = input;
            while let Some(input) = input.next().await {
                test_program.send(input).await.unwrap();
            }
        }
    }, 0);

    // Check that we receive the test message
    TestBuilder::new()
        .expect_message(|_msg: String| { Ok(()) })
        .run_in_scene(&scene, test_program);
}

#[test]
pub fn connect_two_subprograms_before_creating() {
    // Scene with two programs that we'll connect together
    let scene           = Scene::default();
    let test_program    = SubProgramId::new();
    let program_1       = SubProgramId::new();
    let program_2       = SubProgramId::new();

    // Create the connection between the two programs, before the programs are started
    scene.connect_programs(program_1, program_2, StreamId::with_message_type::<String>()).unwrap();

    // Program_1 sends a string message
    scene.add_subprogram(program_1, |_: InputStream<()>, context| {
        async move {
            context.send_message("Test".to_string()).await.unwrap();
        }
    }, 0);

    // Program 2 receives the message and sends it to the test program
    scene.add_subprogram(program_2, move |input: InputStream<String>, context| {
        async move {
            let mut test_program = context.send(test_program).unwrap();

            let mut input = input;
            while let Some(input) = input.next().await {
                test_program.send(input).await.unwrap();
            }
        }
    }, 0);

    // Check that we receive the test message
    TestBuilder::new()
        .expect_message(|_msg: String| { Ok(()) })
        .run_in_scene(&scene, test_program);
}

#[test]
pub fn connect_default_subprograms_before_creating() {
    // Scene with two programs that we'll connect together
    let scene           = Scene::default();
    let test_program    = SubProgramId::new();
    let program_1       = SubProgramId::new();
    let program_2       = SubProgramId::new();

    // Specify that any strings get sent to program 2, before the scene is started
    scene.connect_programs((), program_2, StreamId::with_message_type::<String>()).unwrap();

    // Program_1 sends a string message
    scene.add_subprogram(program_1, |_: InputStream<()>, context| {
        async move {
            context.send_message("Test".to_string()).await.unwrap();
        }
    }, 0);

    // Program 2 receives the message and sends it to the test program
    scene.add_subprogram(program_2, move |input: InputStream<String>, context| {
        async move {
            let mut test_program = context.send(test_program).unwrap();

            let mut input = input;
            while let Some(input) = input.next().await {
                test_program.send(input).await.unwrap();
            }
        }
    }, 0);

    // Check that we receive the test message
    TestBuilder::new()
        .expect_message(|_msg: String| { Ok(()) })
        .run_in_scene(&scene, test_program);
}

#[test]
pub fn connect_two_subprograms_after_creating_stream_using_filter_target() {
    // Scene with two programs that we'll connect together
    let scene           = Scene::default();
    let test_program    = SubProgramId::new();
    let program_1       = SubProgramId::new();
    let program_2       = SubProgramId::new();

    // TestMessage can be filtered into a string, but we don't set the filter up
    #[derive(Debug)]
    struct TestMessage;
    impl SceneMessage for TestMessage { }

    let test_string_filter = FilterHandle::for_filter(|messages| messages.map(|_: TestMessage| "Test".to_string()));

    // Program_1 sends a TestMessage
    scene.add_subprogram(program_1, |_: InputStream<()>, context| {
        async move {
            // Create a stream to send TestMessages to program_2 (but it accepts strings)
            let mut send_strings = context.send(program_2).unwrap();

            // Tell the control program to filter the connection between program 1 and 2
            context.send_message(SceneControl::connect(program_1, StreamTarget::Filtered(test_string_filter, program_2), StreamId::with_message_type::<TestMessage>())).await.unwrap();

            // Send the string to the control programs (sometimes the control program will make the connection first, sometimes it'll happen after we start to send)
            send_strings.send(TestMessage).await.unwrap();
        }
    }, 0);

    // Program 2 receives the message and sends it to the test program
    scene.add_subprogram(program_2, move |input: InputStream<String>, context| {
        async move {
            let mut test_program = context.send(test_program).unwrap();

            let mut input = input;
            while let Some(input) = input.next().await {
                test_program.send(input).await.unwrap();
            }
        }
    }, 0);

    // Check that we receive the test message
    TestBuilder::new()
        .expect_message(|_msg: String| { Ok(()) })
        .run_in_scene(&scene, test_program);
}

#[test]
pub fn connect_default_after_creating_stream_using_filter_target() {
    // Scene with two programs that we'll connect together
    let scene           = Scene::default();
    let test_program    = SubProgramId::new();
    let program_1       = SubProgramId::new();
    let program_2       = SubProgramId::new();

    // TestMessage can be filtered into a string, but we don't set the filter up
    #[derive(Debug)]
    struct TestMessage;
    impl SceneMessage for TestMessage { }

    let test_string_filter = FilterHandle::for_filter(|messages| messages.map(|_: TestMessage| "Test".to_string()));

    // Program_1 sends a TestMessage
    scene.add_subprogram(program_1, |_: InputStream<()>, context| {
        async move {
            // Create a stream to send TestMessages to program_2 (but it accepts strings)
            let mut send_strings = context.send(()).unwrap();

            // Tell the control program to filter the connection to program_2
            context.send_message(SceneControl::connect((), StreamTarget::Filtered(test_string_filter, program_2), StreamId::with_message_type::<TestMessage>())).await.unwrap();

            // Send the string to the control programs (sometimes the control program will make the connection first, sometimes it'll happen after we start to send)
            send_strings.send(TestMessage).await.unwrap();
        }
    }, 0);

    // Program 2 receives the message and sends it to the test program
    scene.add_subprogram(program_2, move |input: InputStream<String>, context| {
        async move {
            let mut test_program = context.send(test_program).unwrap();

            let mut input = input;
            while let Some(input) = input.next().await {
                test_program.send(input).await.unwrap();
            }
        }
    }, 0);

    // Check that we receive the test message
    TestBuilder::new()
        .expect_message(|_msg: String| { Ok(()) })
        .run_in_scene(&scene, test_program);
}
