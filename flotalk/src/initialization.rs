use crate::continuation::*;
use crate::message::*;
use crate::standard_classes::*;
use crate::script_continuation::*;
use crate::value::*;

///
/// Returns a continuation that will load the standard set of classes into 
///
pub fn talk_init_standard_classes() -> TalkContinuation<'static> {
    talk_init_object_class()
        .and_then_if_ok(|_| talk_init_inverted_class())
        .and_then_if_ok(|_| talk_init_stream_class())
        .and_then_if_ok(|_| talk_init_stream_with_reply_class())
        .and_then_if_ok(|_| talk_init_later_class())
        .and_then_if_ok(|_| talk_init_streaming_class())
        .and_then_if_ok(|_| talk_init_streaming_with_reply_class())
        .and_then_if_ok(|_| talk_init_evaluate_class())
        .and_then_if_ok(|_| talk_init_dictionary_class())
        .and_then_if_ok(|_| talk_init_constants())
        .and_then_if_ok(|_| talk_init_modules())
        .panic_on_error("While initializing")
}

///
/// Returns a continuation that will create the 'Object' class definition
///
pub fn talk_init_object_class() -> TalkContinuation<'static> {
    TalkContinuation::soon(|talk_context| {
        SCRIPT_CLASS_CLASS.send_message_in_context(TalkMessage::unary("new"), talk_context)
    }).define_as("Object")
}

///
/// Returns a continuation that will create the 'Inverted' class definition
///
pub fn talk_init_inverted_class() -> TalkContinuation<'static> {
    TalkContinuation::soon(|talk_context| {
        let inverted_class_object   = INVERTED_CLASS.class_object_in_context(talk_context);
        let all_value               = (*INVERTED_ALL).clone();

        talk_context.set_root_symbol_value("Inverted", inverted_class_object.into());
        talk_context.set_root_symbol_value("all", all_value);
        ().into()
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
/// Returns a continuation that will create the 'StreamWithReply' class definition
///
pub fn talk_init_stream_with_reply_class() -> TalkContinuation<'static> {
    TalkContinuation::soon(|talk_context| {
        let stream_class_object = STREAM_WITH_REPLY_CLASS.class_object_in_context(talk_context);
        talk_context.set_root_symbol_value("StreamWithReply", stream_class_object.into());
        ().into()
    })
}

///
/// Returns a continuation that will create the 'Stream' class definition
///
pub fn talk_init_later_class() -> TalkContinuation<'static> {
    TalkContinuation::soon(|talk_context| LATER_CLASS.class_object_in_context(talk_context).into())
        .define_as("Later")
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
    TalkContinuation::soon(|_talk_context| {
        // Subclass 'Stream' to make the 'Streaming' class
        TalkScript::from("Stream subclass").into()
    }).define_as("Streaming").and_then_if_ok(|_| {
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

///
/// Returns a continuation that will create the 'StreamingWithReply' class definition
///
/// Requires 'StreamWithReply' to already be initialised.
///
/// `StreamingWithReply` is a base class that can be used to create objects that handle messages as a stream rather
/// usual instance messages. It 
///
pub fn talk_init_streaming_with_reply_class() -> TalkContinuation<'static> {
    TalkContinuation::soon(|_talk_context| {
        // Subclass 'Stream' to make the 'Streaming' class
        TalkScript::from("StreamWithReply subclass").into()
    }).define_as("StreamingWithReply").and_then_if_ok(|_| {
        // Define the class methods
        TalkScript::from("
            | OriginalStreamingWithReply |
            OriginalStreamingWithReply := StreamingWithReply.

            StreamingWithReply addClassMessage: #subclass: withAction: [ :streamBlock |
                | subclass |

                subclass := OriginalStreamingWithReply subclass.
                subclass addClassMessage: #newSuperclass withAction: [
                    | stream |
                    stream := OriginalStreamingWithReply withReceiver: streamBlock.

                    stream
                ].

                ^subclass
            ].

            StreamingWithReply addClassMessage: #supportMessage: withAction: [ :message | 
            ].
        ").into()
    })
}

///
/// Returns a continuation that will create the 'Evaluate' class definition
///
pub fn talk_init_evaluate_class() -> TalkContinuation<'static> {
    EVALUATE_CLASS.class_object()
        .define_as("Evaluate")
}

pub fn talk_init_dictionary_class() -> TalkContinuation<'static> {
    DICTIONARY_CLASS.class_object()
        .define_as("Dictionary")
}

///
/// Returns a continuation that will initialize the standard set of constant values
///
/// These are:
///
/// * `nil` - the 'nil' value
///
pub fn talk_init_constants() -> TalkContinuation<'static> {
    TalkContinuation::soon(|talk_context| {
        talk_context.set_root_symbol_value("nil", TalkValue::Nil);
        talk_context.set_root_symbol_value("true", TalkValue::Bool(true));
        talk_context.set_root_symbol_value("false", TalkValue::Bool(false));
        ().into()
    })
}

///
/// Returns a continuation that will initialise the module system (the Import and Export classes)
///
pub fn talk_init_modules() -> TalkContinuation<'static> {
    IMPORT_CLASS.class_object()
        .define_as("Import")
}
