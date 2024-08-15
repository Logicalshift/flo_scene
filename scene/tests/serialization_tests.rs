#[cfg(feature = "serde_support")]
mod with_serde_support {
    use flo_scene::*;
    use flo_scene::programs::*;

    use futures::prelude::*;

    use serde::*;
    use serde_json;

    #[derive(Serialize, Deserialize, PartialEq, Eq, Debug)]
    enum TestMessage {
        StringValue(String)
    }

    impl SceneMessage for TestMessage { }

    #[test]
    fn serialize_deserialize() {
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
        json_serializer_filter.into_iter()
            .for_each(|filter|
                { scene.connect_programs((), StreamTarget::Filtered(filter, serialized_resender), filter.source_stream_id_any().unwrap()).ok(); });

        // Create a deserializer to use with the test program
        let json_deserializer_filter = serializer_filter::<SerializedMessage<serde_json::Value>, TestMessage>().unwrap();
        json_deserializer_filter.into_iter()
            .for_each(|filter|
                { scene.connect_programs((), StreamTarget::Filtered(filter, deserialized_receiver), filter.source_stream_id_any().unwrap()).ok(); });

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
        // Create a scene that will serialize and deserialize the message
        let scene = Scene::default();
        scene.with_serializer(|| serde_json::value::Serializer)
            .with_serializable_type::<TestMessage>("flo_scene::TestMessage");

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
    fn query_response_serializer() {
        // Create a scene that will serialize and deserialize the message
        let scene = Scene::default();
        scene.with_serializer(|| serde_json::value::Serializer)
            .with_serializable_type::<TestMessage>("flo_scene::TestMessage");

        // Add a serialized_resender program that sends whatever serialized message it gets to the test program
        let test_program            = SubProgramId::new();
        let serialized_resender     = SubProgramId::new();
        let deserialized_receiver   = SubProgramId::new();

        scene.add_subprogram(serialized_resender, 
            move |input_stream, context| async move {
                let mut input_stream = input_stream;

                while let Some(message) = input_stream.next().await {
                    let mut response: QueryResponse<SerializedMessage<serde_json::Value>> = message;

                    while let Some(message) = response.next().await {
                        println!("Serialized: {:?}", message.0);

                        context.send(deserialized_receiver).unwrap()
                            .send(message)
                            .await
                            .unwrap();
                    }
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
            .send_message_to_target(serialized_resender, QueryResponse::with_data(TestMessage::StringValue(format!("Test"))))
            .expect_message(|msg: TestMessage| {
                if msg != TestMessage::StringValue(format!("Test")) { Err(format!("Expected 'Test' (got {:?})", msg)) } else { Ok(()) }
            })
            .run_in_scene(&scene, test_program);
    }

    #[test]
    fn query_response_deserializer() {
        // Create a scene that will serialize a message, then deserialize it via a query response deserializer
        let scene = Scene::default();
        scene.with_serializer(|| serde_json::value::Serializer)
            .with_serializable_type::<TestMessage>("flo_scene::TestMessage");

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
                        .send(QueryResponse::with_data(message))
                        .await
                        .map_err(|_| ())
                        .unwrap();
                }
            }, 0);

        // The deserialized receiver takes TestMessages and passes them back to the test program
        scene.add_subprogram(deserialized_receiver, move |input_stream, context| async move {
            let mut input_stream = input_stream;

            while let Some(message) = input_stream.next().await {
                let mut response: QueryResponse<TestMessage> = message;

                while let Some(message) = response.next().await {
                    println!("Deserialized: {:?}", message);

                    context.send(test_program).unwrap()
                        .send(message)
                        .await
                        .unwrap();
                }
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
}
