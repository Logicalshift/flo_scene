use crate::*;

use futures::prelude::*;
use futures::channel::mpsc;
use futures::executor;
use futures::{pin_mut};
use once_cell::sync::{Lazy};

use std::thread;
use std::io::{BufRead};

pub static STDIN_PROGRAM: Lazy<SubProgramId> = Lazy::new(|| SubProgramId::called("STDIN_PROGRAM"));

///
/// Text input programs read from an input stream and sends `TextInputResult` messages to a target program
///
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TextInput {
    /// Reads a single character from an input stream and sends it as a TextInputResult to a target program
    RequestCharacter(SubProgramId),

    /// Reads a line of text from an input stream and sends it to a subprogram
    RequestLine(SubProgramId),
}

///
/// The message that's sent as a response to a text input request
///
#[derive(Clone, Debug)]
pub enum TextInputResult {
    /// The stream produced some characters as a result of a request
    Characters(String),

    /// The input stream was closed before the input could be generated
    Eof,
}

impl SceneMessage for TextInputResult { }

impl SceneMessage for TextInput {
    fn default_target() -> StreamTarget { (*STDIN_PROGRAM).into() }
}

///
/// An input subprogram that reads from a `Read` object on request
///
/// This version works by creating a background thread to monitor the input - an approach that doesn't require any extra
/// dependencies and works with anything that implements the [`Read`](std::io::Read) trait, but which becomes less viable 
/// as the number of streams increases)
///
pub async fn text_input_subprogram(source: impl 'static + Send + BufRead, messages: impl Stream<Item=TextInput>, context: SceneContext) {
    // Create some mpsc streams to communicate with the I/O thread
    let (send_request, recv_request)    = mpsc::channel::<TextInput>(0);
    let (send_result, recv_result)      = mpsc::channel::<(SubProgramId, TextInputResult)>(0);

    // The monitor thread runs in the background so we don't block the main process, and monitors the target
    let monitor_thread = thread::spawn(move || executor::block_on(async move {
        use TextInput::*;

        // Requests are forwarded to this thread via the mpsc queue
        let mut recv_request    = recv_request;
        let mut source          = source;
        let mut send_result     = send_result;

        while let Some(input_request) = recv_request.next().await {
            match input_request {
                RequestCharacter(target) => {
                    todo!()
                }

                RequestLine(target) => {
                    // Read a line from the input
                    let mut line = String::new();
                    let read_err = source.read_line(&mut line);

                    // Relay the string that was read
                    let send_err = match read_err {
                        Ok(_)   => send_result.send((target, TextInputResult::Characters(line))).await,
                        Err(_)  => send_result.send((target, TextInputResult::Eof)).await
                    };

                    // Stop receiving requests if there's an error reading from the source
                    if read_err.is_err() || send_err.is_err() {
                        break;
                    }
                }
            }
        }
    }));

    // Read from the input and relay any messages
    pin_mut!(messages);
    let mut send_request    = send_request;
    let mut recv_result     = recv_result;

    while let Some(request) = messages.next().await {
        let result = {
            // Send the requesut
            if send_request.send(request).await.is_ok() {
                // Read the response
                if let Some((target, message)) = recv_result.next().await {
                    // Send to the target (ignoring errors here)
                    if let Ok(mut target) = context.send(target) {
                        target.send(message).await.ok();
                    }

                    // Continue to run
                    Ok(())
                } else {
                    // Could not read the result (input has ended)
                    Err(())
                }
            } else {
                // Could not send the result (input has ended)
                Err(())
            }
        };

        if result.is_err() {
            use TextInput::*;

            // Errors result in an EOF being sent to the target
            let target = match request {
                RequestCharacter(target) | RequestLine(target) => target
            };

            if let Ok(mut target) = context.send(target) {
                target.send(TextInputResult::Eof).await.ok();
            }
        }
    }
}
