use crate::*;

use futures::prelude::*;
use futures::executor;
use futures::future::{BoxFuture};

use std::any::*;
use std::sync::*;

type ActionFn = Box<dyn FnOnce(InputStream<TestRequest>, &SceneContext) -> BoxFuture<'static, InputStream<TestRequest>>>;

///
/// Request sent to a test subprogram
///
enum TestRequest {
    /// A converted message from another source
    AnyMessage(Arc<dyn Send + Sync + Any>),
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
}

impl TestBuilder {
    ///
    /// Creates a new test builder
    ///
    pub fn new() -> Self {
        TestBuilder {
            actions: vec![],
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
            }
        }.boxed()));

        self
    }

    ///
    /// Expects a message of a particular type to be received by the test program
    ///
    /// The test program will configure itself to be able to receive messages of this type
    /// using a filter.
    ///
    pub fn expect_message<TMessage: SceneMessage>(self, assertion: impl FnOnce(TMessage) -> Result<(), String>) -> Self {
        todo!()
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