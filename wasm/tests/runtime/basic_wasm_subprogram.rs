//!
//! These tests depend on the examples in the wasm-tests folder, which need to be compiled separately as
//! `wasm32-unknown-unknown` (there's a shell script for doing this)
//!
//! We redeclare the message types here, which normally you wouldn't do: it's best to have two crates for
//! guest programs, one that defines the messages and one that provides the implementations. The types must
//! match between the host and target for things to make sense (this is particularly important when using
//! postcard which may re-interpret messages as valid-but wrong messages)
//!

use flo_scene::*;
use flo_scene::programs::*;
use flo_scene_wasm::*;

use futures::prelude::*;
use serde::*;

/// Bytecode for the tests
static SUBPROGRAM_TEST_WASM: &'static [u8] = include_bytes!("../../../wasm-tests/wasm/flo_scene_wasm_subprogram_test.wasm");

/// SampleMessage is defined in the subprogram_tests crate in wasm-tests; this should match it exactly
#[derive(Serialize, Deserialize, Debug)]
pub struct SampleMessage {
    value: String
}

impl SceneMessage for SampleMessage {
    fn message_type_name() -> String {
        "flo_scene_tests::guest_subprogram_tests::SimpleTestMessage".into()
    }
}

#[test]
pub fn send_and_receive_single_message() {
    // Create a default scene
    let scene               = Scene::default();
    let test_program_id     = SubProgramId::called("host_test_program");
    let start_program_id    = SubProgramId::called("start_wasm_program");
    let wasm_program_id     = SubProgramId::called("test");
    let wasm_module_id      = WasmModuleId::new();

    // Send any messages sent to the default target to the test subprogram
    scene.connect_programs((), test_program_id, StreamId::with_message_type::<SampleMessage>()).unwrap();

    // Start a WASM subprogram (there's a default WASM control program that will start if we use the message type, so we just need a program that makes a request to start it up)
    scene.add_subprogram(start_program_id, |_input: InputStream<()>, context| async move {
        // Load the module and start the subprogram (we have two messages here as this gives us a way to start many subprograms)
        context.send_message(WasmControl::LoadModule(wasm_module_id, (*SUBPROGRAM_TEST_WASM).into(), Some(test_program_id.into()))).await.unwrap();
        context.send_message(WasmControl::RunModule(wasm_module_id, wasm_program_id)).await.unwrap();

        println!("Started subprogram");

        // Should be able to send messages to it now (the program relays them to the default target, which is our test program)
        let mut wasm_target = context.send(wasm_program_id).unwrap();

        wasm_target.send(SampleMessage { value: "Hello".into() }).await.unwrap();
        wasm_target.send(SampleMessage { value: "Goodbyte".into() }).await.unwrap();

        println!("Sent messages");
    }, 0);

    TestBuilder::new()
        .expect_message(|msg1: SampleMessage| { if &msg1.value == "Hello" { Ok(()) } else { Err(format!("Received wrong message ({})", msg1.value)) } })
        .expect_message(|msg2: SampleMessage| { if &msg2.value == "Goodbyte" { Ok(()) } else { Err(format!("Received wrong message ({})", msg2.value)) } })
        .run_in_scene(&scene, test_program_id);
}
