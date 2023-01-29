use crate::*;

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

/*
///
/// This creates a TalkPuttableStream and passes it to the result of the receive_stream continuation
///
/// This is the type of stream that SmallTalk uses for the 'transcript' object, and forms the basis of the output mechanism for FloTalk
///
pub fn talk_puttable_character_stream(receive_stream: impl Into<TalkContinuation<'static>>) -> (TalkContinuation<'static>, impl Stream<Item = TalkSimpleStreamRequest>) {
    // Create a 'puttable' stream and pass it to the block created by the receive_stream continuation
    let (stream, continuation) = create_talk_stream::<TalkPuttableStreamRequest>(receive_stream);

    // Every value needs to be properly released when done, and we also need to evaluate the characters in the sequence passed to NextPutAll, so we need a stream processing continuation

    todo!()
}
*/