//!
//! This tests the various ways that connections can be established
//!
//! The current design produces a lot of ways this can happen. It's difficult to simplify this without reducing
//! the functionality of the system, though a possible avenue of attack is to combine the source and target filters
//! in a way that supports the existing functionality somehow.
//!
//! There are three types of source ('Any', a subprogram ID and a filtered stream)
//! There are four types of target ('None', 'All', a subprogram ID, and a subprogram ID with a filter attached)
//!
//! The type of stream is specified with a stream ID. Stream IDs are generally untargeted, but subprograms can 
//! request connections to specific subprogram IDs, which uses a stream ID annotated with that target.
//!
//! Here are the general rules:
//!
//! * Connections can be set up before the target subprogram has started (this makes initialization easier, 
//!     especially when talking to the scene control program) 
//! * Connections can be set up after the output sink has been opened (which allows dynamic configuration and can 
//!     also arise out of race conditions)
//! * Source filters can be used to define ways that connections can be rewritten to attach to new targets
//!     (this is useful for messages like 'query' or 'subscribe' where they always need to go through a filter
//!     to be useful)
//! * Target filters can be used to enable a subprogram to accept input types other than its defaults
//!     (this is the more usual type of filtering where we want to be able to adapt the output of one
//!     program to the input of another without the two needing to know about each other)
//! * Filters usually don't combine, but source filters do need to chain with target filters sometimes
//!     (otherwise you have to re-declare the source filters when using a target filter to adapt something)
//! * Connections can use 'Any' as the source to create a default connection without needing to connect every subprogram
//!     (which is just generally useful, but a pain when rewriting existing connections)
//! * Subprograms can create connections to specific subprogram targets, these can be identified and overridden by a 
//!     stream ID with a target if needed
//!     (very uncommon to actually need to do this, except in tests where it's very useful indeed, in the future this
//!     is probably quite useful for 'live patching' software that's running too)
//!
//! (Internally, a connection is either made when sink_for_target() is called in scene_core, or when finish_connecting_programs()
//! is called. The former is for when the connections are made ahead of time, and the latter has to deal with reconnecting
//! existing programs, which is the harder case)
//!

use flo_scene::*;
use flo_scene::programs::*;

use futures::prelude::*;
use serde::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
struct TestResult(String);
impl SceneMessage for TestResult { }

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
                test_program.send(TestResult(input)).await.unwrap();
            }
        }
    }, 0);

    // Connect the two programs
    scene.connect_programs(program_1, program_2, StreamId::with_message_type::<String>()).unwrap();

    // Check that we receive the test message
    TestBuilder::new()
        .expect_message(|_msg: TestResult| { Ok(()) })
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
    #[derive(Debug, Serialize, Deserialize)]
    struct TestMessage;
    impl SceneMessage for TestMessage { }

    let test_string_filter = FilterHandle::for_filter(|messages| messages.map(|_: TestMessage| "Test".to_string()));

    // Program_1 sends a TestMessage
    scene.add_subprogram(program_1, |_: InputStream<()>, context| {
        async move {
            // Create a stream to TestMessages to the default target (will be created disconnected)
            let mut send_strings = context.send(()).unwrap();

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
                test_program.send(TestResult(input)).await.unwrap();
            }
        }
    }, 0);

    // Connect testmessages and strings to program 2, filtering the test messages as we go
    scene.connect_programs((), program_2, StreamId::with_message_type::<String>()).unwrap();
    scene.connect_programs((), StreamTarget::Filtered(test_string_filter, program_2), StreamId::with_message_type::<TestMessage>()).unwrap();

    // Check that we receive the test message
    TestBuilder::new()
        .expect_message(|_msg: TestResult| { Ok(()) })
        .run_in_scene(&scene, test_program);
}

#[test]
pub fn connect_two_subprograms_using_filter_then_all_no_delay() {
    // Scene with two programs that we'll connect together
    let scene           = Scene::default();
    let test_program    = SubProgramId::called("test_program");
    let program_1       = SubProgramId::called("program_1");
    let program_2       = SubProgramId::called("program_2");

    // TestMessage can be filtered into a string, but we don't set the filter up
    #[derive(Debug, Serialize, Deserialize)]
    struct TestMessage;
    impl SceneMessage for TestMessage { }

    let test_string_filter = FilterHandle::for_filter(|messages| messages.map(|_: TestMessage| "Test".to_string()));

    // Program_1 sends a TestMessage
    scene.add_subprogram(program_1, |_: InputStream<()>, context| {
        async move {
            // Create a stream to TestMessages to the default target (will be created disconnected)
            let mut send_strings = context.send(()).unwrap();

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
                test_program.send(TestResult(input)).await.unwrap();
            }
        }
    }, 0);

    // Connect testmessages and strings to program 2, filtering the test messages as we go
    scene.connect_programs((), program_2, StreamId::with_message_type::<String>()).unwrap();
    scene.connect_programs((), program_2, StreamId::with_message_type::<TestMessage>()).unwrap();
    scene.connect_programs((), StreamTarget::Filtered(test_string_filter, program_2), StreamId::with_message_type::<TestMessage>()).unwrap();

    // Check that we receive the test message
    TestBuilder::new()
        .expect_message(|_msg: TestResult| { Ok(()) })
        .run_in_scene(&scene, test_program);
}

#[test]
pub fn connect_default_using_source_filter_after_all() {
    // Scene with two programs that we'll connect together
    let scene           = Scene::default();
    let test_program    = SubProgramId::called("test_program");
    let program_1       = SubProgramId::called("program_1");
    let program_2       = SubProgramId::called("program_2");

    // TestMessage can be filtered into a string, but we don't set the filter up
    #[derive(Debug, Serialize, Deserialize)]
    struct TestMessage;
    impl SceneMessage for TestMessage { }

    let test_string_filter = FilterHandle::for_filter(|messages| messages.map(|_: TestMessage| "Test".to_string()));

    // Program_1 sends a TestMessage
    scene.add_subprogram(program_1, |_: InputStream<()>, context| {
        async move {
            // Create a stream to TestMessages to the default target (will be created disconnected)
            let mut send_strings = context.send(()).unwrap();

            // Tell the control program to add the source filter that we need to make the programs talk
            context.send_message(SceneControl::connect(StreamSource::Filtered(test_string_filter), (), StreamId::with_message_type::<TestMessage>())).await.unwrap();

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
                test_program.send(TestResult(input)).await.unwrap();
            }
        }
    }, 0);

    // Connect testmessages and strings to program 2, filtering the test messages as we go
    scene.connect_programs((), program_2, StreamId::with_message_type::<TestMessage>()).unwrap();
    scene.connect_programs((), program_2, StreamId::with_message_type::<String>()).unwrap();

    // Check that we receive the test message
    TestBuilder::new()
        .expect_message(|_msg: TestResult| { Ok(()) })
        .run_in_scene(&scene, test_program);
}

#[test]
pub fn connect_default_using_source_filter_after_all_no_delay() {
    // Scene with two programs that we'll connect together
    let scene           = Scene::default();
    let test_program    = SubProgramId::called("test_program");
    let program_1       = SubProgramId::called("program_1");
    let program_2       = SubProgramId::called("program_2");

    // TestMessage can be filtered into a string, but we don't set the filter up
    #[derive(Debug, Serialize, Deserialize)]
    struct TestMessage;
    impl SceneMessage for TestMessage { }

    let test_string_filter = FilterHandle::for_filter(|messages| messages.map(|_: TestMessage| "Test".to_string()));

    // Program_1 sends a TestMessage
    scene.add_subprogram(program_1, |_: InputStream<()>, context| {
        async move {
            // Create a stream to TestMessages to the default target (will be created disconnected)
            let mut send_strings = context.send(()).unwrap();

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
                test_program.send(TestResult(input)).await.unwrap();
            }
        }
    }, 0);

    // Connect testmessages and strings to program 2, filtering the test messages as we go
    scene.connect_programs((), program_2, StreamId::with_message_type::<TestMessage>()).unwrap();
    scene.connect_programs((), program_2, StreamId::with_message_type::<String>()).unwrap();
    scene.connect_programs(StreamSource::Filtered(test_string_filter), (), StreamId::with_message_type::<TestMessage>()).unwrap();

    // Check that we receive the test message
    TestBuilder::new()
        .expect_message(|_msg: TestResult| { Ok(()) })
        .run_in_scene(&scene, test_program);
}

#[test]
pub fn connect_default_using_chained_filter() {
    // Scene with two programs that we'll connect together
    let scene           = Scene::default();
    let test_program    = SubProgramId::called("test_program");
    let program_1       = SubProgramId::called("program_1");
    let program_2       = SubProgramId::called("program_2");

    // TestMessage can be filtered into a string, but we don't set the filter up
    #[derive(Debug, Serialize, Deserialize)]
    struct TestMessage1;
    impl SceneMessage for TestMessage1 { }

    #[derive(Debug, Serialize, Deserialize)]
    struct TestMessage2;
    impl SceneMessage for TestMessage2 { }

    let test_message_filter = FilterHandle::for_filter(|messages| messages.map(|_: TestMessage1| TestMessage2));
    let test_string_filter = FilterHandle::for_filter(|messages| messages.map(|_: TestMessage2| "Test".to_string()));

    // Program_1 sends a TestMessage
    scene.add_subprogram(program_1, |_: InputStream<()>, context| {
        async move {
            // Create a stream to TestMessages to the default target (will be created disconnected)
            let mut send_strings = context.send(()).unwrap();

            // Send the string to the control programs (sometimes the control program will make the connection first, sometimes it'll happen after we start to send)
            send_strings.send(TestMessage1).await.unwrap();
        }
    }, 0);

    // Program 2 receives the message and sends it to the test program
    scene.add_subprogram(program_2, move |input: InputStream<String>, context| {
        async move {
            let mut test_program = context.send(test_program).unwrap();

            let mut input = input;
            while let Some(input) = input.next().await {
                test_program.send(TestResult(input)).await.unwrap();
            }
        }
    }, 0);

    // Connect a filtered input for TestMessage2 to program_2, then add a source filter for TestMessage1 so there are two filters in effect
    scene.connect_programs((), StreamTarget::Filtered(test_string_filter, program_2), StreamId::with_message_type::<TestMessage2>()).unwrap();
    scene.connect_programs((), program_2, StreamId::with_message_type::<String>()).unwrap();
    scene.connect_programs(StreamSource::Filtered(test_message_filter), (), StreamId::with_message_type::<TestMessage1>()).unwrap();

    // Check that we receive the test message
    TestBuilder::new()
        .expect_message(|_msg: TestResult| { Ok(()) })
        .run_in_scene(&scene, test_program);
}

#[test]
pub fn connect_default_using_chained_filter_later_1() {
    // Scene with two programs that we'll connect together
    let scene           = Scene::default();
    let test_program    = SubProgramId::called("test_program");
    let program_1       = SubProgramId::called("program_1");
    let program_2       = SubProgramId::called("program_2");

    // TestMessage can be filtered into a string, but we don't set the filter up
    #[derive(Debug, Serialize, Deserialize)]
    struct TestMessage1;
    impl SceneMessage for TestMessage1 { }

    #[derive(Debug, Serialize, Deserialize)]
    struct TestMessage2;
    impl SceneMessage for TestMessage2 { }

    let test_message_filter = FilterHandle::for_filter(|messages| messages.map(|_: TestMessage1| TestMessage2));
    let test_string_filter = FilterHandle::for_filter(|messages| messages.map(|_: TestMessage2| "Test".to_string()));

    // Program_1 sends a TestMessage
    scene.add_subprogram(program_1, |_: InputStream<()>, context| {
        async move {
            // Create a stream to TestMessages to the default target (will be created disconnected)
            let mut send_strings = context.send(()).unwrap();

            // Add the target filter later on (this should be able to connect the source filter version of this stream)
            context.send_message(SceneControl::connect((), StreamTarget::Filtered(test_string_filter, program_2), StreamId::with_message_type::<TestMessage2>())).await.unwrap();

            // Send the string to the control programs (sometimes the control program will make the connection first, sometimes it'll happen after we start to send)
            send_strings.send(TestMessage1).await.unwrap();
        }
    }, 0);

    // Program 2 receives the message and sends it to the test program
    scene.add_subprogram(program_2, move |input: InputStream<String>, context| {
        async move {
            let mut test_program = context.send(test_program).unwrap();

            let mut input = input;
            while let Some(input) = input.next().await {
                test_program.send(TestResult(input)).await.unwrap();
            }
        }
    }, 0);

    // Connect a filtered input for TestMessage2 to program_2, then add a source filter for TestMessage1
    scene.connect_programs((), program_2, StreamId::with_message_type::<String>()).unwrap();
    scene.connect_programs(StreamSource::Filtered(test_message_filter), (), StreamId::with_message_type::<TestMessage1>()).unwrap();

    // Check that we receive the test message
    TestBuilder::new()
        .expect_message(|_msg: TestResult| { Ok(()) })
        .run_in_scene(&scene, test_program);
}

#[test]
pub fn connect_default_using_chained_filter_later_2() {
    // Scene with two programs that we'll connect together
    let scene           = Scene::default();
    let test_program    = SubProgramId::called("test_program");
    let program_1       = SubProgramId::called("program_1");
    let program_2       = SubProgramId::called("program_2");

    // TestMessage can be filtered into a string, but we don't set the filter up
    #[derive(Debug, Serialize, Deserialize)]
    struct TestMessage1;
    impl SceneMessage for TestMessage1 { }

    #[derive(Debug, Serialize, Deserialize)]
    struct TestMessage2;
    impl SceneMessage for TestMessage2 { }

    let test_message_filter = FilterHandle::for_filter(|messages| messages.map(|_: TestMessage1| TestMessage2));
    let test_string_filter = FilterHandle::for_filter(|messages| messages.map(|_: TestMessage2| "Test".to_string()));

    // Program_1 sends a TestMessage
    scene.add_subprogram(program_1, |_: InputStream<()>, context| {
        async move {
            // Create a stream to TestMessages to the default target (will be created disconnected)
            let mut send_strings = context.send(()).unwrap();

            // Add the source filter later on (which should connect via the target filter we added earlier)
            context.send_message(SceneControl::connect(StreamSource::Filtered(test_message_filter), (), StreamId::with_message_type::<TestMessage1>())).await.unwrap();

            // Send the string to the control programs (sometimes the control program will make the connection first, sometimes it'll happen after we start to send)
            send_strings.send(TestMessage1).await.unwrap();
        }
    }, 0);

    // Program 2 receives the message and sends it to the test program
    scene.add_subprogram(program_2, move |input: InputStream<String>, context| {
        async move {
            let mut test_program = context.send(test_program).unwrap();

            let mut input = input;
            while let Some(input) = input.next().await {
                test_program.send(TestResult(input)).await.unwrap();
            }
        }
    }, 0);

    // Connect a filtered input for TestMessage2 to program_2, then add a source filter for TestMessage1
    scene.connect_programs((), StreamTarget::Filtered(test_string_filter, program_2), StreamId::with_message_type::<TestMessage2>()).unwrap();
    scene.connect_programs((), program_2, StreamId::with_message_type::<String>()).unwrap();

    // Check that we receive the test message
    TestBuilder::new()
        .expect_message(|_msg: TestResult| { Ok(()) })
        .run_in_scene(&scene, test_program);
}

#[test]
pub fn connect_default_using_chained_filter_later_3() {
    // Scene with two programs that we'll connect together
    let scene           = Scene::default();
    let test_program    = SubProgramId::called("test_program");
    let program_1       = SubProgramId::called("program_1");
    let program_2       = SubProgramId::called("program_2");

    // TestMessage can be filtered into a string, but we don't set the filter up
    #[derive(Debug, Serialize, Deserialize)]
    struct TestMessage1;
    impl SceneMessage for TestMessage1 { }

    #[derive(Debug, Serialize, Deserialize)]
    struct TestMessage2;
    impl SceneMessage for TestMessage2 { }

    let test_message_filter = FilterHandle::for_filter(|messages| messages.map(|_: TestMessage1| TestMessage2));
    let test_string_filter = FilterHandle::for_filter(|messages| messages.map(|_: TestMessage2| "Test".to_string()));

    // Program_1 sends a TestMessage
    scene.add_subprogram(program_1, |_: InputStream<()>, context| {
        async move {
            // Create a stream to TestMessages to the default target (will be created disconnected)
            let mut send_strings = context.send(()).unwrap();

            // Add the source and target filters as separate steps
            context.send_message(SceneControl::connect(StreamSource::Filtered(test_message_filter), (), StreamId::with_message_type::<TestMessage1>())).await.unwrap();
            context.send_message(SceneControl::connect((), StreamTarget::Filtered(test_string_filter, program_2), StreamId::with_message_type::<TestMessage2>())).await.unwrap();

            // Send the string to the control programs (sometimes the control program will make the connection first, sometimes it'll happen after we start to send)
            send_strings.send(TestMessage1).await.unwrap();
        }
    }, 0);

    // Program 2 receives the message and sends it to the test program
    scene.add_subprogram(program_2, move |input: InputStream<String>, context| {
        async move {
            let mut test_program = context.send(test_program).unwrap();

            let mut input = input;
            while let Some(input) = input.next().await {
                test_program.send(TestResult(input)).await.unwrap();
            }
        }
    }, 0);

    // Connect a filtered input for TestMessage2 to program_2, then add a source filter for TestMessage1
    scene.connect_programs((), program_2, StreamId::with_message_type::<String>()).unwrap();

    // Check that we receive the test message
    TestBuilder::new()
        .expect_message(|_msg: TestResult| { Ok(()) })
        .run_in_scene(&scene, test_program);
}

#[test]
pub fn connect_default_using_filter_added_later() {
    // Scene with two programs that we'll connect together
    let scene           = Scene::default();
    let test_program    = SubProgramId::called("test_program");
    let program_1       = SubProgramId::called("program_1");
    let program_2       = SubProgramId::called("program_2");

    // TestMessage2 can be filtered into a string, but we don't set the filter up until after the program has started
    #[derive(Debug, Serialize, Deserialize)]
    struct TestMessage2;
    impl SceneMessage for TestMessage2 { }

    let test_string_filter = FilterHandle::for_filter(|messages| messages.map(|_: TestMessage2| "Test".to_string()));

    // Program_1 sends a TestMessage
    scene.add_subprogram(program_1, |_: InputStream<()>, context| {
        async move {
            // Create a stream to TestMessages to the default target (will be created disconnected)
            let mut send_strings = context.send(()).unwrap();

            // Add a target filter for program 2 that converts TestMessage2 to a string
            context.send_message(SceneControl::connect((), StreamTarget::Filtered(test_string_filter, program_2), StreamId::with_message_type::<TestMessage2>())).await.unwrap();

            // Send the string to the control programs (sometimes the control program will make the connection first, sometimes it'll happen after we start to send)
            send_strings.send(TestMessage2).await.unwrap();
        }
    }, 0);

    // Program 2 receives the message and sends it to the test program
    scene.add_subprogram(program_2, move |input: InputStream<String>, context| {
        async move {
            let mut test_program = context.send(test_program).unwrap();

            let mut input = input;
            while let Some(input) = input.next().await {
                test_program.send(TestResult(input)).await.unwrap();
            }
        }
    }, 0);

    // Anything that can send a string can send it to program 2
    scene.connect_programs((), program_2, StreamId::with_message_type::<String>()).unwrap();

    // Check that we receive the test message
    TestBuilder::new()
        .expect_message(|_msg: TestResult| { Ok(()) })
        .run_in_scene(&scene, test_program);
}

#[test]
pub fn connect_default_using_source_filter_added_later_1() {
    // Scene with two programs that we'll connect together
    let scene           = Scene::default();
    let test_program    = SubProgramId::called("test_program");
    let program_1       = SubProgramId::called("program_1");
    let program_2       = SubProgramId::called("program_2");

    // TestMessage2 can be filtered into a string, but we don't set the filter up until after we've created the stream
    #[derive(Debug, Serialize, Deserialize)]
    struct TestMessage2;
    impl SceneMessage for TestMessage2 { }

    let test_string_filter = FilterHandle::for_filter(|messages| messages.map(|_: TestMessage2| "Test".to_string()));

    // Program_1 sends a TestMessage
    scene.add_subprogram(program_1, |_: InputStream<()>, context| {
        async move {
            // Create a stream to TestMessages to the default target (will be created disconnected)
            let mut send_strings = context.send(()).unwrap();

            // Add a source filter for converting TestMessage2 to strings (so when we send TestMessage2 it shuld be sent to whatever can handle strings)
            context.send_message(SceneControl::connect(StreamSource::Filtered(test_string_filter), (), StreamId::with_message_type::<TestMessage2>())).await.unwrap();

            // Send the string to the control programs (sometimes the control program will make the connection first, sometimes it'll happen after we start to send)
            send_strings.send(TestMessage2).await.unwrap();
        }
    }, 0);

    // Program 2 receives the message and sends it to the test program
    scene.add_subprogram(program_2, move |input: InputStream<String>, context| {
        async move {
            let mut test_program = context.send(test_program).unwrap();

            let mut input = input;
            while let Some(input) = input.next().await {
                test_program.send(TestResult(input)).await.unwrap();
            }
        }
    }, 0);

    // Program 2 can receive strings (program_1 will try to send a TestMessage2 but will also install a filter)
    scene.connect_programs((), program_2, StreamId::with_message_type::<String>()).unwrap();

    // Check that we receive the test message
    TestBuilder::new()
        .expect_message(|_msg: TestResult| { Ok(()) })
        .run_in_scene(&scene, test_program);
}

#[test]
pub fn connect_default_using_source_filter_added_later_2() {
    // Scene with two programs that we'll connect together
    let scene           = Scene::default();
    let test_program    = SubProgramId::called("test_program");
    let program_1       = SubProgramId::called("program_1");
    let program_2       = SubProgramId::called("program_2");

    // TestMessage2 can be filtered into a string. We set up the filter first, but don't create the connection that will use it until later
    #[derive(Debug, Serialize, Deserialize)]
    struct TestMessage2;
    impl SceneMessage for TestMessage2 { }

    let test_string_filter = FilterHandle::for_filter(|messages| messages.map(|_: TestMessage2| "Test".to_string()));

    // Program_1 sends a TestMessage
    scene.add_subprogram(program_1, |_: InputStream<()>, context| {
        async move {
            // Create a stream to TestMessages to the default target (will be created disconnected)
            let mut send_strings = context.send(()).unwrap();

            // Set up program 2 to receive strings after we've created the stream (we set up a filter earlier on)
            context.send_message(SceneControl::connect((), program_2, StreamId::with_message_type::<String>())).await.unwrap();

            // Send the string to the control programs (sometimes the control program will make the connection first, sometimes it'll happen after we start to send)
            send_strings.send(TestMessage2).await.unwrap();
        }
    }, 0);

    // Program 2 receives the message and sends it to the test program
    scene.add_subprogram(program_2, move |input: InputStream<String>, context| {
        async move {
            let mut test_program = context.send(test_program).unwrap();

            let mut input = input;
            while let Some(input) = input.next().await {
                test_program.send(TestResult(input)).await.unwrap();
            }
        }
    }, 0);

    // Install a source filter for converting TestMessage2 to strings
    scene.connect_programs(StreamSource::Filtered(test_string_filter), (), StreamId::with_message_type::<TestMessage2>()).unwrap();

    // Check that we receive the test message
    TestBuilder::new()
        .expect_message(|_msg: TestResult| { Ok(()) })
        .run_in_scene(&scene, test_program);
}

#[test]
pub fn connect_two_subprograms_using_source_filter() {
    // Scene with two programs that we'll connect together
    let scene           = Scene::default();
    let test_program    = SubProgramId::new();
    let program_1       = SubProgramId::new();
    let program_2       = SubProgramId::new();

    // TestMessage can be filtered into a string, but we don't set the filter up
    #[derive(Debug, Serialize, Deserialize)]
    struct TestMessage;
    impl SceneMessage for TestMessage { }

    let test_string_filter = FilterHandle::for_filter(|messages| messages.map(|_: TestMessage| "Test".to_string()));

    // Program_1 sends a TestMessage
    scene.add_subprogram(program_1, |_: InputStream<()>, context| {
        async move {
            // Create a stream to TestMessages to the default target (will be created disconnected)
            let mut send_strings = context.send(()).unwrap();

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
                test_program.send(TestResult(input)).await.unwrap();
            }
        }
    }, 0);

    // Connect program 1 and 2, but first set up a source filter so that TestMessages are translated
    scene.connect_programs(StreamSource::Filtered(test_string_filter), (), StreamId::with_message_type::<TestMessage>()).unwrap();
    scene.connect_programs(program_1, program_2, StreamId::with_message_type::<TestMessage>()).unwrap();

    // Check that we receive the test message
    TestBuilder::new()
        .expect_message(|_msg: TestResult| { Ok(()) })
        .run_in_scene(&scene, test_program);
}

#[test]
pub fn connect_two_subprograms_using_source_filter_later() {
    // Scene with two programs that we'll connect together
    let scene           = Scene::default();
    let test_program    = SubProgramId::new();
    let program_1       = SubProgramId::new();
    let program_2       = SubProgramId::new();

    // TestMessage can be filtered into a string, but we don't set the filter up
    #[derive(Debug, Serialize, Deserialize)]
    struct TestMessage;
    impl SceneMessage for TestMessage { }

    let test_string_filter = FilterHandle::for_filter(|messages| messages.map(|_: TestMessage| "Test".to_string()));

    // Program_1 sends a TestMessage
    scene.add_subprogram(program_1, |_: InputStream<()>, context| {
        async move {
            // Create a stream to TestMessages to the default target (will be created disconnected)
            let mut send_strings = context.send(()).unwrap();

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
                test_program.send(TestResult(input)).await.unwrap();
            }
        }
    }, 0);

    // Connect program 1 to program 2, then say that we can filter TestMessages
    scene.connect_programs(program_1, program_2, StreamId::with_message_type::<TestMessage>()).unwrap();
    scene.connect_programs(StreamSource::Filtered(test_string_filter), (), StreamId::with_message_type::<TestMessage>()).unwrap();

    // Check that we receive the test message
    TestBuilder::new()
        .expect_message(|_msg: TestResult| { Ok(()) })
        .run_in_scene(&scene, test_program);
}

#[test]
pub fn connect_two_subprograms_using_string_type_then_source_filter() {
    // Scene with two programs that we'll connect together
    let scene           = Scene::default();
    let test_program    = SubProgramId::new();
    let program_1       = SubProgramId::new();
    let program_2       = SubProgramId::new();

    // TestMessage can be filtered into a string, but we don't set the filter up
    #[derive(Debug, Serialize, Deserialize)]
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
                test_program.send(TestResult(input)).await.unwrap();
            }
        }
    }, 0);

    // Connect program 1 and 2 as strings, then set up a source filter so that string messages can be converted from TestMessages (so our message can be sent)
    // This is the same as in `connect_two_subprograms_using_source_filter_later` except we use the input type of the target to make the connection instead of the output type
    scene.connect_programs(program_1, program_2, StreamId::with_message_type::<String>()).unwrap();
    scene.connect_programs(StreamSource::Filtered(test_string_filter), (), StreamId::with_message_type::<TestMessage>()).unwrap();

    // Check that we receive the test message
    TestBuilder::new()
        .expect_message(|_msg: TestResult| { Ok(()) })
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
                test_program.send(TestResult(input)).await.unwrap();
            }
        }
    }, 0);

    // Check that we receive the test message
    TestBuilder::new()
        .expect_message(|_msg: TestResult| { Ok(()) })
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
                test_program.send(TestResult(input)).await.unwrap();
            }
        }
    }, 0);

    // Check that we receive the test message
    TestBuilder::new()
        .expect_message(|_msg: TestResult| { Ok(()) })
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
                test_program.send(TestResult(input)).await.unwrap();
            }
        }
    }, 0);

    // Check that we receive the test message
    TestBuilder::new()
        .expect_message(|_msg: TestResult| { Ok(()) })
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
                test_program.send(TestResult(input)).await.unwrap();
            }
        }
    }, 0);

    // Check that we receive the test message
    TestBuilder::new()
        .expect_message(|_msg: TestResult| { Ok(()) })
        .run_in_scene(&scene, test_program);
}

#[test]
pub fn connect_default_subprograms_before_launching() {
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
            // Connect to the default target for sending strings. It's configured as program_2 but that's not running yet
            let mut send_strings = context.send(()).unwrap();

            context.wait_for_idle(100).await;

            // Start the receiver program after we've started sending the strings
            context.send_message(SceneControl::start_program(program_2, move |input: InputStream<String>, context| {
                async move {
                    let mut test_program = context.send(test_program).unwrap();

                    let mut input = input;
                    while let Some(input) = input.next().await {
                        test_program.send(TestResult(input)).await.unwrap();
                    }
                }
            }, 0)).await.unwrap();

            send_strings.send("Test".to_string()).await.unwrap();
        }
    }, 0);

    // Check that we receive the test message
    TestBuilder::new()
        .expect_message(|_msg: TestResult| { Ok(()) })
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
    #[derive(Debug, Serialize, Deserialize)]
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
                test_program.send(TestResult(input)).await.unwrap();
            }
        }
    }, 0);

    // Check that we receive the test message
    TestBuilder::new()
        .expect_message(|_msg: TestResult| { Ok(()) })
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
    #[derive(Debug, Serialize, Deserialize)]
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
                test_program.send(TestResult(input)).await.unwrap();
            }
        }
    }, 0);

    // Check that we receive the test message
    TestBuilder::new()
        .expect_message(|_msg: TestResult| { Ok(()) })
        .run_in_scene(&scene, test_program);
}

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

// These seem to be failing because the test uses a filter target mechanism and you can't connect a specific stream to a generic filter target
// (Ie, source=Any, target=Filter does not add a filter for the source=specific, target=specific case). It's annoyingly complicated to fix this
// though the source filter is an option here.
//
// (We start the stream as disconnected in anticipation of a filter being added later on, but in a sense there already is a filter)

#[test]
fn connect_single_source_to_single_target() {
    let scene = Scene::default();

    let guest_subprogram_id     = SubProgramId::called("Guest subprogram");
    let sender_subprogram_id    = SubProgramId::called("Sender subprogram");
    let test_subprogram_id      = SubProgramId::called("Test subprogram");

    // Run a program that relays any messages it receives to the default output
    scene.add_subprogram(guest_subprogram_id, move |input_stream: InputStream<SimpleTestMessage>, context| async move {
        // Send responses to the defualt target for the scene
        let mut response = context.send::<SimpleResponseMessage>(()).unwrap();

        let mut input_stream = input_stream;
        while let Some(msg) = input_stream.next().await {
            println!("Received message: {:?}", msg);

            response.send(SimpleResponseMessage { value: msg.value }).await.unwrap();

            println!("Sent message");
        }
    }, 10);

    // Run another program to send messages to the first one
    scene.add_subprogram(sender_subprogram_id, move |_input: InputStream<()>, context| async move {
        let mut test_messages = context.send(guest_subprogram_id).unwrap();

        test_messages.send(SimpleTestMessage { value: "Hello".into() }).await.unwrap();
        test_messages.send(SimpleTestMessage { value: "Goodbyte".into() }).await.unwrap();
    }, 0);

    // Set the default output of just the program we created to the test program (but not the default for every message of this type)
    scene.connect_programs(guest_subprogram_id, test_subprogram_id, StreamId::with_message_type::<SimpleResponseMessage>()).unwrap();

    TestBuilder::new()
        .expect_message(|_: SimpleResponseMessage| { Ok(()) })
        .expect_message(|_: SimpleResponseMessage| { Ok(()) })
        .run_in_scene(&scene, test_subprogram_id);
}


#[test]
fn connect_single_source_to_single_target_before_creation() {
    let scene = Scene::default();

    let guest_subprogram_id     = SubProgramId::called("Guest subprogram");
    let sender_subprogram_id    = SubProgramId::called("Sender subprogram");
    let test_subprogram_id      = SubProgramId::called("Test subprogram");

    // Set the default output of just the program we created to the test program (but not the default for every message of this type)
    scene.connect_programs(guest_subprogram_id, test_subprogram_id, StreamId::with_message_type::<SimpleResponseMessage>()).unwrap();

    // Run a program that relays any messages it receives to the default output
    scene.add_subprogram(guest_subprogram_id, move |input_stream: InputStream<SimpleTestMessage>, context| async move {
        // Send responses to the defualt target for the scene
        let mut response = context.send::<SimpleResponseMessage>(()).unwrap();

        let mut input_stream = input_stream;
        while let Some(msg) = input_stream.next().await {
            println!("Received message: {:?}", msg);

            response.send(SimpleResponseMessage { value: msg.value }).await.unwrap();

            println!("Sent message");
        }
    }, 10);

    // Run another program to send messages to the first one
    scene.add_subprogram(sender_subprogram_id, move |_input: InputStream<()>, context| async move {
        let mut test_messages = context.send(guest_subprogram_id).unwrap();

        test_messages.send(SimpleTestMessage { value: "Hello".into() }).await.unwrap();
        test_messages.send(SimpleTestMessage { value: "Goodbyte".into() }).await.unwrap();
    }, 0);

    TestBuilder::new()
        .expect_message(|_: SimpleResponseMessage| { Ok(()) })
        .expect_message(|_: SimpleResponseMessage| { Ok(()) })
        .run_in_scene(&scene, test_subprogram_id);
}

// TODO: both these tests set up the connection before the connection is made, we also need to test making the connection later on
// TODO: `connect_single_source_to_single_target` but with source and target filters

