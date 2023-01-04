use crate::continuation::*;
use crate::message::*;
use crate::standard_classes::*;
use crate::script_continuation::*;

///
/// Returns a continuation that will load the standard set of classes into 
///
pub fn talk_init_standard_classes() -> TalkContinuation<'static> {
    talk_init_object_class()
        .and_then_if_ok(|_| talk_init_stream_class())
        .and_then_if_ok(|_| talk_init_later_class())
        .and_then_if_ok(|_| talk_init_streaming_class())
        .panic_on_error("While initializing")
}

///
/// Returns a continuation that will create the 'Object' class definition
///
pub fn talk_init_object_class() -> TalkContinuation<'static> {
    TalkContinuation::soon(|talk_context| {
        SCRIPT_CLASS_CLASS.send_message_in_context(TalkMessage::unary("new"), talk_context)
    }).and_then(|script_class| {
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
        let stream_class_object = STREAM_CLASS.class_object_in_context(talk_context);
        talk_context.set_root_symbol_value("Stream", stream_class_object.into());
        ().into()
    })
}

///
/// Returns a continuation that will create the 'Stream' class definition
///
pub fn talk_init_later_class() -> TalkContinuation<'static> {
    TalkContinuation::soon(|talk_context| {
        let later_class_object = LATER_CLASS.class_object_in_context(talk_context);
        talk_context.set_root_symbol_value("Later", later_class_object.into());
        ().into()
    })
}

///
/// Returns a continuation that will create the 'Streaming' class definition
///
/// Requires 'Stream' to already be initialised.
///
/// `Streaming` is a base class that can be used to create objects that handle messages as a stream rather
/// usual instance messages.
///
pub fn talk_init_streaming_class() -> TalkContinuation<'static> {
    TalkContinuation::soon(|talk_context| {
        // Subclass 'Stream' to make the 'Streaming' class
        TalkScript::from("Stream subclass").into()
    }).and_then_if_ok(|streaming_class| {
        // Store in the 'Streaming' variable
        TalkContinuation::soon(move |talk_context| {
            talk_context.set_root_symbol_value("Streaming", streaming_class);
            ().into()
        })
    }).and_then_if_ok(|_| {
        // Define the class methods
        TalkScript::from("
            | OriginalStreaming |
            OriginalStreaming := Streaming.

            Streaming addClassMessage: #subclass: withAction: [ :streamBlock |
                | subclass |

                subclass := OriginalStreaming subclass.
                subclass addClassMessage: #newSuperclass withAction: [
                    | stream |
                    stream := OriginalStreaming withReceiver: streamBlock.

                    stream
                ].

                ^subclass
            ].

            Streaming addClassMessage: #supportMessage: withAction: [ :message | 
            ].
        ").into()
    })
}
