use crate::*;

use smallvec::*;
use futures::prelude::*;

///
/// FloTalk's puttableStream protocol
///
#[derive(TalkMessageType)]
pub enum TalkPuttableStreamRequest {
    /// Writes a carriage return sequence to the stream
    Cr,

    /// Flushes the stream's backing store
    Flush,

    /// Writes the value of an object to the stream
    NextPut(TalkValue),

    /// Writes all of the values in a collection to the stream
    NextPutAll(TalkValue),

    /// Writes a space to the stream
    Space,

    /// Writes a tab character to the stream
    Tab,
}

///
/// FlotTalk's simple stream protocol
///
pub enum TalkSimpleStreamRequest {
    /// Writes a string to the stream
    Write(String),
}

///
/// This creates a TalkPuttableStream and passes it to the result of the receive_stream continuation
///
/// This is the type of stream that SmallTalk uses for the 'transcript' object, and forms the basis of the output mechanism for FloTalk
///
pub fn talk_puttable_character_stream(receive_stream: impl Into<TalkContinuation<'static>>) -> (impl Stream<Item = TalkSimpleStreamRequest>, TalkContinuation<'static>) {
    // Create a 'puttable' stream and pass it to the block created by the receive_stream continuation
    let (create_sender_continuation, stream) = create_talk_sender::<TalkPuttableStreamRequest>();

    // Every value needs to be properly released when done, and we also need to evaluate the characters in the sequence passed to NextPutAll, so we need a stream processing continuation
    let (simple_stream, simple_continuation) = talk_map_stream(stream, |request, talk_context| {
        // TODO: we need to deal with put all by running another continuation :-/
        TalkSimpleStreamRequest::Write("".to_string())
    });

    // Put the simple continuation in the background
    let receive_stream  = receive_stream.into();
    let continuation    = TalkContinuation::soon(move |talk_context| {
        // Put the simple continuation into the background
        talk_context.run_in_background(simple_continuation);

        // Create the sender and then the receive stream block, and send the 'value:' message to it
        create_sender_continuation.and_then_if_ok(move |sender_value| {
            receive_stream.and_then_soon(move |receive_stream, talk_context| {
                receive_stream.send_message_in_context(TalkMessage::WithArguments(*TALK_MSG_VALUE_COLON, smallvec![sender_value]), talk_context)
            })
        })
    });

    (simple_stream, continuation)
}
