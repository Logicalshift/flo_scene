use super::location::*;

use futures::prelude::*;
use futures::stream;
use futures::task::{Poll, Context};

use std::pin::*;

///
/// Stream that keeps track of locations, and also allows characters to be pushed back
///
pub struct PushBackStream<TStream> 
where
    TStream: Stream<Item=char>
{
    /// The source of characters to be supplied to the parser
    source_stream: Option<TStream>,

    /// Characters that have been pushed back to the stream stack
    pushback_stack: Vec<char>,

    /// Current location in the stream
    location: TalkLocation,
}

impl<TStream> PushBackStream<TStream>
where
    TStream: Unpin + Stream<Item=char>
{
    pub fn new(stream: TStream) -> PushBackStream<TStream> {
        PushBackStream {
            source_stream:  Some(stream),
            pushback_stack: vec![],
            location:       TalkLocation::default(),
        }
    }

    ///
    /// Pushes a character back onto the stream so that it will be returned by the next poll
    ///
    pub fn pushback(&mut self, c: char) {
        self.pushback_stack.push(c);
        self.location = self.location.pushback();
    }

    ///
    /// Retrieves the location after the current character
    ///
    pub fn location(&self) -> TalkLocation {
        self.location
    }

    ///
    /// Returns the next character without removing it from the stream
    ///
    pub async fn peek(&mut self) -> Option<char> {
        let next_char = self.next().await;

        if let Some(next_char) = next_char {
            self.pushback(next_char);
        }

        next_char
    }
}

impl<TStream> Stream for PushBackStream<TStream> 
where
    TStream: Unpin + Stream<Item=char>
{
    type Item = char;

    fn poll_next(mut self: Pin<&mut Self>, context: &mut Context) -> Poll<Option<Self::Item>> {
        if let Some(pushed_back) = self.pushback_stack.pop() {

            // Some characters have been pushed back
            self.location = self.location.after_character(pushed_back);
            Poll::Ready(Some(pushed_back))

        } else if let Some(source_stream) = &mut self.source_stream {

            // Source stream is still alive, and there are no pushed back characters
            let next_result = source_stream.poll_next_unpin(context);
            
            match &next_result {
                Poll::Pending                   => { }                              // Waiting for the next character
                Poll::Ready(None)               => { self.source_stream = None; }   // Source stream is exhausted
                Poll::Ready(Some(next_char))    => { self.location = self.location.after_character(*next_char); }
            }

            next_result

        } else {

            // Source stream is dead, and there are no more characters
            Poll::Ready(None)

        }
    }
}
