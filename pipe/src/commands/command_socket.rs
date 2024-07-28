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
use std::sync::*;

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
    /// The `activity_fn` is called back to perform whatever activity is needed on the stream: control is returned once this function has completed.
    ///
    pub async fn stream_raw<'a, TFuture>(&'a mut self, activity_fn: impl 'a + FnOnce(BoxStream<'a, Vec<u8>>, mpsc::Sender<Vec<u8>>) -> TFuture) -> TFuture::Output
    where
        TFuture: 'a + Future,
    {
        use std::mem;

        // Fetch the input and output streams
        // TODO: if the buffer is not emptied, then we need to preserve it in the input stream on return
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
