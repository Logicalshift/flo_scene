//!
//! Reads lines of text and counts the number of characters
//!

use flo_scene::*;
use flo_scene::programs::*;

use futures::prelude::*;
use futures::executor;

pub fn main() {
    // The default scene comes with some standard programs
    let scene = Scene::default();
    let sample_program = SubProgramId::new();

    scene.add_subprogram(sample_program, |input: InputStream<TextInputResult>, context| {
        async move {
            let mut input = input;

            loop {
                // Prompt for some input
                context.send_message(TextInput::PromptRequestLine(vec![TextOutput::Line("Enter text> ".to_string())], sample_program)).await.unwrap();

                // Wait for the input to arrive
                match input.next().await {
                    Some(TextInputResult::Characters(input)) => {
                        // Display a message about how many characters there were in the prompt
                        context.send_message(TextOutput::Line(format!("'{}' has {} characters\n", input, input.len()))).await.ok();
                    }

                    // Stop if the input stream is closed or an end-of-file is reached
                    Some(TextInputResult::Eof) | None => { break; }
                }
            }

            // Stop the scene once there's no more input
            context.send_message(SceneControl::StopScene).await.unwrap();
        }
    }, 0);

    executor::block_on(async {
        scene.run_scene().await;
    })
}
