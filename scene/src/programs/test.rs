use crate::*;

use futures::prelude::*;
use futures::executor;
use futures::future::{BoxFuture};

use std::any::*;
use std::collections::{HashMap};
use std::sync::*;

type ActionFn = Box<dyn FnOnce(InputStream<TestRequest>, &SceneContext) -> BoxFuture<'static, InputStream<TestRequest>>>;

///
/// Request sent to a test subprogram
///
enum TestRequest {
    /// A converted message from another source
    AnyMessage(Box<dyn Send + Sync + Any>),
}

impl SceneMessage for TestRequest {

}

///
/// The test builder can be used to create a test subprogram for a scene
///
/// A test subprogram can send and expect messages in response
///
pub struct TestBuilder {
    /// The actions for the test process to perform
    actions: Vec<ActionFn>,

    /// The filters that need to be applied to the input of the test program
    filters: HashMap<StreamId, FilterHandle>,
}

impl TestBuilder {
    ///
    /// Creates a new test builder
    ///
    pub fn new() -> Self {
        TestBuilder {
            actions: vec![],
            filters: HashMap::new(),
        }
    }

    ///
    /// Adds a test action that sends a message to the scene originating from the test program
    ///
    pub fn send_message<TMessage: 'static + SceneMessage>(self, message: TMessage) -> Self {
        self.send_message_to_target((), message)
    }

    ///
    /// Adds a test action that sends a message to the scene originating from the test program
    ///
    pub fn send_message_to_target<TMessage: 'static + SceneMessage>(mut self, target: impl Into<StreamTarget>, message: TMessage) -> Self {
        let target = target.into();

        // Add an action that retrieves the target stream and sends the message to it
        self.actions.push(Box::new(move |input_stream, context| {
            let mut target_stream = context.send::<TMessage>(target).unwrap();

            async move {
                target_stream.send(message).await.unwrap();

                input_stream
            }.boxed()
        }));

        self
    }

    ///
    /// Expects a message of a particular type to be received by the test program
    ///
    /// The test program will configure itself to be able to receive messages of this type
    /// using a filter.
    ///
    pub fn expect_message<TMessage: 'static + Send + Sync + SceneMessage>(mut self, assertion: impl 'static + Send + FnOnce(TMessage) -> Result<(), String>) -> Self {
        // Create a filter for the message type
        self.filters.entry(StreamId::with_message_type::<TMessage>())
            .or_insert_with(|| {
                FilterHandle::for_filter(|source_stream: InputStream<TMessage>| source_stream.map(|msg| TestRequest::AnyMessage(Box::new(msg))))
            });

        // Add an action to receive the message from the target
        self.actions.push(Box::new(move |input_stream, context| {
            async move {
                let mut input_stream    = input_stream;
                let next_message        = input_stream.next().await;

                match next_message {
                    Some(TestRequest::AnyMessage(any_message))  => {
                        // Check that the message matches
                        if let Ok(message) = any_message.downcast::<TMessage>() {
                            match assertion(*message) {
                                Ok(()) => {
                                    // Assertion OK so we can continue
                                }

                                Err(assertion_msg) => {
                                    // Message does not match the assertion
                                    todo!("add assertion failed")
                                }
                            }
                        } else {
                            // We expect the exact message that was specified
                            todo!("message was an unexpected type");
                        }
                    },

                    None => {
                        // The input stream was closed while we were waiting for the message
                        todo!("test finished prematurely");
                    }
                }

                input_stream
            }.boxed()
        }));

        self
    }

    ///
    /// Runs the tests and the assertions in a scene
    ///
    pub fn run_in_scene(scene: &Scene, test_subprogram: SubProgramId) {
        // Set up the filters to read the messages resulting from the test messages
        todo!();

        // Create the test subprogram
        todo!();

        // Run the scene on the current thread, until the test actions have been finished
        executor::block_on(async {
            scene.run_scene().await;
        });

        // Report any assertion failures
        todo!();
    }
}