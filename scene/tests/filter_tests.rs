//!
//! Sometimes, it's necessary to connect two programs where one produces a different kind
//! of output to the input. Filters provide a general way to map one stream of messages
//! to another.
//!
//! This isn't needed for the basic scene functionality: the same thing can be done by
//! creating a subprogram to do the conversion, but having filters built in like this makes
//! it much easier to create adapters between programs.
//!

use flo_scene::*;
use flo_scene::programs::*;

use futures::prelude::*;
use futures::future::{select};
use futures::executor;
use futures_timer::*;

use std::time::{Duration};
use std::sync::*;
use std::sync::atomic::{AtomicUsize, Ordering};

#[test]
fn write_to_filter_target() {
    // List of messages that were received by the subprogram
    let recv_messages = Arc::new(Mutex::new(vec![]));

    // Create a scene with just this subprogram in it
    let scene           = Scene::empty();
    let sent_messages   = recv_messages.clone();

    // Create a filter that converts numbers to strings
    let usize_to_string = FilterHandle::for_filter(|number_stream: InputStream<usize>| number_stream.map(|num| num.to_string()));

    // Add a program that receives some strings and writes them to recv_messages
    let string_program = SubProgramId::new();
    scene.add_subprogram(
        string_program,
        move |mut strings: InputStream<String>, _| async move {
            let next_string = strings.next().await.unwrap();
            sent_messages.lock().unwrap().push(next_string);
            let next_string = strings.next().await.unwrap();
            sent_messages.lock().unwrap().push(next_string);
            let next_string = strings.next().await.unwrap();
            sent_messages.lock().unwrap().push(next_string);
            let next_string = strings.next().await.unwrap();
            sent_messages.lock().unwrap().push(next_string);
        },
        0,
    );

    // Add another program that outputs some numbers to the first program
    let number_program = SubProgramId::new();
    scene.add_subprogram(
        number_program, 
        move |_: InputStream<()>, context| async move {
            let mut filtered_output = context.send::<usize>(StreamTarget::Filtered(usize_to_string, string_program)).unwrap();

            filtered_output.send(1).await.unwrap();
            filtered_output.send(2).await.unwrap();
            filtered_output.send(3).await.unwrap();
            filtered_output.send(4).await.unwrap();
        }, 
        0);

    // Run the scene
    let mut has_finished = false;
    executor::block_on(select(async {
        scene.run_scene().await;
        has_finished = true;
    }.boxed(), Delay::new(Duration::from_millis(5000))));

    // Received output should match the numbers
    let recv_messages = (*recv_messages.lock().unwrap()).clone();
    assert!(recv_messages == vec![1.to_string(), 2.to_string(), 3.to_string(), 4.to_string()], "Test program did not send correct numbers (sent {:?})", recv_messages);
    assert!(has_finished, "Scene did not finish when the programs terminated");
}

#[test]
fn write_to_conversion_filter() {
    // List of messages that were received by the subprogram
    let recv_messages = Arc::new(Mutex::new(vec![]));

    // Create a scene with just this subprogram in it
    let scene           = Scene::empty();
    let sent_messages   = recv_messages.clone();

    // Create a filter that converts u32s to u64s
    let u32_to_u64 = FilterHandle::conversion_filter::<u32, u64>();

    // Add a program that receives some u64s and writes them to recv_messages as strings
    let string_program = SubProgramId::new();
    scene.add_subprogram(
        string_program,
        move |mut numbers: InputStream<u64>, _| async move {
            let next_number = numbers.next().await.unwrap();
            sent_messages.lock().unwrap().push(next_number.to_string());
            let next_number = numbers.next().await.unwrap();
            sent_messages.lock().unwrap().push(next_number.to_string());
            let next_number = numbers.next().await.unwrap();
            sent_messages.lock().unwrap().push(next_number.to_string());
            let next_number = numbers.next().await.unwrap();
            sent_messages.lock().unwrap().push(next_number.to_string());
        },
        0,
    );

    // Add another program that outputs some u32s to the first program via the conversion filter
    let number_program = SubProgramId::new();
    scene.add_subprogram(
        number_program, 
        move |_: InputStream<()>, context| async move {
            let mut filtered_output = context.send::<u32>(StreamTarget::Filtered(u32_to_u64, string_program)).unwrap();

            filtered_output.send(1u32).await.unwrap();
            filtered_output.send(2u32).await.unwrap();
            filtered_output.send(3u32).await.unwrap();
            filtered_output.send(4u32).await.unwrap();
        }, 
        0);

    // Run the scene
    let mut has_finished = false;
    executor::block_on(select(async {
        scene.run_scene().await;
        has_finished = true;
    }.boxed(), Delay::new(Duration::from_millis(5000))));

    // Received output should match the numbers
    let recv_messages = (*recv_messages.lock().unwrap()).clone();
    assert!(recv_messages == vec![1.to_string(), 2.to_string(), 3.to_string(), 4.to_string()], "Test program did not send correct numbers (sent {:?})", recv_messages);
    assert!(has_finished, "Scene did not finish when the programs terminated");
}

#[test]
fn apply_filter_to_direct_connection() {
    // List of messages that were received by the subprogram
    let recv_messages = Arc::new(Mutex::new(vec![]));

    // Create a scene with just this subprogram in it
    let scene           = Scene::empty();
    let sent_messages   = recv_messages.clone();

    // Create a filter that converts numbers to strings
    let usize_to_string = FilterHandle::for_filter(|number_stream: InputStream<usize>| number_stream.map(|num| num.to_string()));

    // Add a program that receives some strings and writes them to recv_messages
    let string_program = SubProgramId::new();
    scene.add_subprogram(
        string_program,
        move |mut strings: InputStream<String>, _| async move {
            let next_string = strings.next().await.unwrap();
            sent_messages.lock().unwrap().push(next_string);
            let next_string = strings.next().await.unwrap();
            sent_messages.lock().unwrap().push(next_string);
            let next_string = strings.next().await.unwrap();
            sent_messages.lock().unwrap().push(next_string);
            let next_string = strings.next().await.unwrap();
            sent_messages.lock().unwrap().push(next_string);
        },
        0,
    );

    // Create a filter targeting our new program that maps from usize to string
    scene.connect_programs((), StreamTarget::Filtered(usize_to_string, string_program), StreamId::with_message_type::<usize>().for_target(string_program)).unwrap();

    // Add another program that outputs some numbers as usize values to the first program
    // The connection defined above will apply the filter, even though the first program only accepts strings as an input
    let number_program = SubProgramId::new();
    scene.add_subprogram(
        number_program, 
        move |_: InputStream<()>, context| async move {
            let mut filtered_output = context.send::<usize>(string_program).unwrap();

            filtered_output.send(1).await.unwrap();
            filtered_output.send(2).await.unwrap();
            filtered_output.send(3).await.unwrap();
            filtered_output.send(4).await.unwrap();
        }, 
        0);

    // Run the scene
    let mut has_finished = false;
    executor::block_on(select(async {
        scene.run_scene().await;
        has_finished = true;
    }.boxed(), Delay::new(Duration::from_millis(5000))));

    // Received output should match the numbers
    let recv_messages = (*recv_messages.lock().unwrap()).clone();
    assert!(recv_messages == vec![1.to_string(), 2.to_string(), 3.to_string(), 4.to_string()], "Test program did not send correct numbers (sent {:?})", recv_messages);
    assert!(has_finished, "Scene did not finish when the programs terminated");
}

#[test]
fn connect_all_to_filter_target() {
    // List of messages that were received by the subprogram
    let recv_messages = Arc::new(Mutex::new(vec![]));

    // Create a scene with just this subprogram in it
    let scene           = Scene::empty();
    let sent_messages   = recv_messages.clone();

    // Create a filter that converts numbers to strings
    let usize_to_string = FilterHandle::for_filter(|number_stream: InputStream<usize>| number_stream.map(|num| num.to_string()));

    // Add a program that receives some strings and writes them to recv_messages
    let string_program = SubProgramId::new();
    scene.add_subprogram(
        string_program,
        move |mut strings: InputStream<String>, _| async move {
            let next_string = strings.next().await.unwrap();
            sent_messages.lock().unwrap().push(next_string);
            let next_string = strings.next().await.unwrap();
            sent_messages.lock().unwrap().push(next_string);
            let next_string = strings.next().await.unwrap();
            sent_messages.lock().unwrap().push(next_string);
            let next_string = strings.next().await.unwrap();
            sent_messages.lock().unwrap().push(next_string);
        },
        0,
    );

    // Add another program that outputs some numbers to the first program
    let number_program = SubProgramId::new();
    scene.add_subprogram(
        number_program, 
        move |_: InputStream<()>, context| async move {
            let mut filtered_output = context.send::<usize>(StreamTarget::Any).unwrap();

            filtered_output.send(1).await.unwrap();
            filtered_output.send(2).await.unwrap();
            filtered_output.send(3).await.unwrap();
            filtered_output.send(4).await.unwrap();
        }, 
        0);

    scene.connect_programs((), StreamTarget::Filtered(usize_to_string, string_program), StreamId::with_message_type::<usize>()).unwrap();

    // Run the scene
    let mut has_finished = false;
    executor::block_on(select(async {
        scene.run_scene().await;
        has_finished = true;
    }.boxed(), Delay::new(Duration::from_millis(5000))));

    // Received output should match the numbers
    let recv_messages = (*recv_messages.lock().unwrap()).clone();
    assert!(recv_messages == vec![1.to_string(), 2.to_string(), 3.to_string(), 4.to_string()], "Test program did not send correct numbers (sent {:?})", recv_messages);
    assert!(has_finished, "Scene did not finish when the programs terminated");
}

#[test]
fn direct_connection_via_general_filter() {
    // List of messages that were received by the subprogram
    let recv_messages = Arc::new(Mutex::new(vec![]));

    // Create a scene with just this subprogram in it
    let scene           = Scene::empty();
    let sent_messages   = recv_messages.clone();

    // Create a filter that converts numbers to strings
    let usize_to_string = FilterHandle::for_filter(|number_stream: InputStream<usize>| number_stream.map(|num| num.to_string()));

    // Add a program that receives some strings and writes them to recv_messages
    let string_program = SubProgramId::new();
    scene.add_subprogram(
        string_program,
        move |mut strings: InputStream<String>, _| async move {
            let next_string = strings.next().await.unwrap();
            sent_messages.lock().unwrap().push(next_string);
            let next_string = strings.next().await.unwrap();
            sent_messages.lock().unwrap().push(next_string);
            let next_string = strings.next().await.unwrap();
            sent_messages.lock().unwrap().push(next_string);
            let next_string = strings.next().await.unwrap();
            sent_messages.lock().unwrap().push(next_string);
        },
        0,
    );

    // Add another program that outputs some numbers to the first program
    // This directly connections to the first program, but the filter is applied to all connections of that message type
    let number_program = SubProgramId::new();
    scene.add_subprogram(
        number_program, 
        move |_: InputStream<()>, context| async move {
            let mut filtered_output = context.send::<usize>(string_program).unwrap();

            filtered_output.send(1).await.unwrap();
            filtered_output.send(2).await.unwrap();
            filtered_output.send(3).await.unwrap();
            filtered_output.send(4).await.unwrap();
        }, 
        0);

    // This connects all 'usize' streams to string_program via a filter
    scene.connect_programs((), StreamTarget::Filtered(usize_to_string, string_program), StreamId::with_message_type::<usize>()).unwrap();

    // Run the scene
    let mut has_finished = false;
    executor::block_on(select(async {
        scene.run_scene().await;
        has_finished = true;
    }.boxed(), Delay::new(Duration::from_millis(5000))));

    // Received output should match the numbers
    let recv_messages = (*recv_messages.lock().unwrap()).clone();
    assert!(recv_messages == vec![1.to_string(), 2.to_string(), 3.to_string(), 4.to_string()], "Test program did not send correct numbers (sent {:?})", recv_messages);
    assert!(has_finished, "Scene did not finish when the programs terminated");
}

#[test]
fn connect_all_both_filtered_and_unfiltered() {
    // List of messages that were received by the subprogram
    let recv_messages = Arc::new(Mutex::new(vec![]));

    // Create a scene with just this subprogram in it
    let scene           = Scene::empty();
    let sent_messages   = recv_messages.clone();

    // Create a filter that converts numbers to strings
    let usize_to_string = FilterHandle::for_filter(|number_stream: InputStream<usize>| number_stream.map(|num| num.to_string()));

    // Add a program that receives some strings and writes them to recv_messages
    let string_program = SubProgramId::new();
    scene.add_subprogram(
        string_program,
        move |mut strings: InputStream<String>, _| async move {
            for _ in 0..8 {
                let next_string = strings.next().await.unwrap();
                sent_messages.lock().unwrap().push(next_string);
            }
        },
        3,
    );

    // Add another program that outputs some numbers to the first program
    let string_generator_program = SubProgramId::new();
    scene.add_subprogram(
        string_generator_program, 
        move |_: InputStream<()>, context| async move {
            let mut filtered_output = context.send::<String>(StreamTarget::Any).unwrap();

            filtered_output.send(5.to_string()).await.unwrap();
            filtered_output.send(6.to_string()).await.unwrap();
            filtered_output.send(7.to_string()).await.unwrap();
            filtered_output.send(8.to_string()).await.unwrap();
        }, 
        0);

    let number_program = SubProgramId::new();
    scene.add_subprogram(
        number_program, 
        move |_: InputStream<()>, context| async move {
            let mut filtered_output = context.send::<usize>(StreamTarget::Any).unwrap();

            filtered_output.send(1).await.unwrap();
            filtered_output.send(2).await.unwrap();
            filtered_output.send(3).await.unwrap();
            filtered_output.send(4).await.unwrap();
        }, 
        0);

    scene.connect_programs((), string_program, StreamId::with_message_type::<String>()).unwrap();
    scene.connect_programs((), StreamTarget::Filtered(usize_to_string, string_program), StreamId::with_message_type::<usize>()).unwrap();

    // Run the scene
    let mut has_finished = false;
    executor::block_on(select(async {
        scene.run_scene().await;
        has_finished = true;
    }.boxed(), Delay::new(Duration::from_millis(5000))));

    // Received output should match the numbers
    let recv_messages = (*recv_messages.lock().unwrap()).clone();
    let one_to_four     = recv_messages.iter().filter(|msg| *msg == "1" || *msg == "2" || *msg == "3" || *msg == "4").cloned().collect::<Vec<_>>();
    let five_to_eight   = recv_messages.iter().filter(|msg| *msg == "5" || *msg == "6" || *msg == "7" || *msg == "8").cloned().collect::<Vec<_>>();

    // The messages can appear interleaved from the two programs but should otherwise be in order
    assert!(recv_messages.len() == 8, "Wrong number of messages received: {:?}", recv_messages);
    assert!(one_to_four == vec![1.to_string(), 2.to_string(), 3.to_string(), 4.to_string()], "Test program did not send correct numbers (sent {:?})", recv_messages);
    assert!(five_to_eight == vec![5.to_string(), 6.to_string(), 7.to_string(), 8.to_string()], "Test program did not send correct numbers (sent {:?})", recv_messages);
    assert!(has_finished, "Scene did not finish when the programs terminated");
}

#[test]
fn disconnect_filter_target() {
    // List of messages that were received by the subprogram
    let recv_messages = Arc::new(Mutex::new(vec![]));

    // CountDisconnects is a struct that gets dropped when the filter stream is closed: it should get called twice here (once when we reconnect, once when the programs end)
    static NUM_DISCONNECTS: AtomicUsize = AtomicUsize::new(0);

    struct CountDisconnents {

    }

    impl CountDisconnents {
        fn convert_to_string(&self, num: usize) -> String {
            num.to_string()
        }
    }

    impl Drop for CountDisconnents {
        fn drop(&mut self) {
            println!("Disconnect");
            NUM_DISCONNECTS.fetch_add(1, Ordering::Relaxed);
        }
    }

    NUM_DISCONNECTS.store(0, Ordering::Relaxed);

    // Create a scene with just this subprogram in it
    let scene           = Arc::new(Scene::empty());
    let sent_messages   = recv_messages.clone();

    // Create a filter that converts numbers to strings
    println!("Register filter");
    let usize_to_string = FilterHandle::for_filter(|number_stream: InputStream<usize>| {
        println!("Connecting");
        let count_disconnects = CountDisconnents {};

        number_stream.map(move |num| count_disconnects.convert_to_string(num))
    });

    // Add a program that receives some strings and writes them to recv_messages
    println!("Create string program");
    let string_program = SubProgramId::new();
    scene.add_subprogram(
        string_program,
        move |mut strings: InputStream<String>, _| async move {
            let next_string = strings.next().await.unwrap();
            sent_messages.lock().unwrap().push(next_string);
            let next_string = strings.next().await.unwrap();
            sent_messages.lock().unwrap().push(next_string);
            let next_string = strings.next().await.unwrap();
            sent_messages.lock().unwrap().push(next_string);
            let next_string = strings.next().await.unwrap();
            sent_messages.lock().unwrap().push(next_string);
        },
        0,
    );

    // Add another program that outputs some numbers to the first program
    println!("Create number program");
    let number_program  = SubProgramId::new();
    let scene2          = scene.clone();
    scene.add_subprogram(
        number_program, 
        move |_: InputStream<()>, context| async move {
            println!("Create initial stream...");
            let mut filtered_output = context.send::<usize>(StreamTarget::Any).unwrap();
            println!("  ... created");

            // Send first two messages
            filtered_output.send(1).await.unwrap();
            filtered_output.send(2).await.unwrap();

            // Disconnect the stream
            println!("Disconnecting initial stream...");
            scene2.connect_programs(number_program, StreamTarget::None, StreamId::with_message_type::<usize>()).unwrap();
            println!("   ... disconnected");

            // Send another two messages into oblivion
            filtered_output.send(3).await.unwrap();
            filtered_output.send(4).await.unwrap();

            // Reconnect the two programs
            println!("Reconnecting filter stream...");
            scene2.connect_programs(number_program, StreamTarget::Filtered(usize_to_string, string_program), StreamId::with_message_type::<usize>()).unwrap();
            println!("   ... reconnected");

            // Final two messages
            filtered_output.send(5).await.unwrap();
            filtered_output.send(6).await.unwrap();

            // Disconnect them again
            println!("Disconnecting again...");
            scene2.connect_programs(number_program, StreamTarget::None, StreamId::with_message_type::<usize>()).unwrap();
            println!("   ... disconnected");
        }, 
        0);

    // Start the programs connected
    println!("Connect programs");
    scene.connect_programs(number_program, StreamTarget::Filtered(usize_to_string, string_program), StreamId::with_message_type::<usize>()).unwrap();

    // Run the scene
    println!("Start scene");
    let mut has_finished = false;
    executor::block_on(select(async {
        scene.run_scene().await;
        has_finished = true;
    }.boxed(), Delay::new(Duration::from_millis(5000))));

    println!("Scene finished");

    // Received output should match the numbers
    let recv_messages   = (*recv_messages.lock().unwrap()).clone();
    let num_disconnects = NUM_DISCONNECTS.load(Ordering::Relaxed);

    assert!(recv_messages == vec![1.to_string(), 2.to_string(), 5.to_string(), 6.to_string()], "Test program did not send correct numbers (sent {:?})", recv_messages);
    assert!(num_disconnects == 2, "Filtered stream was not dropped the expected number of times ({} != 2)", num_disconnects);
    assert!(has_finished, "Scene did not finish when the programs terminated");
}

#[test]
fn filter_with_send_and_target_filter() {
    // Create a standard scene
    let scene = Scene::default();

    // The test program can pick up on a filter defined before it starts
    #[derive(Debug, PartialEq)]
    enum Message1 { Msg(String) }
    #[derive(Debug, PartialEq)]
    enum Message2 { Msg(String) }

    impl SceneMessage for Message1 { }
    impl SceneMessage for Message2 { }

    let str_to_msg1  = FilterHandle::for_filter(|string_value: InputStream<&'static str>| string_value.map(|val| Message1::Msg(val.to_string())));
    let msg1_to_msg2 = FilterHandle::for_filter(|msg1: InputStream<Message1>| { msg1.map(|msg| match msg { Message1::Msg(val) => Message2::Msg(val) })});

    // The IDs of the two programs involved
    let message1_sender_program     = SubProgramId::new();
    let message2_receiver_program   = SubProgramId::new();
    let test_program                = SubProgramId::new();

    // Create a program that generates 'message1' as an output via the message2_receiver program
    scene.add_subprogram(message1_sender_program, |_: InputStream<()>, context| async move {
        let mut sender = context.send(StreamTarget::Filtered(str_to_msg1, message2_receiver_program)).unwrap();

        println!("Sending 1...");
        sender.send("Hello").await.unwrap();
        println!("Sending 2...");
        sender.send("Goodbyte").await.unwrap();
        println!("Done");
    }, 0);

    // message2_receiver_program relays all of the messages to the test program
    scene.add_subprogram(message2_receiver_program, |mut input: InputStream<Message2>, context| async move {
        // We need an intermediate prorgam because the test program has its own set of filters. All this does is relay its message to the next program along.
        let mut test_program = context.send(()).unwrap();

        while let Some(input) = input.next().await { 
            test_program.send(input).await.unwrap(); 
        }
    }, 0);

    // message2_receiver_program accepts Message1 from any source as an input via a filter
    scene.connect_programs((), StreamTarget::Filtered(msg1_to_msg2, message2_receiver_program), StreamId::with_message_type::<Message1>()).unwrap();

    // Test program receives message2
    TestBuilder::new()
        .expect_message(|msg2: Message2| if msg2 != Message2::Msg("Hello".to_string()) { Err(format!("Expected 'Hello'")) } else { Ok(()) })
        .expect_message(|msg2: Message2| if msg2 != Message2::Msg("Goodbyte".to_string()) { Err(format!("Expected 'Goodbyte'")) } else { Ok(()) })
        .run_in_scene(&scene, test_program);
}

#[test]
fn filter_at_target() {
    // Create a standard scene
    let scene = Scene::default();

    // The test program can pick up on a filter defined before it starts
    #[derive(Debug, PartialEq)]
    enum Message1 { Msg(String) }
    #[derive(Debug, PartialEq)]
    enum Message2 { Msg(String) }

    impl SceneMessage for Message1 { }
    impl SceneMessage for Message2 { }

    let msg1_to_msg2 = FilterHandle::for_filter(|msg1: InputStream<Message1>| { msg1.map(|msg| match msg { Message1::Msg(val) => Message2::Msg(val) })});

    // The IDs of the two programs involved
    let message1_sender_program     = SubProgramId::new();
    let message2_receiver_program   = SubProgramId::new();
    let test_program                = SubProgramId::new();

    // Create a program that generates 'message1' as an output
    scene.add_subprogram(message1_sender_program, |_: InputStream<()>, context| async move {
        let mut sender = context.send(()).unwrap();

        println!("Sending 1...");
        sender.send(Message1::Msg("Hello".to_string())).await.unwrap();
        println!("Sending 2...");
        sender.send(Message1::Msg("Goodbyte".to_string())).await.unwrap();
        println!("Done");
    }, 0);

    // message2_receiver_program relays all of the messages to the test program
    scene.add_subprogram(message2_receiver_program, |mut input: InputStream<Message2>, context| async move {
        // We need an intermediate prorgam because the test program has its own set of filters. All this does is relay its message to the next program along.
        let mut test_program = context.send(()).unwrap();

        while let Some(input) = input.next().await { 
            test_program.send(input).await.unwrap(); 
        }
    }, 0);

    // message2_receiver_program accepts Message1 from any source as an input via a filter
    scene.connect_programs((), StreamTarget::Filtered(msg1_to_msg2, message2_receiver_program), StreamId::with_message_type::<Message1>()).unwrap();

    // Test program receives message2
    TestBuilder::new()
        .expect_message(|msg2: Message2| if msg2 != Message2::Msg("Hello".to_string()) { Err(format!("Expected 'Hello'")) } else { Ok(()) })
        .expect_message(|msg2: Message2| if msg2 != Message2::Msg("Goodbyte".to_string()) { Err(format!("Expected 'Goodbyte'")) } else { Ok(()) })
        .run_in_scene(&scene, test_program);
}

#[test]
fn filter_target_using_source_filter() {
    // Create a standard scene
    let scene = Scene::default();

    // The test program can pick up on a filter defined before it starts
    #[derive(Debug, PartialEq)]
    enum Message1 { Msg(String) }
    #[derive(Debug, PartialEq)]
    enum Message2 { Msg(String) }

    impl SceneMessage for Message1 { }
    impl SceneMessage for Message2 { }

    let msg1_to_msg2 = FilterHandle::for_filter(|msg1: InputStream<Message1>| { msg1.map(|msg| match msg { Message1::Msg(val) => Message2::Msg(val) })});

    // The IDs of the two programs involved
    let message1_sender_program     = SubProgramId::new();
    let message2_receiver_program   = SubProgramId::new();
    let test_program                = SubProgramId::new();

    // Create a program that generates 'message1' as an output
    scene.add_subprogram(message1_sender_program, |_: InputStream<()>, context| async move {
        let mut sender = context.send(()).unwrap();

        println!("Sending 1...");
        sender.send(Message1::Msg("Hello".to_string())).await.unwrap();
        println!("Sending 2...");
        sender.send(Message1::Msg("Goodbyte".to_string())).await.unwrap();
        println!("Done");
    }, 0);

    // message2_receiver_program relays all of the messages to the test program
    scene.add_subprogram(message2_receiver_program, |mut input: InputStream<Message2>, context| async move {
        // We need an intermediate prorgam because the test program has its own set of filters. All this does is relay its message to the next program along.
        let mut test_program = context.send(()).unwrap();

        while let Some(input) = input.next().await { 
            test_program.send(input).await.unwrap(); 
        }
    }, 0);

    // message2_receiver_program accepts Message1 from any source as an input via a filter
    scene.connect_programs(StreamSource::Filtered(msg1_to_msg2), message2_receiver_program, StreamId::with_message_type::<Message1>()).unwrap();

    // Test program receives message2
    TestBuilder::new()
        .expect_message(|msg2: Message2| if msg2 != Message2::Msg("Hello".to_string()) { Err(format!("Expected 'Hello'")) } else { Ok(()) })
        .expect_message(|msg2: Message2| if msg2 != Message2::Msg("Goodbyte".to_string()) { Err(format!("Expected 'Goodbyte'")) } else { Ok(()) })
        .run_in_scene(&scene, test_program);
}

#[test]
fn filter_at_source_with_specific_target() {
    // Create a standard scene
    let scene = Scene::default();

    // Filters can also be specified as a source for a stream. These change the input for all targets that target that source
    #[derive(Debug)]
    enum Message1 { Msg(String) }
    #[derive(Debug, PartialEq)]
    enum Message2 { Msg(String) }

    impl SceneMessage for Message1 { }
    impl SceneMessage for Message2 { }

    let msg1_to_msg2 = FilterHandle::for_filter(|msg1: InputStream<Message1>| { msg1.map(|msg| match msg { Message1::Msg(val) => Message2::Msg(val) })});

    // For any stream that tries to connect to a program with an input stream of type 'Message2' using 'Message1', automatically use the filter we just defined
    // Target filters will take priority
    scene.connect_programs(msg1_to_msg2, (), StreamId::with_message_type::<Message1>()).unwrap();

    // The IDs of the two programs involved
    let message1_sender_program     = SubProgramId::new();
    let message2_receiver_program   = SubProgramId::new();
    let test_program                = SubProgramId::new();

    // Create a program that generates 'message1' as an output
    scene.add_subprogram(message1_sender_program, |_: InputStream<()>, context| async move {
        let mut sender = context.send(()).unwrap();

        println!("Sending 1...");
        sender.send(Message1::Msg("Hello".to_string())).await.unwrap();
        println!("Sending 2...");
        sender.send(Message1::Msg("Goodbyte".to_string())).await.unwrap();
        println!("Done");
    }, 0);

    // message2_receiver_program should receive all messages of the target type. Temp subprogram here is replaced by the test later on
    scene.add_subprogram(message2_receiver_program, |mut input: InputStream<Message2>, context| async move {
        // We need an intermediate prorgam because the test program has its own set of filters. All this does is relay its message to the next program along.
        let mut test_program = context.send(()).unwrap();

        while let Some(input) = input.next().await { 
            test_program.send(input).await.unwrap(); 
        }
    }, 0);
    scene.connect_programs(message1_sender_program, message2_receiver_program, StreamId::with_message_type::<Message1>()).unwrap();

    // Test program receives message2
    TestBuilder::new()
        .expect_message(|msg2: Message2| if msg2 != Message2::Msg("Hello".to_string()) { Err(format!("Expected 'Hello'")) } else { Ok(()) })
        .expect_message(|msg2: Message2| if msg2 != Message2::Msg("Goodbyte".to_string()) { Err(format!("Expected 'Goodbyte'")) } else { Ok(()) })
        .run_in_scene(&scene, test_program);
}

#[test]
fn filter_at_source_with_any_target() {
    // Create a standard scene
    let scene = Scene::default();

    // Filters can also be specified as a source for a stream. These change the input for all targets that target that source
    #[derive(Debug)]
    enum Message1 { Msg(String) }
    #[derive(Debug, PartialEq)]
    enum Message2 { Msg(String) }

    impl SceneMessage for Message1 { }
    impl SceneMessage for Message2 { }

    let msg1_to_msg2 = FilterHandle::for_filter(|msg1: InputStream<Message1>| { msg1.map(|msg| match msg { Message1::Msg(val) => Message2::Msg(val) })});

    // For any stream that tries to connect to a program with an input stream of type 'Message2' using 'Message1', automatically use the filter we just defined
    // Target filters will take priority
    scene.connect_programs(msg1_to_msg2, (), StreamId::with_message_type::<Message1>()).unwrap();

    // The IDs of the two programs involved
    let message1_sender_program     = SubProgramId::new();
    let message2_receiver_program   = SubProgramId::new();
    let test_program                = SubProgramId::new();

    // Create a program that generates 'message1' as an output
    scene.add_subprogram(message1_sender_program, |_: InputStream<()>, context| async move {
        let mut sender = context.send(()).unwrap();

        println!("Sending 1...");
        sender.send(Message1::Msg("Hello".to_string())).await.unwrap();
        println!("Sending 2...");
        sender.send(Message1::Msg("Goodbyte".to_string())).await.unwrap();
        println!("Done");
    }, 0);

    // message2_receiver_program should receive all messages of the target type. Temp subprogram here is replaced by the test later on
    scene.add_subprogram(message2_receiver_program, |mut input: InputStream<Message2>, context| async move {
        // We need an intermediate prorgam because the test program has its own set of filters. All this does is relay its message to the next program along.
        let mut test_program = context.send(()).unwrap();

        while let Some(input) = input.next().await { 
            // We resend as a string, as otherwise the connection below will be overridden by the test
            match input {
                Message2::Msg(msg) => test_program.send(msg).await.unwrap()
            }
        }
    }, 0);

    // Anything that generates 'Message2' should be connected to the message2 receiver program
    scene.connect_programs((), message2_receiver_program, StreamId::with_message_type::<Message2>()).unwrap();

    // Test program receives strings relayed by the receiver program
    TestBuilder::new()
        .expect_message(|msg2: String| if msg2 != "Hello".to_string() { Err(format!("Expected 'Hello'")) } else { Ok(()) })
        .expect_message(|msg2: String| if msg2 != "Goodbyte".to_string() { Err(format!("Expected 'Goodbyte'")) } else { Ok(()) })
        .run_in_scene(&scene, test_program);
}

#[test]
fn filter_at_source_with_direct_target() {
    // Create a standard scene
    let scene = Scene::default();

    // Filters can also be specified as a source for a stream. These change the input for all targets that target that source
    #[derive(Debug)]
    enum Message1 { Msg(String) }
    #[derive(Debug, PartialEq)]
    enum Message2 { Msg(String) }

    impl SceneMessage for Message1 { }
    impl SceneMessage for Message2 { }

    let msg1_to_msg2 = FilterHandle::for_filter(|msg1: InputStream<Message1>| { msg1.map(|msg| match msg { Message1::Msg(val) => Message2::Msg(val) })});

    // For any stream that tries to connect to a program with an input stream of type 'Message2' using 'Message1', automatically use the filter we just defined
    // Target filters will take priority
    scene.connect_programs(msg1_to_msg2, (), StreamId::with_message_type::<Message1>()).unwrap();

    // The IDs of the two programs involved
    let message1_sender_program     = SubProgramId::new();
    let message2_receiver_program   = SubProgramId::new();
    let test_program                = SubProgramId::new();

    // Create a program that sends Message1 directly to the receiver program
    scene.add_subprogram(message1_sender_program, |_: InputStream<()>, context| async move {
        let mut sender = context.send(message2_receiver_program).unwrap();

        println!("Sending 1...");
        sender.send(Message1::Msg("Hello".to_string())).await.unwrap();
        println!("Sending 2...");
        sender.send(Message1::Msg("Goodbyte".to_string())).await.unwrap();
        println!("Done");
    }, 0);

    // message2_receiver_program should receive all messages of the target type. Temp subprogram here is replaced by the test later on
    scene.add_subprogram(message2_receiver_program, |mut input: InputStream<Message2>, context| async move {
        // We need an intermediate prorgam because the test program has its own set of filters. All this does is relay its message to the next program along.
        let mut test_program = context.send(()).unwrap();

        while let Some(input) = input.next().await { 
            // We resend as a string, as otherwise the connection below will be overridden by the test
            match input {
                Message2::Msg(msg) => test_program.send(msg).await.unwrap()
            }
        }
    }, 0);

    // Test program receives strings relayed by the receiver program
    TestBuilder::new()
        .expect_message(|msg2: String| if msg2 != "Hello".to_string() { Err(format!("Expected 'Hello'")) } else { Ok(()) })
        .expect_message(|msg2: String| if msg2 != "Goodbyte".to_string() { Err(format!("Expected 'Goodbyte'")) } else { Ok(()) })
        .run_in_scene(&scene, test_program);
}

#[test]
fn chain_filters_with_target_filter() {
    // Create a standard scene
    let scene = Scene::default();

    // Filters can also be specified as a source for a stream. These change the input for all targets that target that source
    #[derive(Debug)]
    enum Message1 { Msg(String) }
    #[derive(Debug)]
    enum Message2 { Msg(String) }
    #[derive(Debug)]
    enum Message3 { Msg(String) }

    impl SceneMessage for Message1 { }
    impl SceneMessage for Message2 { }
    impl SceneMessage for Message3 { }

    // Our source program outputs Message1, and we install a source filter that can convert that to Message2
    // The target program receives Message3 but has an input filter that can convert from Message2 (so if we chain the first filter to the second we should be able to receive the messages)
    let msg1_to_msg2 = FilterHandle::for_filter(|msg1: InputStream<Message1>| { msg1.map(|msg| match msg { Message1::Msg(val) => Message2::Msg(val) })});
    let msg2_to_msg3 = FilterHandle::for_filter(|msg1: InputStream<Message2>| { msg1.map(|msg| match msg { Message2::Msg(val) => Message3::Msg(val) })});

    // For any stream that tries to connect to a program with an input stream of type 'Message2' using 'Message1', automatically use the filter we just defined
    // Target filters will take priority
    scene.connect_programs(msg1_to_msg2, (), StreamId::with_message_type::<Message1>()).unwrap();

    // The IDs of the two programs involved
    let message1_sender_program     = SubProgramId::new();
    let message2_receiver_program   = SubProgramId::new();
    let message3_receiver_program   = SubProgramId::new();
    let test_program                = SubProgramId::new();

    // Create a program that generates 'message1' as an output
    scene.add_subprogram(message1_sender_program, |_: InputStream<()>, context| async move {
        let mut sender = context.send(()).unwrap();

        sender.send(Message1::Msg("Hello".to_string())).await.unwrap();
        sender.send(Message1::Msg("Goodbyte".to_string())).await.unwrap();
    }, 0);

    // message2_receiver_program should receive all messages of the target type. Temp subprogram here is replaced by the test later on
    scene.add_subprogram(message2_receiver_program, |mut input: InputStream<Message2>, _| async move {
        while let Some(_input) = input.next().await { 
            // We should connect using the msg1_to_msg3 filter
            assert!(false, "Should receive only Message3");
        }
    }, 0);

    // message3_receiver_program is as above but instead receives Message3
    scene.add_subprogram(message3_receiver_program, |mut input, context| async move {
        // We need an intermediate prorgam because the test program has its own set of filters. All this does is relay its message to the next program along.
        let mut test_program = context.send(()).unwrap();

        while let Some(input) = input.next().await { 
            // We resend as a string, as otherwise the connection below will be overridden by the test
            match input {
                Message3::Msg(msg) => test_program.send(msg).await.unwrap()
            }
        }
    }, 0);

    // Anything that generates 'Message2' should be connected to the initial message2 receiver program
    scene.connect_programs((), message2_receiver_program, StreamId::with_message_type::<Message2>()).unwrap();
    scene.connect_programs((), message3_receiver_program, StreamId::with_message_type::<Message3>()).unwrap();

    // Now make message3_receiver_program a receiver of Message2 messages via a filter (these can't be further filtered, so Message1 should no longer be sendable)
    scene.connect_programs((), StreamTarget::Filtered(msg2_to_msg3, message3_receiver_program), StreamId::with_message_type::<Message2>()).unwrap();

    // Test program receives strings relayed by the receiver program
    TestBuilder::new()
        .expect_message(|msg2: String| if msg2 != "Hello".to_string() { Err(format!("Expected 'Hello'")) } else { Ok(()) })
        .expect_message(|msg2: String| if msg2 != "Goodbyte".to_string() { Err(format!("Expected 'Goodbyte'")) } else { Ok(()) })
        .run_in_scene(&scene, test_program);
}

#[test]
fn chain_two_filters_with_target_filter_1() {
    // Create a standard scene
    let scene = Scene::default();

    // Filters can also be specified as a source for a stream. These change the input for all targets that target that source
    #[derive(Debug)]
    enum Message1 { Msg(String) }
    #[derive(Debug)]
    enum Message2 { Msg(String) }
    #[derive(Debug)]
    enum Message3 { Msg(String) }
    #[derive(Debug)]
    enum Message4 { Msg(String) }

    impl SceneMessage for Message1 { }
    impl SceneMessage for Message2 { }
    impl SceneMessage for Message3 { }
    impl SceneMessage for Message4 { }

    let msg1_to_msg2 = FilterHandle::for_filter(|msg1: InputStream<Message1>| { msg1.map(|msg| match msg { Message1::Msg(val) => Message2::Msg(val) })});
    let msg1_to_msg4 = FilterHandle::for_filter(|msg1: InputStream<Message1>| { msg1.map(|msg| match msg { Message1::Msg(val) => Message4::Msg(val) })});
    let msg2_to_msg3 = FilterHandle::for_filter(|msg1: InputStream<Message2>| { msg1.map(|msg| match msg { Message2::Msg(val) => Message3::Msg(val) })});

    // Add converters for both message 2 and message 4. We should be able to find the 1 -> 2 -> 3 chain
    // We can have more than one filter installed on a source filter, and we'll pick one that lets us match the target if we can
    scene.connect_programs(msg1_to_msg2, (), StreamId::with_message_type::<Message1>()).unwrap();
    scene.connect_programs(msg1_to_msg4, (), StreamId::with_message_type::<Message1>()).unwrap();

    // The IDs of the two programs involved
    let message1_sender_program     = SubProgramId::new();
    let message2_receiver_program   = SubProgramId::new();
    let message3_receiver_program   = SubProgramId::new();
    let test_program                = SubProgramId::new();

    // Create a program that generates 'message1' as an output
    scene.add_subprogram(message1_sender_program, |_: InputStream<()>, context| async move {
        let mut sender = context.send(()).unwrap();

        sender.send(Message1::Msg("Hello".to_string())).await.unwrap();
        sender.send(Message1::Msg("Goodbyte".to_string())).await.unwrap();
    }, 0);

    // message2_receiver_program should receive all messages of the target type. Temp subprogram here is replaced by the test later on
    scene.add_subprogram(message2_receiver_program, |mut input: InputStream<Message2>, _| async move {
        while let Some(_input) = input.next().await { 
            // We should connect using the msg1_to_msg3 filter
            assert!(false, "Should receive only Message3");
        }
    }, 0);

    // message3_receiver_program is as above but instead receives Message3
    scene.add_subprogram(message3_receiver_program, |mut input, context| async move {
        // We need an intermediate prorgam because the test program has its own set of filters. All this does is relay its message to the next program along.
        let mut test_program = context.send(()).unwrap();

        while let Some(input) = input.next().await { 
            // We resend as a string, as otherwise the connection below will be overridden by the test
            match input {
                Message3::Msg(msg) => test_program.send(msg).await.unwrap()
            }
        }
    }, 0);

    // Anything that generates 'Message2' should be connected to the initial message2 receiver program
    scene.connect_programs((), message2_receiver_program, StreamId::with_message_type::<Message2>()).unwrap();
    scene.connect_programs((), message3_receiver_program, StreamId::with_message_type::<Message3>()).unwrap();

    // Now make message3_receiver_program a receiver of Message2 messages via a filter (these can't be further filtered, so Message1 should no longer be sendable)
    scene.connect_programs((), StreamTarget::Filtered(msg2_to_msg3, message3_receiver_program), StreamId::with_message_type::<Message2>()).unwrap();

    // Test program receives strings relayed by the receiver program
    TestBuilder::new()
        .expect_message(|msg2: String| if msg2 != "Hello".to_string() { Err(format!("Expected 'Hello'")) } else { Ok(()) })
        .expect_message(|msg2: String| if msg2 != "Goodbyte".to_string() { Err(format!("Expected 'Goodbyte'")) } else { Ok(()) })
        .run_in_scene(&scene, test_program);
}

#[test]
fn chain_two_filters_with_target_filter_2() {
    // Create a standard scene
    let scene = Scene::default();

    // Filters can also be specified as a source for a stream. These change the input for all targets that target that source
    #[derive(Debug)]
    enum Message1 { Msg(String) }
    #[derive(Debug)]
    enum Message2 { Msg(String) }
    #[derive(Debug)]
    enum Message3 { Msg(String) }
    #[derive(Debug)]
    enum Message4 { Msg(String) }

    impl SceneMessage for Message1 { }
    impl SceneMessage for Message2 { }
    impl SceneMessage for Message3 { }
    impl SceneMessage for Message4 { }

    let msg1_to_msg2 = FilterHandle::for_filter(|msg1: InputStream<Message1>| { msg1.map(|msg| match msg { Message1::Msg(val) => Message2::Msg(val) })});
    let msg1_to_msg4 = FilterHandle::for_filter(|msg1: InputStream<Message1>| { msg1.map(|msg| match msg { Message1::Msg(val) => Message4::Msg(val) })});
    let msg2_to_msg3 = FilterHandle::for_filter(|msg1: InputStream<Message2>| { msg1.map(|msg| match msg { Message2::Msg(val) => Message3::Msg(val) })});

    // Add converters for both message 2 and message 4. We should be able to find the 1 -> 2 -> 3 chain
    // We can have more than one filter installed on a source filter, and we'll pick one that lets us match the target if we can
    scene.connect_programs(msg1_to_msg4, (), StreamId::with_message_type::<Message1>()).unwrap();
    scene.connect_programs(msg1_to_msg2, (), StreamId::with_message_type::<Message1>()).unwrap();

    // The IDs of the two programs involved
    let message1_sender_program     = SubProgramId::new();
    let message2_receiver_program   = SubProgramId::new();
    let message3_receiver_program   = SubProgramId::new();
    let test_program                = SubProgramId::new();

    // Create a program that generates 'message1' as an output
    scene.add_subprogram(message1_sender_program, |_: InputStream<()>, context| async move {
        let mut sender = context.send(()).unwrap();

        sender.send(Message1::Msg("Hello".to_string())).await.unwrap();
        sender.send(Message1::Msg("Goodbyte".to_string())).await.unwrap();
    }, 0);

    // message2_receiver_program should receive all messages of the target type. Temp subprogram here is replaced by the test later on
    scene.add_subprogram(message2_receiver_program, |mut input: InputStream<Message2>, _| async move {
        while let Some(_input) = input.next().await { 
            // We should connect using the msg1_to_msg3 filter
            assert!(false, "Should receive only Message3");
        }
    }, 0);

    // message3_receiver_program is as above but instead receives Message3
    scene.add_subprogram(message3_receiver_program, |mut input, context| async move {
        // We need an intermediate prorgam because the test program has its own set of filters. All this does is relay its message to the next program along.
        let mut test_program = context.send(()).unwrap();

        while let Some(input) = input.next().await { 
            // We resend as a string, as otherwise the connection below will be overridden by the test
            match input {
                Message3::Msg(msg) => test_program.send(msg).await.unwrap()
            }
        }
    }, 0);

    // Anything that generates 'Message2' should be connected to the initial message2 receiver program
    scene.connect_programs((), message2_receiver_program, StreamId::with_message_type::<Message2>()).unwrap();
    scene.connect_programs((), message3_receiver_program, StreamId::with_message_type::<Message3>()).unwrap();

    // Now make message3_receiver_program a receiver of Message2 messages via a filter (these can't be further filtered, so Message1 should no longer be sendable)
    scene.connect_programs((), StreamTarget::Filtered(msg2_to_msg3, message3_receiver_program), StreamId::with_message_type::<Message2>()).unwrap();

    // Test program receives strings relayed by the receiver program
    TestBuilder::new()
        .expect_message(|msg2: String| if msg2 != "Hello".to_string() { Err(format!("Expected 'Hello'")) } else { Ok(()) })
        .expect_message(|msg2: String| if msg2 != "Goodbyte".to_string() { Err(format!("Expected 'Goodbyte'")) } else { Ok(()) })
        .run_in_scene(&scene, test_program);
}

#[test]
fn chain_with_direct_target() {
    // Create a standard scene
    let scene = Scene::default();

    // Filters can also be specified as a source for a stream. These change the input for all targets that target that source
    #[derive(Debug)]
    enum Message1 { Msg(String) }
    #[derive(Debug, PartialEq)]
    enum Message2 { Msg(String) }
    #[derive(Debug)]
    enum Message3 { Msg(String) }

    impl SceneMessage for Message1 { }
    impl SceneMessage for Message2 { }
    impl SceneMessage for Message3 { }

    let msg1_to_msg2 = FilterHandle::for_filter(|msg1: InputStream<Message1>| { msg1.map(|msg| match msg { Message1::Msg(val) => Message2::Msg(val) })});
    let msg2_to_msg3 = FilterHandle::for_filter(|msg1: InputStream<Message2>| { msg1.map(|msg| match msg { Message2::Msg(val) => Message3::Msg(val) })});

    // For any stream that tries to connect to a program with an input stream of type 'Message2' using 'Message1', automatically use the filter we just defined
    // Target filters will take priority
    scene.connect_programs(msg1_to_msg2, (), StreamId::with_message_type::<Message1>()).unwrap();

    // The IDs of the two programs involved
    let message1_sender_program     = SubProgramId::new();
    let message3_receiver_program   = SubProgramId::new();
    let test_program                = SubProgramId::new();

    // Create a program that sends Message1 directly to the receiver program
    scene.add_subprogram(message1_sender_program, |_: InputStream<()>, context| async move {
        // Send message1 direct to message3 receiver (should convert to message2 then message3)
        let mut sender = context.send(message3_receiver_program).unwrap();

        println!("Sending 1...");
        sender.send(Message1::Msg("Hello".to_string())).await.unwrap();
        println!("Sending 2...");
        sender.send(Message1::Msg("Goodbyte".to_string())).await.unwrap();
        println!("Done");
    }, 0);

    // message2_receiver_program should receive all messages of the target type. Temp subprogram here is replaced by the test later on
    scene.add_subprogram(message3_receiver_program, |mut input, context| async move {
        // We need an intermediate prorgam because the test program has its own set of filters. All this does is relay its message to the next program along.
        let mut test_program = context.send(()).unwrap();

        while let Some(input) = input.next().await { 
            // We resend as a string, as otherwise the connection below will be overridden by the test
            match input {
                Message3::Msg(msg) => test_program.send(msg).await.unwrap()
            }
        }
    }, 0);

    // Add an input filter to the message 3 program
    scene.connect_programs((), StreamTarget::Filtered(msg2_to_msg3, message3_receiver_program), StreamId::with_message_type::<Message2>()).unwrap();

    // Test program receives strings relayed by the receiver program
    TestBuilder::new()
        .expect_message(|msg2: String| if msg2 != "Hello".to_string() { Err(format!("Expected 'Hello'")) } else { Ok(()) })
        .expect_message(|msg2: String| if msg2 != "Goodbyte".to_string() { Err(format!("Expected 'Goodbyte'")) } else { Ok(()) })
        .run_in_scene(&scene, test_program);
}
