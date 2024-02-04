use crate::*;

use futures::prelude::*;
use futures::executor;
use futures::future;
use futures::future::{BoxFuture};
use futures::channel::mpsc;

use std::any::*;
use std::collections::{HashMap};

type ActionFn = Box<dyn Send + FnOnce(InputStream<TestRequest>, &SceneContext, mpsc::Sender<String>) -> BoxFuture<'static, (InputStream<TestRequest>, mpsc::Sender<String>)>>;

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
            actions:            vec![],
            filters:            HashMap::new(),
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
        self.actions.push(Box::new(move |input_stream, context, failed_assertions| {
            let mut target_stream = context.send::<TMessage>(target).unwrap();

            async move {
                target_stream.send(message).await.unwrap();

                (input_stream, failed_assertions)
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
        self.actions.push(Box::new(move |input_stream, _context, failed_assertions| {
            async move {
                let mut input_stream        = input_stream;
                let mut failed_assertions   = failed_assertions;
                let next_message            = input_stream.next().await;

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
                                    failed_assertions.send(assertion_msg).await.ok();
                                }
                            }
                        } else {
                            // We expect the exact message that was specified
                            failed_assertions.send(format!("Received a message of an unexpected type (was expecting {})", type_name::<TMessage>())).await.ok();
                        }
                    },

                    None => {
                        // The input stream was closed while we were waiting for the message
                        failed_assertions.send(format!("Test finished prematurely")).await.ok();
                    }
                }

                (input_stream, failed_assertions)
            }.boxed()
        }));

        self
    }

    ///
    /// Runs the tests and the assertions in a scene
    ///
    pub fn run_in_scene(mut self, scene: &Scene, test_subprogram: SubProgramId) {
        use std::mem;

        // Set up filters for the expected message types
        for (stream_id, filter_handle) in self.filters.iter() {
            scene.connect_programs((), StreamTarget::Filtered(*filter_handle, test_subprogram), stream_id.clone()).unwrap();
        }

        // Create the test subprogram
        let (sender, receiver)  = mpsc::channel(100);
        let mut actions         = vec![];
        mem::swap(&mut self.actions, &mut actions);

        scene.add_subprogram(test_subprogram, |input_stream: InputStream<TestRequest>, context| {
            async move {
                let mut input_stream    = input_stream;
                let mut sender          = sender;

                for action in actions.into_iter() {
                    let (recycled_input_stream, recycled_sender) = action(input_stream, &context, sender).await;

                    input_stream    = recycled_input_stream;
                    sender          = recycled_sender;
                }

                // Close the assertions stream (which will end the test)
                mem::drop(sender);
            }
        }, 100);

        // Run the scene on the current thread, until the test actions have been finished
        let mut failures    = vec![];
        let future_failures = &mut failures;

        executor::block_on(future::select(async {
                scene.run_scene().await;
            }.boxed(),

            async move {
                let mut receiver = receiver;

                while let Some(assertion_failure) = receiver.next().await {
                    println!("{}", assertion_failure);
                    future_failures.push(assertion_failure);
                }
            }.boxed())
        );

        // Report any assertion failures
        let succeeded = failures.len() != 0;
        assert!(succeeded, "Scene tests failed\n\n  {}",
            failures.join("\n  "));
    }
}