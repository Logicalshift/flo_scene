use crate::host::scene_context::*;

use super::error::*;

use futures::prelude::*;

use std::fmt::{Debug};

///
/// Extension functions for generating errors in a scene
///
/// Using these we can write, say, `some_task.with_report().await.ok()` to report but ignore errors,
/// or `some_task.or_fail().await` to fail the current program if there's an error. `or_fail()` is
/// nicer than `unwrap()` as it doesn't panic and it shuts down the current scene.
///
pub trait SceneErrorExt<'a, TVal, TErr> {
    /// If an error occurs, report it but otherwise continue
    fn with_report(self) -> impl 'a + Send + Future<Output=Result<TVal, TErr>>;

    /// If an error occurs, report a failure and immediately stop the running subprogram without returning
    /// Failures usually shut down the scene as well.
    fn or_fail(self) -> impl 'a + Send + Future<Output=TVal>;
}

impl<'a, TFuture, TVal, TErr> SceneErrorExt<'a, TVal, TErr> for TFuture
where 
    TFuture:    'a + Send + Future<Output=Result<TVal, TErr>>,
    TVal:       Send,
    TErr:       Send + Sync,
    TErr:       Debug,
{
    fn with_report(self) -> impl 'a + Send + Future<Output=Result<TVal, TErr>> {
        async move {
            // Wait for the result
            let result = self.await;

            // Report any errors if they occur
            if let Err(err) = &result {
                let context     = scene_context();
                let program_id  = context.as_ref().and_then(|ctxt| ctxt.current_program_id());

                if let (Some(context), Some(program_id)) = (context, program_id) {
                    context.send_message(Error::Error { source: program_id, message: format!("{:?}", err).into() }).await.ok();
                }
            }

            result
        }
    }

    fn or_fail(self) -> impl 'a + Send + Future<Output=TVal> {
        async move {
            // Wait for the result
            let result = self.await;

            // Report any errors if they occur
            match result {
                Err(err) => {
                    let context     = scene_context();
                    let program_id  = context.as_ref().and_then(|ctxt| ctxt.current_program_id());

                    if let (Some(context), Some(program_id)) = (context, program_id) {
                        // Send the failure message
                        let fail_result = context.send_message(Error::Failure { source: program_id, message: format!("{:?}", err).into() }).await;

                        if let Err(send_fail) = fail_result {
                            // Panic if the failure could not be sent
                            panic!("UNEXPECTED FAILURE: {:?} ({:?} when sending)", err, send_fail);
                        }

                        // TODO: abort the current process/program
                    } else {
                        // This is a panic if it happens outside of a scene
                        panic!("UNEXPECTED FAILURE: {:?}", err);
                    }

                    // Yield forever (program will abort)
                    use futures::task::{Poll};
                    future::poll_fn(|_ctxt| Poll::<()>::Pending).await;

                    unreachable!("Should sleep forever (and abort the program)");
                }

                // Result of the future is only ever the successful value
                Ok(val) => {
                    val
                }
            }
        }
    }
}