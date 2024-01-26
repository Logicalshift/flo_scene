//!
//! Reads lines of text and counts the number of characters
//!
//! This demonstrates the usual way that feedback works in a flo_scene system. It's not possible to directly receive
//! a response to a message: all programs can do is generate an output stream. So with the input subprogram, the
//! requests specify which subprogram to send the response to. In this case the same program is receiving the responses
//! as is making the requests so it reads from its input stream immediately after every request.
//!
//! (This is analagous to how shell scripts are written and is also similar to how requests made over TCP/IP work)
//!
//! Note that it's possible to connect the stream coming from the input program using a filter if a subprogram needs
//! to be able to deal with different kinds of message, or messages from multiple sources with different output
//! types.
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
