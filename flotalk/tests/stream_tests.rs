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
        runtime.set_root_symbol_value("Object", object.leak()).await;

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
        assert!(*new_x_value == TalkValue::Int(42));
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

#[test]
fn receive_one_message() {
    executor::block_on(async {
        let runtime = TalkRuntime::empty();
        
        // Create a source stream, which is itself a script
        let source_stream = stream::iter(vec![
            TalkMessage::with_arguments(vec![("add:", 200)]),
            TalkMessage::with_arguments(vec![("sub:", 180)]),
            TalkMessage::with_arguments(vec![("add:", 22)]),
        ]);

        // Create a receiver object (in the root namespace for now)
        runtime.run(TalkContinuation::soon(|context| {
            let receiver = create_talk_receiver(source_stream, context).leak();
            context.set_root_symbol_value("receiver", TalkValue::Reference(receiver));

            ().into()
        })).await;

        // Run a script that updates a variable based on what the source stream says
        let script_result = runtime.run(TalkScript::from("
            receiver next
            ")).await;

        // 200 - 180 + 22 = 42, indicating we received the messages we were expecting in our script
        println!("{:?}", script_result);
        assert!(*script_result == TalkValue::Message(Box::new(TalkMessage::with_arguments(vec![("add:", 200)]))));
    });
}

#[test]
fn receive_two_messages() {
    executor::block_on(async {
        let runtime = TalkRuntime::empty();
        
        // Create a source stream, which is itself a script
        let source_stream = stream::iter(vec![
            TalkMessage::with_arguments(vec![("add:", 200)]),
            TalkMessage::with_arguments(vec![("sub:", 180)]),
            TalkMessage::with_arguments(vec![("add:", 22)]),
        ]);

        // Create a receiver object (in the root namespace for now)
        runtime.run(TalkContinuation::soon(|context| {
            let receiver = create_talk_receiver(source_stream, context).leak();
            context.set_root_symbol_value("receiver", TalkValue::Reference(receiver));

            ().into()
        })).await;

        // Run a script that updates a variable based on what the source stream says
        let script_result = runtime.run(TalkScript::from("
            receiver next.
            receiver next
            ")).await;

        // 200 - 180 + 22 = 42, indicating we received the messages we were expecting in our script
        println!("{:?}", script_result);
        assert!(*script_result == TalkValue::Message(Box::new(TalkMessage::with_arguments(vec![("sub:", 180)]))));
    });
}

#[test]
fn stream_messages() {
    executor::block_on(async {
        let runtime = TalkRuntime::empty();
        
        // Create a source stream, which is itself a script
        let mut source_stream = runtime.stream_from::<TalkMessage>(TalkScript::from("
            [ :output | 
                output add: 200 . 
                output sub: 180 .
                output add: 22 .
            ]"));

        let first = source_stream.next().await;
        println!("{:?}", first);
        assert!(first == Some(Ok(TalkMessage::with_arguments(vec![("add:", 200)]))));

        let second = source_stream.next().await;
        println!("{:?}", second);
        assert!(second == Some(Ok(TalkMessage::with_arguments(vec![("sub:", 180)]))));

        let third = source_stream.next().await;
        println!("{:?}", third);
        assert!(third == Some(Ok(TalkMessage::with_arguments(vec![("add:", 22)]))));
    })
}

#[test]
fn stream_through_receiver() {
    executor::block_on(async {
        let runtime = TalkRuntime::empty();
        
        // Create a source stream, which is itself a script
        let source_stream = runtime.stream_from::<TalkMessage>(TalkScript::from("
            [ :output | 
                output add: 200 . 
                output sub: 180 .
                output add: 22 .
            ]"));

        // Create a receiver object (in the root namespace for now)
        runtime.run(TalkContinuation::soon(|context| {
            let receiver = create_talk_receiver(source_stream.map(|val| val.unwrap()), context).leak();
            context.set_root_symbol_value("receiver", TalkValue::Reference(receiver));

            ().into()
        })).await;

        // Run a script that updates a variable based on what the source stream says
        let script_result = runtime.run(TalkScript::from("
            | x nextMessage |

            x := 0 .

            [
                nextMessage ifMatches: #add: do: [ :value | x := x + value ].
                nextMessage ifMatches: #sub: do: [ :value | x := x - value ].
            ] while: [
                nextMessage := receiver next.
                ^(nextMessage isNil) not
            ].

            x
            ")).await;

        // 200 - 180 + 22 = 42, indicating we received the messages we were expecting in our script
        println!("{:?}", script_result);
        assert!(*script_result == TalkValue::Int(42));
    });
}
