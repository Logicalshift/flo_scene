use crate::parser::*;
use crate::socket::*;
use crate::commands::command_stream::*;

use futures::prelude::*;
use futures::stream::{BoxStream};
use futures::task::{Poll};
use futures::channel::mpsc;
use futures::{pin_mut};

use serde_json;

use std::collections::{HashMap};
use std::iter;
use std::sync::*;

///
/// Data intended to be sent to a command socket (a command socket sends and receives the bytes directly)
///
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CommandData(pub Vec<u8>);

impl From<&str> for CommandData {
    #[inline]
    fn from(string: &str) -> Self {
        Self(string.bytes().collect())
    }
}

impl From<String> for CommandData {
    #[inline]
    fn from(string: String) -> Self {
        Self(string.into_bytes())
    }
}

///
/// The notifications that can be sent to a command socket when it's in 'normal' mode
///
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum CommandNotification {
    /// Display the '> ' prompt, indicating that we're ready for new commands
    Prompt,

    /// JSON response from a command
    JsonResponse(serde_json::Value),

    /// Informational message
    Message(String),

    /// Error message
    Error(String),

    /// Indicates that a background stream has started
    NewStream(usize),

    /// Indicates that a background stream has finished
    EndStream(usize),

    /// Received a value from a background stream
    StreamJson(usize, serde_json::Value),

    /// Change the IO mode of the socket (eg, to RAW or JSON)
    StartMode(String),

    /// Restore the original mode of the socket
    EndMode(String),
}


///
/// A command socket manages the socket connection for a command
///
/// Commands are usually streams of requests and responses, but the socket can also be taken over to send raw bytes or
/// streams of JSON messages, so we need a way of changing the input and output stream's states to match these conditions.
/// This type manages both the input and the output streams for this purpose.
///
pub struct CommandSocket {
    /// Data that has been read from the input stream but has not yet been parsed into a command
    buffer: Vec<u8>,

    /// The input stream for the command
    input_stream: BoxStream<'static, CommandData>,

    /// The output stream for the command
    output_stream: mpsc::Sender<CommandData>,

    /// The background streams that have been started on this socket. Background streams are monitored while waiting for input.
    background_json_streams: HashMap<usize, BoxStream<'static, serde_json::Value>>,

    /// The next handle to apply to a background stream
    next_background_stream_handle: usize,
}

impl CommandSocket {
    ///
    /// Creates a command socket by activating a socket connection
    ///
    pub fn connect(connection: SocketConnection<CommandData, CommandData>) -> Self {
        // Finish the connection to create the CommandSocket structure
        let (send_output, recv_output) = mpsc::channel(0);
        let input_stream = connection.connect(recv_output);

        Self {
            buffer:                         vec![],
            input_stream:                   input_stream,
            output_stream:                  send_output,
            background_json_streams:        HashMap::new(),
            next_background_stream_handle:  0,
        }
    }

    ///
    /// Reads the next request from the input stream
    ///
    pub async fn next_request(&mut self) -> Result<CommandRequest, CommandParseError> {
        use std::mem;

        // The input is whatever we have in the buffer + what we can read from the input stream
        let mut buffer      = vec![];
        mem::swap(&mut buffer, &mut self.buffer);

        let input           = &mut self.input_stream;
        let input           = stream::iter(iter::once(buffer)).chain(input.map(|CommandData(data)| data));

        // Set up a tokenizer and parser for the input
        let mut tokenizer   = Tokenizer::new(input);
        let mut parser      = Parser::new();

        tokenizer.with_command_matchers();

        // Read the next command using our parser/tokenizer
        let next_command = command_parse(&mut parser, &mut tokenizer).await;

        // Convert the tokenizer back to a buffer
        let buffer  = tokenizer.to_u8_lookahead();
        self.buffer = buffer;

        // Fetch the matched command from the parser
        match next_command {
            Ok(()) => {
                let command = parser.finish()?;
                Ok(command)
            }

            Err(err) => {
                Err(err)
            }
        }
    }

    ///
    /// Sends a notification to the output stream
    ///
    pub async fn notify(&mut self, notification: CommandNotification) -> Result<(), mpsc::SendError> {
        use CommandNotification::*;

        match notification {
            Prompt                      => {
                self.output_stream.send("\n\n> ".into()).await?;
            },

            JsonResponse(json)          => {
                // Format the JSON as a pretty-printed string (TODO: the to_writer_pretty version would be better for very long JSON)
                let json_string = serde_json::to_string_pretty(&json);

                if let Ok(json_string) = json_string {
                    self.output_stream.send(format!("{}\n", json_string).into()).await?;
                } else {
                    self.output_stream.send(format!("!!! {:?}\n", "Could not format JSON response").into()).await?;
                }
            },

            Message(message)            => {
                let msg = message.replace("\n", "\n   ");
                self.output_stream.send(format!("   {}\n", msg).into()).await?;
            },

            Error(message)              => {
                self.output_stream.send(format!("!!! {}\n", message).into()).await?;
            },

            NewStream(stream_id)        => {
                self.output_stream.send(format!("<<< {}\n", stream_id).into()).await?;
            },

            EndStream(stream_id)        => {
                self.output_stream.send(format!("=== {}\n", stream_id).into()).await?;
            },

            StreamJson(stream_id, json) => {
                // Format the JSON as a pretty-printed string (TODO: the to_writer_pretty version would be better for very long JSON)
                let json_string = serde_json::to_string_pretty(&json);

                if let Ok(json_string) = json_string {
                    self.output_stream.send(format!("\n<{} {}\n", stream_id, json_string).into()).await?;
                }
            },

            StartMode(mode)             => {
                self.output_stream.send(format!("\n<< {} <<\n\n", mode).into()).await?;
            },

            EndMode(mode)               => {
                self.output_stream.send(format!("\n== {} ==\n\n", mode).into()).await?;
            },
        }

        Ok(())
    }

    ///
    /// Sends a single response to the output of the command
    ///
    pub async fn send_response(&mut self, response: CommandResponse) -> Result<(), ()> {
        match response {
            CommandResponse::Message(msg) => {
                self.notify(CommandNotification::Message(msg)).await.map_err(|_| ())
            }

            CommandResponse::Json(json) => {
                self.notify(CommandNotification::JsonResponse(json)).await.map_err(|_| ())
            },

            CommandResponse::BackgroundStream(stream) => {
                // This requires moving the stream to the background
                let stream_id = self.next_background_stream_handle;
                self.next_background_stream_handle += 1;

                self.background_json_streams.insert(stream_id, stream);

                self.notify(CommandNotification::NewStream(stream_id)).await.map_err(|_| ())
            },

            CommandResponse::IoStream(create_stream) => {
                self.notify(CommandNotification::StartMode("JSON".into())).await.map_err(|_| ())?;

                // Take over the command stream
                self.stream_json(move |input, output| async move {
                    // We relay input via a channel as these streams can have a static lifetime
                    let (send_input, recv_input) = mpsc::channel(0);

                    // Create the output stream using the supplied functions
                    let mut output_stream   = create_stream(recv_input.boxed());
                    let mut output          = output;

                    future::join(async move {
                        // Copy output from the interactive stream to the main output
                        while let Some(bytes) = output_stream.next().await {
                            if output.send(bytes).await.is_err() {
                                break;
                            }
                        }
                    }, async move {
                        // Copy input from the main stream to the interactive stream (should finish once the interactive stream stops waiting for input)
                        let mut input = input;
                        let mut send_input = send_input;

                        while let Some(input) = input.next().await {
                            if send_input.send(input).await.is_err() {
                                break;
                            }
                        }
                    }).await;
                }).await;

                self.notify(CommandNotification::EndMode("JSON".into())).await.map_err(|_| ())?;

                Ok(())
            },

            CommandResponse::InteractiveStream(create_stream) => {
                self.notify(CommandNotification::StartMode("RAW".into())).await.map_err(|_| ())?;

                // Take over the command stream
                self.stream_raw(move |input, output| async move {
                    // We relay input via a channel as these streams can have a static lifetime
                    let (send_input, recv_input) = mpsc::channel(0);

                    // Create the output stream using the supplied functions
                    let mut output_stream   = create_stream(recv_input.boxed());
                    let mut output          = output;

                    future::join(async move {
                        // Copy output from the interactive stream to the main output
                        while let Some(bytes) = output_stream.next().await {
                            if output.send(bytes).await.is_err() {
                                break;
                            }
                        }
                    }, async move {
                        // Copy input from the main stream to the interactive stream (should finish once the interactive stream stops waiting for input)
                        let mut input = input;
                        let mut send_input = send_input;

                        while let Some(input) = input.next().await {
                            if send_input.send(input).await.is_err() {
                                break;
                            }
                        }
                    }).await;
                }).await;

                self.notify(CommandNotification::EndMode("JSON".into())).await.map_err(|_| ())?;

                Ok(())
            }

            CommandResponse::Error(error_message) => {
                self.notify(CommandNotification::Error(error_message)).await.map_err(|_| ())
            }
        }
    }

    ///
    /// Sends responses from a command
    ///
    pub async fn send_responses(&mut self, responses: impl Send + Stream<Item=CommandResponse>) -> Result<(), ()> {
        pin_mut!(responses);

        while let Some(response) = responses.next().await {
            self.send_response(response).await?;
        }

        Ok(())
    }

    ///
    /// Takes over the socket to send a stream of raw JSON data
    ///
    /// JSON is read from the input stream until a '.' is encountered (at the top level), or an error is encountered (at any point).
    ///
    pub async fn stream_json<'a, TFuture>(&'a mut self, activity_fn: impl 'a + FnOnce(BoxStream<'a, serde_json::Value>, mpsc::Sender<serde_json::Value>) -> TFuture) -> TFuture::Output 
    where
        TFuture: 'a + Future,
    {
        use std::mem;

        // Fetch the streams for the JSON
        let mut buffer      = vec![];
        mem::swap(&mut buffer, &mut self.buffer);
        let buffer          = Arc::new(Mutex::new(buffer));
        let input_stream    = &mut self.input_stream;
        let output_stream   = &mut self.output_stream;

        // Read first from the internal buffer, then the main input stream
        let input_stream_buffer = Arc::clone(&buffer);

        let input_stream = stream::poll_fn(move |context| {
            let mut buffer = input_stream_buffer.lock().unwrap();

            if !buffer.is_empty() {
                let mut ready_buffer = vec![];
                mem::swap(&mut ready_buffer, &mut *buffer);

                Poll::Ready(Some(ready_buffer))
            } else if let Poll::Ready(data) = input_stream.poll_next_unpin(context) {
                Poll::Ready(data.map(|CommandData(data)| data))
            } else {
                Poll::Pending
            }
        }).boxed();

        // The input stream is read as tokenized JSON and ends on error
        // TODO: need to return the input buffer from the tokenizer when dropped/finished
        let mut tokenizer   = Tokenizer::<JsonToken, _>::new(input_stream);
        let mut parser      = Parser::new();

        tokenizer.with_json_matchers();

        let (send_input, recv_input) = mpsc::channel(0);
        let parse_input = async move {
            let mut send_input = send_input;

            loop {
                match json_parse_value(&mut parser, &mut tokenizer).await {
                    Ok(()) => {
                        // Finish on any JSON error ('.' is intended as the 'true' finish here)
                        let value = parser.finish();

                        match value {
                            Err(_)    => { break; }
                            Ok(value) => {
                                if send_input.send(value).await.is_err() { break; } 
                            }
                        }
                    },

                    Err(_) => { break; }
                }
            }
        };

        // The output stream is formatted JSON
        let (send_output, recv_output) = mpsc::channel(0);

        let output_relay = async move {
            let mut recv_output = recv_output;
            while let Some(json) = recv_output.next().await {
                let json: serde_json::Value = json;
                let json_string = serde_json::to_string_pretty(&json);

                if let Ok(json_string) = json_string {
                    if output_stream.send(json_string.into()).await.is_err() {
                        break;
                    }
                    if output_stream.send(CommandData("\n\n".into())).await.is_err() {
                        break;
                    }
                }
            }
        };

        // Start the activity
        let activity = activity_fn(recv_input.boxed(), send_output);

        // Run all the futures together to wait for the activity to finish
        let mut parse_input     = Some(Box::pin(parse_input));
        let mut output_relay    = Some(Box::pin(output_relay));
        let mut activity        = Box::pin(activity);

        let result = future::poll_fn(|context| {
            if let Some(parse_input_future) = &mut parse_input {
                if parse_input_future.poll_unpin(context).is_ready() {
                    parse_input = None;
                }
            }

            if let Some(output_relay_future) = &mut output_relay {
                if output_relay_future.poll_unpin(context).is_ready() {
                    output_relay = None;
                }
            }

            activity.poll_unpin(context)
        }).await;

        result
    }

    ///
    /// Takes over the socket to send raw u8 data
    ///
    /// The `activity_fn` is called back to perform whatever activity is needed on the stream: control is returned once this function has completed.
    ///
    pub async fn stream_raw<'a, TFuture>(&'a mut self, activity_fn: impl 'a + FnOnce(BoxStream<'a, Vec<u8>>, mpsc::Sender<Vec<u8>>) -> TFuture) -> TFuture::Output
    where
        TFuture: 'a + Future,
    {
        use std::mem;

        // Fetch the input and output streams
        let mut buffer      = vec![];
        mem::swap(&mut buffer, &mut self.buffer);
        let buffer          = Arc::new(Mutex::new(buffer));
        let input_stream    = &mut self.input_stream;
        let output_stream   = &mut self.output_stream;

        // Create a variant of the input stream that reads from the internal buffer first
        let input_stream_buffer = Arc::clone(&buffer);

        let input_stream = stream::poll_fn(move |context| {
            let mut buffer = input_stream_buffer.lock().unwrap();

            if !buffer.is_empty() {
                let mut ready_buffer = vec![];
                mem::swap(&mut ready_buffer, &mut *buffer);

                Poll::Ready(Some(ready_buffer))
            } else if let Poll::Ready(data) = input_stream.poll_next_unpin(context) {
                Poll::Ready(data.map(|CommandData(data)| data))
            } else {
                Poll::Pending
            }
        }).boxed();

        // Create a future for forwarding the output (we could also expose the output sender directly here, note that there's an extra item allowed to be queued for backpressure purposes)
        let (send_output, recv_output) = mpsc::channel(0);

        let output_relay = async move {
            // Copy output until the output stream closes
            let mut recv_output = recv_output;
            while let Some(output) = recv_output.next().await {
                let send_result = output_stream.send(CommandData(output)).await;
                if send_result.is_err() {
                    break;
                }
            }
        };

        // Start the activity
        let activity = activity_fn(input_stream, send_output);

        // Run the activity and the output relay as a future
        let mut output_relay = Some(Box::pin(output_relay));
        pin_mut!(activity);

        let result = future::poll_fn(move |context| {
            if let Some(output_relay_future) = &mut output_relay {
                match output_relay_future.poll_unpin(context) {
                    Poll::Ready(()) => { output_relay = None; },
                    Poll::Pending   => { },
                }
            }

            activity.poll_unpin(context)
        }).await;

        // If the buffer wasn't read, return it to this object
        let mut buffer = buffer.lock().unwrap();
        mem::swap(&mut *buffer, &mut self.buffer);

        // Return the value returned from the activity future
        result
    }
}
