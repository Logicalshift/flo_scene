use flo_scene::*;
use flo_scene::guest::*;
use flo_scene::programs::*;

use futures::prelude::*;
use futures::future::{BoxFuture};

use serde::*;
use serde::de::{Error as DeError};
use serde::ser::{Error as SeError};

use std::sync::*;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SimpleTestMessage {
    value: String,
}

impl SceneMessage for SimpleTestMessage {
    fn message_type_name() -> String {
        "flo_scene_tests::function_tests::SimpleTestMessage".into()
    }
}

///
/// Test message containing a function (and an implementation that sends the function across the guest boundary)
///
#[derive(Clone)]
pub struct TestFunctionMessage(Arc<dyn Send + Sync + Fn(Vec<u8>) -> BoxFuture<'static, ()>>);

#[derive(Serialize, Deserialize)]
struct SerializedTestFunctionMessage(SerializationId);

impl SceneMessage for TestFunctionMessage {
    #[cfg(any(feature="postcard", target_family="wasm"))]
    #[inline]
    fn to_guest_message(self, context: &impl SerializationContext) -> Result<Vec<u8>, SceneSendError<Self>> {
        // Convert the function to one that works as a future
        let callback = self.0.clone();
        let callback = Box::new(move |args| {
            callback(args)
        });

        // Send as a serialized function to the other side of the connection
        let serialized_function = context.send_function(callback)
            .map_err(|err| err.map(|_| self.clone()))?;

        // Serialize as a SerializedTestFunctionMessage
        let serialized_function = SerializedTestFunctionMessage(serialized_function);
        let serialized_function = postcard::to_stdvec(&serialized_function)
            .map_err(|err| SceneSendError::CannotSerialize(TestFunctionMessage(self.0.clone()), format!("{:?}", err)))?;

        // Created a guest message
        Ok(serialized_function)
    }

    #[cfg(any(feature="postcard", target_family="wasm"))]
    #[inline]
    fn from_guest_message(value: &Vec<u8>, context: &impl SerializationContext) -> Result<Self, SceneSendError<()>> {
        // Deserialize as a serialized function
        let serialized_function = postcard::from_bytes::<SerializedTestFunctionMessage>(value)
            .map_err(move |postcard_error| SceneSendError::CannotDeserialize((), format!("{:?}", postcard_error)))?;

        // Receive the function from the other side
        let function = context.receive_function(serialized_function.0).map_err(|err| err.map(|_| ()))?;

        Ok(TestFunctionMessage(Arc::new(function)))
    }
}

impl Serialize for TestFunctionMessage {
    fn serialize<S>(&self, _serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer 
    {
        Err(S::Error::custom("TestFunctionMessage cannot be serialized"))
    }
}

impl<'a> Deserialize<'a> for TestFunctionMessage {
    fn deserialize<D>(_deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'a> 
    {
        Err(D::Error::custom("TestFunctionMessage cannot be serialized"))
    }
}

#[test]
fn send_function_from_guest() {
    let scene = Scene::default();

    let guest_subprogram_id     = SubProgramId::called("Guest subprogram");
    let receiver_subprogram_id  = SubProgramId::called("Receiver subprogram");
    let test_subprogram_id      = SubProgramId::called("Test subprogram");

    // Start a guest runtime that mirrors messages
    let guest_runtime = GuestRuntime::with_default_subprogram(guest_subprogram_id, move |input: GuestInputStream<SimpleTestMessage>, context| async move {
        let function_context = context.clone();

        // Send a function to the host
        context.send_message(TestFunctionMessage(Arc::new(move |msg| { 
            let mut results = function_context.send::<SimpleTestMessage>(()).unwrap();

            async move {
                println!("Guest function called: {:?}", msg);

                let msg = postcard::from_bytes::<String>(&msg).unwrap();
                results.send(SimpleTestMessage { value: msg }).await.unwrap();
            }.boxed()
        }))).await.unwrap();

        // Read input forever to keep the guest running so the function can be called
        let mut input = input;
        while let Some(_) = input.next().await {
        }
    });

    // Run a receiver that receives the query from the guest and sends it on as test messages
    scene.add_subprogram(receiver_subprogram_id, move |input: InputStream<TestFunctionMessage>, _| async move {
        let mut input = input;
        while let Some(next_function) = input.next().await {
            // Call the function with some test messages
            let hello       = postcard::to_stdvec(&"hello".to_string()).unwrap();
            let goodbyte    = postcard::to_stdvec(&"goodbyte".to_string()).unwrap();

            println!("Calling with 'hello'");
            (next_function.0)(hello).await;
            println!("Calling with 'goodbyte'");
            (next_function.0)(goodbyte).await;
        }
    }, 0);

    // Run the guest in the scene, using the JSON encoder
    let (sender, receiver) = guest_runtime.as_streams();
    scene.add_subprogram(guest_subprogram_id, move |input: InputStream<SimpleTestMessage>, context| run_host_subprogram(input, context, sender, receiver), 20);

    // Connect the programs
    scene.connect_programs(guest_subprogram_id, test_subprogram_id, StreamId::with_message_type::<SimpleTestMessage>()).unwrap();
    scene.connect_programs(guest_subprogram_id, receiver_subprogram_id, StreamId::with_message_type::<TestFunctionMessage>()).unwrap();

    TestBuilder::new()
        .expect_message(|msg: SimpleTestMessage| { if msg.value == "hello" { Ok(()) } else { Err(format!("Value is {} (should be Hello)", msg.value)) } })
        .expect_message(|msg: SimpleTestMessage| { if msg.value == "goodbyte" { Ok(()) } else { Err(format!("Value is {} (should be Goodbyte)", msg.value)) } })
        .run_in_scene(&scene, test_subprogram_id);
}

#[test]
fn send_function_from_host() {
    let scene = Scene::default();

    let guest_subprogram_id     = SubProgramId::called("Guest subprogram");
    let receiver_subprogram_id  = SubProgramId::called("Receiver subprogram");
    let test_subprogram_id      = SubProgramId::called("Test subprogram");

    // Start a guest runtime that mirrors messages
    let guest_runtime = GuestRuntime::with_default_subprogram(guest_subprogram_id, move |input: GuestInputStream<TestFunctionMessage>, context| async move {
        let mut input = input;
        while let Some(next_function) = input.next().await {
            // Call the function with some test messages
            let hello       = postcard::to_stdvec(&"hello".to_string()).unwrap();
            let goodbyte    = postcard::to_stdvec(&"goodbyte".to_string()).unwrap();

            println!("Calling with 'hello'");
            (next_function.0)(hello).await;
            println!("Calling with 'goodbyte'");
            (next_function.0)(goodbyte).await;
        }
    });

    // Run a receiver that receives the query from the guest and sends it on as test messages
    scene.add_subprogram(receiver_subprogram_id, move |_: InputStream<()>, context| async move {
        let function_context = context.clone();

        // Send a function to the host
        context.send_message(TestFunctionMessage(Arc::new(move |msg| { 
            let mut results = function_context.send::<SimpleTestMessage>(()).unwrap();

            async move {
                println!("Guest function called: {:?}", msg);

                let msg = postcard::from_bytes::<String>(&msg).unwrap();
                results.send(SimpleTestMessage { value: msg }).await.unwrap();
            }.boxed()
        }))).await.unwrap();
    }, 0);

    // Run the guest in the scene, using the JSON encoder
    let (sender, receiver) = guest_runtime.as_streams();
    scene.add_subprogram(guest_subprogram_id, move |input: InputStream<TestFunctionMessage>, context| run_host_subprogram(input, context, sender, receiver), 20);

    // Connect the programs
    scene.connect_programs(receiver_subprogram_id, test_subprogram_id, StreamId::with_message_type::<SimpleTestMessage>()).unwrap();
    scene.connect_programs(receiver_subprogram_id, guest_subprogram_id, StreamId::with_message_type::<TestFunctionMessage>()).unwrap();

    TestBuilder::new()
        .expect_message(|msg: SimpleTestMessage| { if msg.value == "hello" { Ok(()) } else { Err(format!("Value is {} (should be Hello)", msg.value)) } })
        .expect_message(|msg: SimpleTestMessage| { if msg.value == "goodbyte" { Ok(()) } else { Err(format!("Value is {} (should be Goodbyte)", msg.value)) } })
        .run_in_scene(&scene, test_subprogram_id);
}
