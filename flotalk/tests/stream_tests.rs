use flo_talk::*;

use futures::prelude::*;
use futures::executor;
use futures::stream;

#[test]
fn send_values_to_script_stream() {
    executor::block_on(async {
        #[derive(TalkMessageType)]
        pub enum Message {
            #[message("setValue:")]
            SetValue(i64),

            #[message("addValue:")]
            AddValue(i64),
        }

        // Create a runtime with a value in the root values
        let runtime = TalkRuntime::empty();
        runtime.set_root_symbol_value("x", 0).await;

        // Manually create the 'object' in this context (by sending 'new' to the script class class)
        let object = runtime.run(TalkContinuation::soon(|talk_context| {
            SCRIPT_CLASS_CLASS.send_message_in_context(TalkMessage::unary("new"), talk_context)
        })).await;
        runtime.set_root_symbol_value("Object", object).await;

        // Declare a class to handle the 'setValue' and 'addValue' messages and use that as the target of the stream
        let stream_values   = vec![Message::SetValue(26), Message::AddValue(16)];
        let stream_result   = runtime.stream_to(TalkScript::from("
            | TestClass |
            
            TestClass := Object subclass.
            TestClass addInstanceMessage: #setValue: withAction: [ :newVal | x := newVal. ].
            TestClass addInstanceMessage: #addValue: withAction: [ :addVal | x := x + addVal. ].

            TestClass new
            "), stream::iter(stream_values)).await;

        assert!(stream_result.is_ok());

        // 'x' should have the value '42' (26 + 16)
        let new_x_value = runtime.run(TalkScript::from("x")).await;
        assert!(new_x_value == TalkValue::Int(42));
    });
}

#[test]
fn receive_values_from_script_via_stream() {
    executor::block_on(async {
        let runtime = TalkRuntime::empty();
        #[derive(TalkMessageType, PartialEq)]
        enum HelloWorld { #[message("helloWorld")] Hello, #[message("say:")] Say(String), #[message("goodbye")] Goodbye }
        
        let mut hello_world = runtime.stream_from::<HelloWorld>(TalkScript::from("
            [ :output | 
                output helloWorld. 
                output say: 'Test'. 
                output goodbye. 
            ]"));

        assert!(hello_world.next().await == Some(Ok(HelloWorld::Hello)));
        assert!(hello_world.next().await == Some(Ok(HelloWorld::Say("Test".into()))));
        assert!(hello_world.next().await == Some(Ok(HelloWorld::Goodbye)));
        assert!(hello_world.next().await == None);
    });
}
