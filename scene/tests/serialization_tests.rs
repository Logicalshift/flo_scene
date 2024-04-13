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
        let json_serializer_filter = serializer_filter::<TestMessage, _, _>(|| serde_json::value::Serializer, |stream| stream);
        scene.connect_programs((), StreamTarget::Filtered(json_serializer_filter, serialized_resender), StreamId::with_message_type::<TestMessage>()).unwrap();

        // Create a deserializer to use with the test program
        let json_deserializer_filter = deserializer_filter::<TestMessage, serde_json::value::Value, _>(|stream| stream);
        scene.connect_programs((), StreamTarget::Filtered(json_deserializer_filter, deserialized_receiver), StreamId::with_message_type::<SerializedMessage<serde_json::value::Value>>()).unwrap();

        // Run some tests with a message that gets serialized and deserialized
        TestBuilder::new()
            .send_message_to_target(serialized_resender, TestMessage::StringValue(format!("Test")))
            .expect_message(|msg: TestMessage| {
                if msg != TestMessage::StringValue(format!("Test")) { Err(format!("Expected 'Test' (got {:?})", msg)) } else { Ok(()) }
            })
            .run_in_scene(&scene, test_program);
    }
}
