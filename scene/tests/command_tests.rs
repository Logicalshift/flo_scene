use flo_scene::*;
use flo_scene::programs::*;
use flo_scene::commands::*;

use futures::prelude::*;

#[test]
fn simple_command() {
    let scene = Scene::default();

    // Create a test command that sends some usize values to its output
    let test_command = FnCommand::<(), usize>::new(|_input, context| async move {
        // Connect the usize output
        let mut output = context.send::<usize>(()).unwrap();

        // Send some output data
        output.send(1).await.unwrap();
        output.send(2).await.unwrap();
        output.send(3).await.unwrap();
        output.send(4).await.unwrap();
    });

    // Run the command using the test builder
    let test_program = SubProgramId::new();
    TestBuilder::new()
        .run_command(test_command.clone(), vec![], |output| if &output != &vec![1, 2, 3, 4] { Err(format!("Unexpected command output: {:?}", output)) } else { Ok(()) })
        .run_in_scene_with_threads(&scene, test_program, 5);
}

#[test]
fn pipe_command() {
    let scene = Scene::empty();

    // Create a test command that sends some usize values to its output
    let test_command = FnCommand::<(), usize>::new(|_input, context| async move {
        // Connect the usize output
        let mut output = context.send::<usize>(()).unwrap();

        // Send some output data
        println!("send(1)");
        output.send(1).await.unwrap();
        println!("send(2)");
        output.send(2).await.unwrap();
        println!("send(3)");
        output.send(3).await.unwrap();
        println!("send(4)");
        output.send(4).await.unwrap();
        println!("done.");
    });

    let add_one_command = FnCommand::<usize, usize>::new(|input, context| async move {
        let mut input  = input;
        let mut output = context.send::<usize>(()).unwrap();

        // Add one to the input
        println!("+1 start");
        while let Some(next) = input.next().await {
            println!("+1: {:?}", next);
            output.send(next+1).await.unwrap();
            println!("  = {:?}", next+1);
        }
        println!("+1 done");
    });

    let combined_command = test_command.pipe_to(add_one_command);

    // Run the command using the test builder
    let test_program = SubProgramId::new();
    TestBuilder::new()
        .run_command(combined_command.clone(), vec![], |output| if &output != &vec![2, 3, 4, 5] { Err(format!("Unexpected command output: {:?}", output)) } else { Ok(()) })
        .run_in_scene_with_threads(&scene, test_program, 5);
}

#[test]
fn query_command() {
    let scene = Scene::default();

    // Run the command using the test builder
    let test_program = SubProgramId::new();
    TestBuilder::new()
        .send_message(IdleRequest::WhenIdle(test_program))
        .expect_message(|_: IdleNotification| { Ok(()) })
        .run_query(ReadCommand::default(), Query::<SceneUpdate>::with_no_target(), *SCENE_CONTROL_PROGRAM, |output| if output.len() == 0 { Err(format!("Unexpected command output: {:?}", output)) } else { Ok(()) })
        .run_in_scene_with_threads(&scene, test_program, 5);
}
