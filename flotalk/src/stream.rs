use crate::continuation::*;
use crate::message::*;
use crate::standard_classes::*;

use smallvec::*;
use once_cell::sync::{Lazy};
use futures::prelude::*;

///
/// Creates a sender stream and sends it as a message to the result of the `receive_stream` continuation, returning a stream of the messages sent to
/// that stream, and a continuation to run
///
/// Essentially, the continuation that's passed in should return a block object that takes a single parameter. This function can be used like this:
///
/// ```ignore
/// # // Can't compile this as cargo will say the crate is 'flo_talk' when it's not, which breaks the macro
/// # #[macro_use] extern crate flo_talk_macros;
/// # use flo_talk::*;
/// #[derive(TalkMessageType)]
/// enum HelloWorld { #[message("helloWorld")] Hello, #[message("goodbye")] Goodbye }
///
/// let (mut hello_world_stream, continuation) = talk_stream_from::<HelloWorld>(TalkScript::from("[ :output | output helloWorld. output goodbye. ]"));
/// ```
///
/// The continuation must be run on a runtime before anything can be retrieved from the stream: the function `TalkRuntime::stream_from()` can be used
/// to create a stream that will automatically execute its code in parallel and report any errors.
///
pub fn talk_stream_from<TStreamItem>(receive_sender_stream: impl Into<TalkContinuation<'static>>) -> (impl 'static + Send + Stream<Item=TStreamItem>, TalkContinuation<'static>) 
where
    TStreamItem: 'static + Send + TalkMessageType,
{
    static VALUE_COLON_MSG: Lazy<TalkMessageSignatureId>  = Lazy::new(|| "value:".into());

    // Convert receive_stream into a continuation
    let receive_sender_stream = receive_sender_stream.into();

    // Create the sender stream continuation
    let (sender_stream, receiver) = create_talk_sender::<TStreamItem>();

    // Create a continuation that streams from the receiver channel to the sender channel
    let run_stream = receive_sender_stream.and_then_if_ok(move |receive_sender_stream| {
        sender_stream.and_then_soon_if_ok(move |sender_stream, talk_context| {
            receive_sender_stream.send_message_in_context(TalkMessage::WithArguments(*VALUE_COLON_MSG, smallvec![sender_stream]), talk_context)
        })
    });

    (receiver, run_stream)
}
