use flo_scene::*;
use flo_scene::programs::*;

use futures::prelude::*;

#[test]
fn basic_timeout() {
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
