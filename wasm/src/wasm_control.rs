use crate::error::*;
use crate::module_id::*;
use crate::control_subprogram::*;

use flo_scene::*;

use serde::*;

///
/// The maximum number of waiting messages allowed in the queue for a WASM subprogram
///
#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub struct WasmMaxInputWaiting(pub usize);

///
/// Control messages for loading WASM modules
///
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum WasmControl {
    /// Loads a module defined as a byte stream, optionally sending updates about it to the specified subprogram
    LoadModule(WasmModuleId, Vec<u8>, Option<StreamTarget>),

    /// Runs the default subprogram contained within a WASM module as the specified subprogram ID
    RunModule(WasmModuleId, SubProgramId, WasmMaxInputWaiting),
}

///
/// Updates sent by the WASM control program
///
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum WasmUpdate {
    /// Tried to load a module but there was a problem
    CouldNotLoadModule(WasmModuleId, WasmSubprogramError),

    /// A subprogram could not be started for some reason
    CouldNotStartSubProgram(WasmModuleId, SubProgramId, WasmSubprogramError),

    /// A `LoadModule` command was successful
    ModuleLoaded(WasmModuleId),

    /// A subprogram from a module is running
    RunningModule(WasmModuleId, SubProgramId),
}

impl SceneMessage for WasmControl {
    fn default_target() -> StreamTarget {
        StreamTarget::Program(SubProgramId::called("flo_scene_wasm::control"))
    }

    fn initialise(scene: &Scene) {
        // Connect to the default subprogram by default
        scene.connect_programs((), Self::default_target(), StreamId::with_message_type::<Self>()).unwrap();

        // Run the default subprogram at this location
        scene.add_subprogram(Self::default_target().target_sub_program().unwrap(), |input, context| wasm_control_subprogram(input, context), 1);
    }
}

impl SceneMessage for WasmUpdate {
}