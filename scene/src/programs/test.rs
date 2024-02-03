use crate::*;

use futures::executor;

use std::any::*;
use std::sync::*;

///
/// The test builder can be used to create a test subprogram for a scene
///
/// A test subprogram can send and expect messages in response
///
pub struct TestBuilder {

}

impl TestBuilder {
    ///
    /// Creates a new test builder
    ///
    pub fn new() -> Self {
        TestBuilder {

        }
    }

    ///
    /// Adds a test action that sends a message to the scene originating from the test program
    ///
    pub fn send_message<TMessage: SceneMessage>(self, message: TMessage) -> Self {
        todo!()
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
        // Create the test subprogram
        todo!();

        // Run the scene on the current thread
        executor::block_on(async {
            scene.run_scene().await;
        });

        // Report any assertion failures
        todo!();
    }
}