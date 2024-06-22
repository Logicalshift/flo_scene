#[cfg(feature = "serde_support")]
mod with_serde_support {
    use flo_scene::*;
    use flo_scene::programs::*;

    use futures::prelude::*;

    use serde::*;
    use serde_json;

    #[test]
    fn serialize_deserialize() {
        #[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
        enum TestMessage {
            StringValue(String)
        }

        impl SceneMessage for TestMessage { }

        let scene = Scene::default();

        let test_program            = SubProgramId::new();
        let serialized_resender     = SubProgramId::new();
        let deserialized_receiver   = SubProgramId::new();

        install_serializer(|| serde_json::value::Serializer);
        install_serializable_type::<TestMessage, serde_json::value::Serializer>("flo_scene::TestMessage").unwrap();

        // Add a serialized_resender program that sends whatever serialized message it gets to the test program
        scene.add_subprogram(serialized_resender, 
            move |input_stream, context| async move {
                let mut input_stream = input_stream;

                while let Some(message) = input_stream.next().await {
                    let message: SerializedMessage<serde_json::Value> = message;

                    println!("Serialized: {:?}", message.0);

                    context.send(deserialized_receiver).unwrap()
                        .send(message)
                        .await
                        .unwrap();
                }
            }, 0);

        // The deserialized receiver takes TestMessages and passes them back to the test program
        scene.add_subprogram(deserialized_receiver, move |input_stream, context| async move {
            let mut input_stream = input_stream;

            while let Some(message) = input_stream.next().await {
                let message: TestMessage = message;

                println!("Deserialized: {:?}", message);

                context.send(test_program).unwrap()
                    .send(message)
                    .await
                    .unwrap();
            }
        }, 0);

        // Create a JSON serializer to allow test messages to be sent directly to the serialized_resender program
        let json_serializer_filter = serializer_filter::<TestMessage, SerializedMessage<serde_json::Value>>().unwrap();
        scene.connect_programs((), StreamTarget::Filtered(json_serializer_filter, serialized_resender), StreamId::with_message_type::<TestMessage>()).unwrap();

        // Create a deserializer to use with the test program
        let json_deserializer_filter = serializer_filter::<SerializedMessage<serde_json::Value>, TestMessage>().unwrap();
        scene.connect_programs((), StreamTarget::Filtered(json_deserializer_filter, deserialized_receiver), StreamId::with_message_type::<SerializedMessage<serde_json::value::Value>>()).unwrap();

        // Run some tests with a message that gets serialized and deserialized
        TestBuilder::new()
            .send_message_to_target(serialized_resender, TestMessage::StringValue(format!("Test")))
            .expect_message(|msg: TestMessage| {
                if msg != TestMessage::StringValue(format!("Test")) { Err(format!("Expected 'Test' (got {:?})", msg)) } else { Ok(()) }
            })
            .run_in_scene(&scene, test_program);
    }

    #[test]
    fn install_basic_serializer() {
        #[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
        enum TestMessage {
            StringValue(String)
        }

        impl SceneMessage for TestMessage { }

        install_serializer(|| serde_json::value::Serializer);
        install_serializable_type::<TestMessage, serde_json::value::Serializer>("flo_scene::TestMessage").unwrap();

        // Create a scene that will serialize and deserialize the message
        let scene = Scene::default();
        install_serializers::<TestMessage, _>(&scene, "flo_scene::TestMessage", || serde_json::value::Serializer).unwrap();

        // Add a serialized_resender program that sends whatever serialized message it gets to the test program
        let test_program            = SubProgramId::new();
        let serialized_resender     = SubProgramId::new();
        let deserialized_receiver   = SubProgramId::new();

        scene.add_subprogram(serialized_resender, 
            move |input_stream, context| async move {
                let mut input_stream = input_stream;

                while let Some(message) = input_stream.next().await {
                    let message: SerializedMessage<serde_json::Value> = message;

                    println!("Serialized: {:?}", message.0);

                    context.send(deserialized_receiver).unwrap()
                        .send(message)
                        .await
                        .unwrap();
                }
            }, 0);

        // The deserialized receiver takes TestMessages and passes them back to the test program
        scene.add_subprogram(deserialized_receiver, move |input_stream, context| async move {
            let mut input_stream = input_stream;

            while let Some(message) = input_stream.next().await {
                let message: TestMessage = message;

                println!("Deserialized: {:?}", message);

                context.send(test_program).unwrap()
                    .send(message)
                    .await
                    .unwrap();
            }
        }, 0);

        // Run some tests with a message that gets serialized and deserialized
        TestBuilder::new()
            .send_message_to_target(serialized_resender, TestMessage::StringValue(format!("Test")))
            .expect_message(|msg: TestMessage| {
                if msg != TestMessage::StringValue(format!("Test")) { Err(format!("Expected 'Test' (got {:?})", msg)) } else { Ok(()) }
            })
            .run_in_scene(&scene, test_program);
    }

    #[test]
    fn send_serialized_messages() {
        #[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
        enum TestMessage2 {
            StringValue(String)
        }

        impl SceneMessage for TestMessage2 { }

        // Create a scene that will serialize and deserialize the message
        let scene           = Scene::default();
        let test_program    = SubProgramId::new();
        install_serializers::<TestMessage2, _>(&scene, "flo_scene::TestMessage2", || serde_json::value::Serializer).unwrap();

        // Add a subprogram that sends some serialized messages to the test program
        let send_program = SubProgramId::new();
        scene.add_subprogram(send_program, move |_: InputStream<()>, context| async move {
            // Create a sink to send serialized messages to
            let mut send_sink   = context.send_serialized::<serde_json::Value>("flo_scene::TestMessage2", test_program).unwrap();

            // Send some serialized messages via the sink (which get turned straight back into TestMessage2)
            send_sink.send(TestMessage2::StringValue("Hello".to_string()).serialize(serde_json::value::Serializer).unwrap()).await.unwrap();
            send_sink.send(TestMessage2::StringValue("Goodbye".to_string()).serialize(serde_json::value::Serializer).unwrap()).await.unwrap();
        }, 0);

        TestBuilder::new()
            .expect_message(|msg: TestMessage2| {
                if msg != TestMessage2::StringValue(format!("Hello")) { Err(format!("Expected 'Hello' (got {:?})", msg)) } else { Ok(()) }
            }).expect_message(|msg: TestMessage2| {
                if msg != TestMessage2::StringValue(format!("Goodbye")) { Err(format!("Expected 'Goodbye' (got {:?})", msg)) } else { Ok(()) }
            })
            .run_in_scene(&scene, test_program);
    }
}
