use crate::parser::*;
use crate::socket::*;
use crate::commands::command_program::*;
use crate::commands::command_stream::*;
use crate::commands::parse_command::*;

use futures::prelude::*;
use futures::stream::{BoxStream};

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
    output_stream: BoxStream<'static, CommandData>,
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
    /// The returned stream is the JSON data sent from the other side. We close it when a '.' is sent, but control is not returned until the `json_stream`
    /// stream is returned.
    ///
    pub async fn stream_json<'a>(&'a mut self, json_stream: impl 'a + Send + Stream<Item=serde_json::Value>) -> impl 'a + Send + Stream<Item=serde_json::Value> {
        todo!();
        stream::empty()
    }

    ///
    /// Takes over the socket to send raw u8 data
    ///
    /// This allows commands to perform an interactive session with a user, directly interacting with their connection. It's up to the command when
    /// to close the streams: we will stop listening for raw data from the other side when 
    ///
    pub async fn stream_raw<'a>(&'a mut self, raw_stream: impl 'a + Send + Stream<Item=Vec<u8>>) -> impl 'a + Send + Stream<Item=Vec<u8>> {
        todo!();
        stream::empty()
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
        // let buffer  = tokenizer.to_u8_lookahead();
        // self.buffer = buffer;

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
}
