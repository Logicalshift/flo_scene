#![no_std]

extern crate alloc;

use flo_scene_guest::*;
use serde::*;
use futures::prelude::*;

use alloc::string::*;

extern crate wee_alloc;

// Use `wee_alloc` as the global allocator.
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

/// All the test program does is re-send the sample messages sent to it, which gives a basic test of a running subprogram
#[derive(Serialize, Deserialize, Debug)]
pub struct SampleMessage {
    value: String
}

impl SceneGuestMessage for SampleMessage {
    fn message_type_name() -> String {
        "flo_scene_tests::guest_subprogram_tests::SimpleTestMessage".into()
    }
}

///
/// Creates a subprogram running in a guest runtime
///
#[no_mangle]
pub extern "C" fn start_test_subprogram() -> GuestRuntimeHandle {
    // Start a runtime with a default subprogram that just echoes messages back again
    let runtime = GuestRuntime::with_default_subprogram(SubProgramId::new(), |input, context| async move {
        let mut input   = input;
        let sender      = context.send::<SampleMessage>(());

        if let Ok(mut sender) = sender {
            while let Some(msg) = input.next().await {
                let msg: SampleMessage = msg;

                sender.send(msg).await.ok();
            }
        }
    });

    // Register using postcard as the encoding scheme
    register_runtime(runtime)
}
