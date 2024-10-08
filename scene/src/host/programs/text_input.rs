use crate::host::*;
use super::text_output::*;

use futures::prelude::*;
use futures::channel::mpsc;
use futures::executor;
use futures::{pin_mut};

use std::str;
use std::thread;
use std::io::{BufRead};

use serde::*;

pub static STDIN_PROGRAM: StaticSubProgramId = StaticSubProgramId::called("flo_scene::stdin");

///
/// Text input programs read from an input stream and sends `TextInputResult` messages to a target program
///
#[derive(Clone, Debug, PartialEq, Eq)]
#[derive(Serialize, Deserialize)]
pub enum TextInput {
    /// Reads a single character from an input stream and sends it as a TextInputResult to a target program
    RequestCharacter(SubProgramId),

    /// Reads a line of text from an input stream and sends it to a subprogram
    RequestLine(SubProgramId),

    /// Sends some data to the TextOutput program before requesting a line of text (the output will be deferred until the input is being read)
    PromptRequestLine(Vec<TextOutput>, SubProgramId),
}

///
/// The message that's sent as a response to a text input request
///
#[derive(Clone, PartialEq, PartialOrd, Ord, Eq, Hash, Debug)]
#[derive(Serialize, Deserialize)]
pub enum TextInputResult {
    /// The stream produced some characters as a result of a request
    Characters(String),

    /// The input stream was closed before the input could be generated
    Eof,
}

impl SceneMessage for TextInputResult {
    #[inline]
    fn message_type_name() -> String { "flo_scene::TextInputResult".into() }
}

impl SceneMessage for TextInput {
    fn default_target() -> StreamTarget { (*STDIN_PROGRAM).into() }

    #[inline]
    fn message_type_name() -> String { "flo_scene::TextInput".into() }
}

///
/// An input subprogram that reads from a `Read` object on request
///
/// This version works by creating a background thread to monitor the input - an approach that doesn't require any extra
/// dependencies and works with anything that implements the [`Read`](std::io::Read) trait, but which becomes less viable 
/// as the number of streams increases)
///
pub async fn text_input_subprogram(source: impl 'static + Send + BufRead, messages: impl Stream<Item=TextInput>, context: SceneContext) {
    use std::mem;

    let mut text_output = context.send(()).unwrap();

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
                    let mut bytes       = vec![];

                    // Read a single character from the input
                    let result = loop {
                        // Read the next byte from the stream
                        let pos = bytes.len();
                        bytes.push(0);
                        let read_err = source.read(&mut bytes[pos..pos+1]);

                        // Stop if there's an error
                        match read_err {
                            Err(err)    => { break Err(err); },
                            Ok(0)       => { bytes.pop(); continue; }
                            Ok(_)       => { }
                        }

                        // Try to decode what we've read so far as a string
                        let utf8_error = str::from_utf8(&bytes);

                        match utf8_error {
                            Ok(chr)     => { break Ok(chr.to_string()); }
                            Err(err)    => {
                                if err.error_len().is_some() {
                                    // Encountered an unexpected byte
                                    break Ok("\u{fffd}".to_string());
                                }
                            }
                        }
                    };

                    // Relay the string that was read
                    let result_is_err = result.is_err();
                    let send_err = match result {
                        Ok(chr) => send_result.send((target, TextInputResult::Characters(chr))).await,
                        Err(_)  => send_result.send((target, TextInputResult::Eof)).await,
                    };

                    if result_is_err || send_err.is_err() {
                        break;
                    }
                }

                RequestLine(target) | PromptRequestLine(_, target) => {
                    // Read a line from the input
                    let mut line = String::new();
                    let read_err = source.read_line(&mut line);

                    // Trim the newline at the end
                    if line.ends_with('\n') {
                        line.remove(line.len()-1);

                        if line.ends_with('\r') {
                            line.remove(line.len()-1);
                        }
                    }

                    // Relay the string that was read (EOF if 0 characters were read)
                    let send_err = match read_err {
                        Ok(0)   => { send_result.send((target, TextInputResult::Eof)).await.ok(); Err(()) },
                        Ok(_)   => send_result.send((target, TextInputResult::Characters(line))).await.map_err(|_| ()),
                        Err(_)  => send_result.send((target, TextInputResult::Eof)).await.map_err(|_| ()),
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
        use TextInput::*;

        let target = match &request {
            RequestCharacter(target) | RequestLine(target) | PromptRequestLine(_, target) => *target
        };

        // Send the prompt, if there is one
        let request = match request {
            PromptRequestLine(prompt, target) => {
                // Send the prompt to the output
                for prompt_output in prompt {
                    text_output.send(prompt_output).await.ok();
                }

                // Request a line of data
                RequestLine(target)
            }

            other => other,
        };

        // Use the thread to read the request from the stream
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
            // Errors result in an EOF being sent to the target
            if let Ok(mut target) = context.send(target) {
                target.send(TextInputResult::Eof).await.ok();
            }
        }
    }

    // Close the channel
    send_request.close_channel();
    recv_result.close();
    mem::drop(send_request);
    mem::drop(recv_result);

    // Monitor thread should shut down once the channel closes
    monitor_thread.join().ok();
}
