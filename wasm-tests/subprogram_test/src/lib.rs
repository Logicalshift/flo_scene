use flo_scene::host::*;
use flo_scene::guest::*;
use flo_scene::wasm_rt::*;

use futures::prelude::*;
use serde::*;

/// All the test program does is re-send the sample messages sent to it, which gives a basic test of a running subprogram
#[derive(Serialize, Deserialize)]
pub struct SampleMessage {
    value: String
}

impl SceneMessage for SampleMessage {
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
    let runtime = GuestRuntime::with_default_subprogram(SubProgramId::new(), GuestPostcardEncoder, |input, context| async move {
        let mut input = input;
        let mut sender = context.send(()).unwrap();

        while let Some(msg) = input.next().await {
            let msg: SampleMessage = msg;

            sender.send(msg).await.unwrap();
        }
    });

    // Register using postcard as the encoding scheme
    register_postcard_runtime(runtime)
}
