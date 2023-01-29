use crate::*;

use smallvec::*;
use futures::prelude::*;

use std::sync::*;

///
/// FloTalk's puttableStream protocol
///
#[derive(Debug, TalkMessageType, PartialEq)]
pub enum TalkPuttableStreamRequest {
    /// Writes a carriage return sequence to the stream
    Cr,

    /// Flushes the stream's backing store
    Flush,

    /// Writes the value of an object to the stream
    #[message("nextPut:")]
    NextPut(TalkValue),

    /// Writes all of the values in a collection to the stream
    #[message("nextPutAll:")]
    NextPutAll(TalkValue),

    /// Writes a space to the stream
    Space,

    /// Writes a tab character to the stream
    Tab,
}

///
/// FlotTalk's simple stream protocol
///
#[derive(Debug, TalkMessageType, PartialEq)]
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
    use futures::stream;

    // Create a 'puttable' stream and pass it to the block created by the receive_stream continuation
    let (create_sender_continuation, stream) = create_talk_sender::<TalkPuttableStreamRequest>();

    // Every value needs to be properly released when done, and we also need to evaluate the characters in the sequence passed to NextPutAll, so we need a stream processing continuation
    let (simple_stream, simple_continuation) = talk_pipe_stream(stream, |request, talk_context| {
        use TalkPuttableStreamRequest::*;

        match request {
            Flush                               => TalkSimpleStreamRequest::Write("".into()).into_talk_value(talk_context).leak().into(),
            Cr                                  => TalkSimpleStreamRequest::Write("\n".into()).into_talk_value(talk_context).leak().into(),
            Space                               => TalkSimpleStreamRequest::Write(" ".into()).into_talk_value(talk_context).leak().into(),
            Tab                                 => TalkSimpleStreamRequest::Write("\t".into()).into_talk_value(talk_context).leak().into(),
            NextPut(TalkValue::Character(chr))  => TalkSimpleStreamRequest::Write(chr.into()).into_talk_value(talk_context).leak().into(),
            NextPutAll(sequence_val)            => {
                // A place to gather the string in
                let string      = Arc::new(Mutex::new(String::default()));

                // Create the block function to pass to the 'do:' message
                let string_copy = Arc::clone(&string);
                let fill_string = talk_fn_block_in_context(move |next_chr: TalkValue /* TODO: use an owned value? Will leak otherwise */| {
                    let chr = match next_chr {
                        TalkValue::Character(chr)   => chr,
                        _                           => '?'
                    };

                    string_copy.lock().unwrap().push(chr);
                }, talk_context);

                // Call 'do:' on the target with the block function we just created, then send the string once the iteration completes
                sequence_val.send_message_in_context(TalkMessage::WithArguments(*TALK_MSG_DO, smallvec![fill_string.leak().into()]), talk_context)
                    .and_then_soon(move |_, talk_context| {
                        // String has been generated: send it to the stream
                        let string = string.lock().unwrap().clone();
                        TalkSimpleStreamRequest::Write(string).into_talk_value(talk_context).leak().into()
                    })
            },

            NextPut(other)                      => {
                other.release_in_context(talk_context);
                TalkSimpleStreamRequest::Write("?".into()).into_talk_value(talk_context).leak().into()
            }

        }
    });

    let simple_stream = simple_stream.flat_map(|val| stream::iter(val.ok()));

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
