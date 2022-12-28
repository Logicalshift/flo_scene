use crate::continuation::*;
use crate::message::*;
use crate::standard_classes::*;

///
/// Returns a continuation that will load the standard set of classes into 
///
pub fn talk_init_standard_classes() -> TalkContinuation<'static> {
    talk_init_object_class()
        .and_then(|_| talk_init_stream_class())
}

///
/// Returns a continuation that will create the 'Object' class definition
///
pub fn talk_init_object_class() -> TalkContinuation<'static> {
    TalkContinuation::soon(|talk_context| {
        SCRIPT_CLASS_CLASS.send_message_in_context(TalkMessage::unary("new"), talk_context)
    }) .and_then(|script_class| {
        TalkContinuation::soon(move |talk_context| {
            talk_context.set_root_symbol_value("Object", script_class);
            ().into()
        })
    })
}

///
/// Returns a continuation that will create the 'Stream' class definition
///
pub fn talk_init_stream_class() -> TalkContinuation<'static> {
    TalkContinuation::soon(|talk_context| {
        talk_context.set_root_symbol_value("Stream", STREAM_CLASS.class_object_in_context(talk_context).into());
        ().into()
    })
}
