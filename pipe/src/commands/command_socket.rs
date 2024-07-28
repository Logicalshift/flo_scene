use crate::parser::*;
use crate::socket::*;
use crate::commands::command_program::*;
use crate::commands::command_stream::*;
use crate::commands::parse_command::*;

use futures::prelude::*;
use futures::stream::{BoxStream};
use futures::task::{Poll};
use futures::channel::mpsc;
use futures::{pin_mut};

use serde_json;

use std::iter;

///
/// Data intended to be sent to a command socket (a command socket sends and receives the bytes directly)
///
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct CommandData(pub Vec<u8>);

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
}

impl CommandSocket {
    ///
    /// Creates a command socket by activating a socket connection
    ///
    pub fn connect(connection: SocketConnection<CommandData, CommandData>) -> Self {
        todo!()
    }

    ///
    /// Takes over the socket to send a stream of raw JSON data
    ///
    /// JSON is read from the input stream until a '.' is encountered (at the top level), or an error is encountered (at any point).
    ///
    pub fn stream_json<'a>(&'a mut self, json_stream: impl 'a + Send + Stream<Item=serde_json::Value>) -> impl 'a + Send + Stream<Item=serde_json::Value> {
        use std::mem;

        // Fetch the streams for the JSON
        // TODO: if the buffer is not emptied, then we need to preserve it in the input stream on return
        let mut buffer      = vec![];
        mem::swap(&mut buffer, &mut self.buffer);
        let input_stream    = &mut self.input_stream;
        let input_stream    = stream::iter(iter::once(buffer)).chain(input_stream.map(|CommandData(data)| data));
        let output_stream   = &mut self.output_stream;

        todo!();
        stream::empty()
    }

    ///
    /// Takes over the socket to send raw u8 data
    ///
    /// This allows commands to perform an interactive session with a user, directly interacting with their connection. It's up to the command when
    /// to close the streams: we will stop listening for raw data from the other side when the returned input stream is closed
    ///
    pub fn stream_raw<'a>(&'a mut self, raw_output_stream: impl 'a + Send + Stream<Item=Vec<u8>>) -> impl 'a + Unpin + Send + Stream<Item=Vec<u8>> {
        use std::mem;

        // Fetch the input and output streams
        // TODO: if the buffer is not emptied, then we need to preserve it in the input stream on return
        let mut buffer      = vec![];
        mem::swap(&mut buffer, &mut self.buffer);
        let input_stream    = &mut self.input_stream;
        let output_stream   = &mut self.output_stream;

        // Create a future to send the data from the raw stream to the output
        let send_output_future = async move {
            pin_mut!(raw_output_stream);

            // Read output...
            while let Some(data) = raw_output_stream.next().await {
                // ... send to the output stream
                let send_result = output_stream.send(CommandData(data)).await;
                if send_result.is_err() {
                    // Stop early if there is no more output to send from the raw stream
                    break;
                }
            }
        };

        // The returned stream reads from the input or the buffer
        let mut send_output_future = Some(Box::pin(send_output_future));

        let raw_stream = stream::poll_fn(move |context| {
            // Poll the output future so any generated output is sent
            // TODO: this might cause a blockage if the input is not polled when something is trying to send to the output stream
            if let Some(future) = &mut send_output_future {
                match future.poll_unpin(context) {
                    Poll::Ready(()) => { send_output_future = None; }
                    Poll::Pending   => { }
                }
            }

            // Check for any activity on the input stream
            if !buffer.is_empty() {
                // If there are buffered bytes, then return those as the very first piece of input
                let mut ready_buffer = vec![];
                mem::swap(&mut buffer, &mut ready_buffer);

                Poll::Ready(Some(ready_buffer))
            } else if let Poll::Ready(data) = input_stream.poll_next_unpin(context) {
                // Data is ready on the input stream
                Poll::Ready(data.map(|CommandData(data)| data))
            } else {
                // No data is ready
                Poll::Pending
            }
        });

        // Result is the stream from the future
        raw_stream
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
    /// Sends responses from a command
    ///
    pub async fn send_responses(&mut self, responses: impl Stream<Item=CommandResponse>) -> Result<(), ()> {
        todo!()
    }
}
