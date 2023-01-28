use crate::*;

///
/// FloTalk's puttableStream protocol
///
// #[derive(TalkMessageType)]
pub enum TalkPuttableStream {
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
pub enum TalkSimpleStream {
    /// Writes a string to the stream
    Write(String),
}