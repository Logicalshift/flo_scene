use flo_scene::*;
use flo_scene::programs::*;
use flo_scene_wasm::*;

use serde::*;

/// Bytecode for the tests
static SUBPROGRAM_TEST_WASM: &'static [u8] = include_bytes!("../../../wasm-tests/wasm/flo_scene_wasm_subprogram_test.wasm");

/// ProgramIdMessage is defined in the subprogram_tests crate in wasm-tests; this should match it exactly
#[derive(Serialize, Deserialize, Debug)]
pub struct ProgramIdMessage {
    id: SubProgramId
}

impl SceneMessage for ProgramIdMessage {
    fn message_type_name() -> String {
        "flo_scene_tests::guest_subprogram_tests::ProgramIdMessage".into()
    }
}

#[test]
pub fn receive_program_id() {
    // Create a default scene
    let scene               = Scene::default();
    let test_program_id     = SubProgramId::called("host_test_program");
    let start_program_id    = SubProgramId::called("start_wasm_program");
    let wasm_program_id     = SubProgramId::called("uuid");
    let wasm_module_id      = WasmModuleId::new();

    // Send any messages sent to the default target to the test subprogram
    scene.connect_programs((), test_program_id, StreamId::with_message_type::<ProgramIdMessage>()).unwrap();

    // Start a WASM subprogram that sends a UUID message
    scene.add_subprogram(start_program_id, |_input: InputStream<()>, context| async move {
        context.send_message(WasmControl::LoadModule(wasm_module_id, (*SUBPROGRAM_TEST_WASM).into(), Some(test_program_id.into()))).await.unwrap();
        context.send_message(WasmControl::RunModule(wasm_module_id, wasm_program_id)).await.unwrap();

        println!("Started subprogram");
    }, 0);

    // Should receive notification that the module loaded, then a subprogram ID
    TestBuilder::new()
        .expect_message(|loaded_module: WasmUpdate| { if let WasmUpdate::ModuleLoaded(_) = loaded_module { Ok(()) } else { Err(format!("Unexpected update: {:?}", loaded_module)) } })
        .expect_message(|running_module: WasmUpdate| { if let WasmUpdate::RunningModule(_, _) = running_module { Ok(()) } else { Err(format!("Unexpected update: {:?}", running_module)) } })
        .expect_message(|program_id: ProgramIdMessage| {
            // The UUID is random so all we can really do is assert that it's a V4 UUID (corrupt UUIDs will likely produce other version numbers)
            println!("Received program ID {:?}", program_id.id);
            let uuid = program_id.id.to_uuid();

            if uuid.unwrap().get_version_num() == 4 {
                Ok(())
            } else {
                Err(format!("{:?} is not a valid V4 UUID", uuid))
            }
        })
        .run_in_scene(&scene, test_program_id);
}
