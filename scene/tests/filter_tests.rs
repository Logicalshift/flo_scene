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

            filtered_output.send(1).await;
            filtered_output.send(2).await;
            filtered_output.send(3).await;
            filtered_output.send(4).await;
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
    let usize_to_string = FilterHandle::for_filter(|number_stream: InputStream<usize>| {
        println!("Connecting");
        let count_disconnects = CountDisconnents {};

        number_stream.map(move |num| count_disconnects.convert_to_string(num))
    });

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
    let number_program  = SubProgramId::new();
    let scene2          = scene.clone();
    scene.add_subprogram(
        number_program, 
        move |_: InputStream<()>, context| async move {
            println!("Create initial stream...");
            let mut filtered_output = context.send::<usize>(StreamTarget::Any).unwrap();
            println!("  ... created");

            // Send first two messages
            filtered_output.send(1).await;
            filtered_output.send(2).await;

            // Disconnect the stream
            println!("Disconnecting initial stream...");
            scene2.connect_programs(number_program, StreamTarget::None, StreamId::with_message_type::<usize>()).unwrap();
            println!("   ... disconnected");

            // Send another two messages into oblivion
            filtered_output.send(3).await;
            filtered_output.send(4).await;

            // Reconnect the two programs
            println!("Reconnecting filter stream...");
            scene2.connect_programs(number_program, StreamTarget::Filtered(usize_to_string, string_program), StreamId::with_message_type::<usize>()).unwrap();
            println!("   ... reconnected");

            // Final two messages
            filtered_output.send(5).await;
            filtered_output.send(6).await;

            // Disconnect them again
            scene2.connect_programs(number_program, StreamTarget::None, StreamId::with_message_type::<usize>()).unwrap();
        }, 
        0);

    // Start the programs connected
    scene.connect_programs(number_program, StreamTarget::Filtered(usize_to_string, string_program), StreamId::with_message_type::<usize>()).unwrap();

    // Run the scene
    let mut has_finished = false;
    executor::block_on(select(async {
        scene.run_scene().await;
        has_finished = true;
    }.boxed(), Delay::new(Duration::from_millis(5000))));

    // Received output should match the numbers
    let recv_messages   = (*recv_messages.lock().unwrap()).clone();
    let num_disconnects = NUM_DISCONNECTS.load(Ordering::Relaxed);

    assert!(recv_messages == vec![1.to_string(), 2.to_string(), 5.to_string(), 6.to_string()], "Test program did not send correct numbers (sent {:?})", recv_messages);
    assert!(num_disconnects == 2, "Filtered stream was not dropped the expected number of times ({} != 2)", num_disconnects);
    assert!(has_finished, "Scene did not finish when the programs terminated");
}
