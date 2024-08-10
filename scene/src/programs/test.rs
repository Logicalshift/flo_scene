use crate::*;
use super::control::*;
use super::query::*;

use futures::prelude::*;
use futures::executor;
use futures::future;
use futures::future::{BoxFuture};
use futures::channel::mpsc;
use futures_timer::{Delay};

use std::any::*;
use std::collections::{HashMap};
use std::time::{Duration};

type ActionFn = Box<dyn Send + FnOnce(InputStream<TestRequest>, &SceneContext, mpsc::Sender<String>) -> BoxFuture<'static, (InputStream<TestRequest>, mpsc::Sender<String>)>>;

///
/// Request sent to a test subprogram
///
enum TestRequest {
    /// A converted message from another source
    AnyMessage(Box<dyn Send + Any>),
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

    /// Timeout before the tests are considered to have failed
    timeout: Duration,
}

impl TestBuilder {
    ///
    /// Creates a new test builder
    ///
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        TestBuilder {
            actions:            vec![],
            filters:            HashMap::new(),
            timeout:            Duration::from_millis(5000),
        }
    }

    ///
    /// Sets the amount of time the test will wait until failing automatically
    ///
    pub fn timeout_after(mut self, timeout: impl Into<Duration>) -> Self {
        self.timeout = timeout.into();

        self
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
                target_stream.send(message).await.map_err(|err| ConnectionError::from(err)).unwrap();

                (input_stream, failed_assertions)
            }.boxed()
        }));

        self
    }

    ///
    /// Runs a `Command` and then evaluates an assertion against the messages that it returns
    ///
    /// The command is run to completion and the output stream is gathered into a vec that's passed to the assertion routine.
    ///
    pub fn run_command<TCommand: 'static + Command>(mut self, command: TCommand, input: Vec<TCommand::Input>, assertion: impl 'static + Send + Fn(Vec<TCommand::Output>) -> Result<(), String>) -> Self {
        self.actions.push(Box::new(move |input_stream, context, failed_assertions| {
            let context = context.clone();

            async move {
                // Run the command and gather the output
                let command_result = context.spawn_command(command, stream::iter(input)).unwrap().collect::<Vec<_>>().await;

                // Check the result against the assertion
                let mut failed_assertions = failed_assertions;
                match assertion(command_result) {
                    Ok(())                  => { }
                    Err(failure_messge)     => { failed_assertions.send(failure_messge).await.unwrap(); }
                }

                (input_stream, failed_assertions)
            }.boxed()
        }));

        self
    }

    ///
    /// Runs a `Command` with the output of a query and then evaluates an assertion against the messages that it returns
    ///
    /// The command is run to completion and the output stream is gathered into a vec that's passed to the assertion routine.
    ///
    pub fn run_query<TCommand: 'static + Command>(mut self, command: TCommand, query: impl 'static + QueryRequest<ResponseData=TCommand::Input>, query_target: impl Into<StreamTarget>, assertion: impl 'static + Send + Fn(Vec<TCommand::Output>) -> Result<(), String>) -> Self {
        let query_target = query_target.into();

        self.actions.push(Box::new(move |input_stream, context, failed_assertions| {
            let context = context.clone();

            async move {
                // Run the command and gather the output
                let command_result = context.spawn_query(command, query, query_target).unwrap().collect::<Vec<_>>().await;

                // Check the result against the assertion
                let mut failed_assertions = failed_assertions;
                match assertion(command_result) {
                    Ok(())                  => { }
                    Err(failure_messge)     => { failed_assertions.send(failure_messge).await.unwrap(); }
                }

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
    pub fn expect_message<TMessage: 'static + Send + SceneMessage>(self, assertion: impl 'static + Send + FnOnce(TMessage) -> Result<(), String>) -> Self {
        self.expect_message_async(move |value| async move { assertion(value) })
    }

    ///
    /// Expects a message of a particular type to be received by the test program
    ///
    /// The test program will configure itself to be able to receive messages of this type
    /// using a filter.
    ///
    pub fn expect_message_async<TMessage: 'static + Send + SceneMessage, TFuture>(mut self, assertion: impl 'static + Send + FnOnce(TMessage) -> TFuture) -> Self 
    where
        TFuture: 'static + Send + Future<Output=Result<(), String>>,
    {
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
                            match assertion(*message).await {
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
                        failed_assertions.send("Test finished prematurely".to_string()).await.ok();
                    }
                }

                (input_stream, failed_assertions)
            }.boxed()
        }));

        self
    }

    ///
    /// Creates a test action that redirects the input for a particular message type to the test program (optionally for a specific target subprogram)
    ///
    /// Note that `expect_message` will redirect the default target of a message to the test program already, so this is generally used to redirect
    /// a direct connection.
    ///
    pub fn redirect_input<TMessage>(mut self, target: SubProgramId) -> Self 
    where
        TMessage: 'static + SceneMessage,
    {
        let stream_id = StreamId::with_message_type::<TMessage>().for_target(target);

        // Create a filter for the message type
        let filter_handle = *self.filters.entry(StreamId::with_message_type::<TMessage>())
            .or_insert_with(|| {
                FilterHandle::for_filter(|source_stream: InputStream<TMessage>| source_stream.map(|msg| TestRequest::AnyMessage(Box::new(msg))))
            });

        self.actions.push(Box::new(move |input_stream, context, failed_assertions| { 
            let program_id  = context.current_program_id().unwrap();
            let context     = context.clone();

            // Retrieve the filter for this message type
            async move {
                context.send_message(SceneControl::Connect(().into(), StreamTarget::Filtered(filter_handle, program_id.into()), stream_id)).await.unwrap();

                (input_stream, failed_assertions)
            }.boxed()
        })
    );

        self
    }

    ///
    /// Sets up a scene to run the tests, then awaits the 'runner' future
    ///
    fn run_tests(mut self, scene: &Scene, test_subprogram: SubProgramId, runner: impl Send + Future<Output=()>) {
        use std::mem;

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

        // Set up filters for the expected message types
        for (stream_id, filter_handle) in self.filters.iter() {
            scene.connect_programs((), StreamTarget::Filtered(*filter_handle, test_subprogram), stream_id.clone()).unwrap();
        }

        // Run the scene on the current thread, until the test actions have been finished
        let mut failures    = vec![];
        let future_failures = &mut failures;
        let timeout         = self.timeout;
        let mut timed_out   = false;

        executor::block_on(future::select(async {
                // Run the scene
                runner.await;
            }.boxed(),

            future::select(
                async move {
                    // Wait for assertion failures and add them to the list
                    // Stop when the assertions stream is closed (which stops the tests overall)
                    let mut receiver = receiver;

                    while let Some(assertion_failure) = receiver.next().await {
                        println!("{}", assertion_failure);
                        future_failures.push(assertion_failure);
                    }
                }.boxed(),

                async {
                    // Stop when the timeout is reached and set the 'timed_out' flag
                    Delay::new(timeout).await;
                    timed_out = true;
                }.boxed()).boxed(),
        ));

        // If we timed out, that counts as an assertion failure
        if timed_out {
            failures.push(format!("Tests took more than {:?} to complete", timeout));
        }

        // Report any assertion failures
        let succeeded = failures.is_empty();
        assert!(succeeded, "Scene tests failed\n\n  {}",
            failures.join("\n  "));
    }

    ///
    /// Runs the tests and the assertions in a scene
    ///
    pub fn run_in_scene(self, scene: &Scene, test_subprogram: SubProgramId) {
        self.run_tests(scene, test_subprogram, scene.run_scene());
    }

    ///
    /// Run the test program in a scene using multithreading
    ///
    pub fn run_in_scene_with_threads(self, scene: &Scene, test_subprogram: SubProgramId, thread_count: usize) {
        self.run_tests(scene, test_subprogram, scene.run_scene_with_threads(thread_count));
    }
}
