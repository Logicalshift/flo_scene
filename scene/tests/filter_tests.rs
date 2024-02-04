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
