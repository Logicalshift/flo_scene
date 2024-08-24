use crate::error::*;
use crate::module_id::*;

use flo_scene::*;

use serde::*;

///
/// Control messages for loading WASM modules
///
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum WasmControl {
    /// Loads a module defined as a byte stream, optionally sending updates about it to the specified subprogram
    LoadModule(WasmModuleId, Vec<u8>, Option<StreamTarget>),

    /// Runs the default subprogram contained within a WASM module as the specified subprogram ID
    RunModule(WasmModuleId, SubProgramId),
}

///
/// Updates sent by the WASM control program
///
#[derive(Serialize, Deserialize, Clone, Debug)]
pub enum WasmUpdate {
    /// Tried to load a module but there was a problem
    CouldNotLoadModule(WasmModuleId, WasmError),

    /// A `LoadModule` command was successful
    ModuleLoaded(WasmModuleId),

    /// A subprogram from a module is running
    RunningModule(WasmModuleId, SubProgramId),
}

impl SceneMessage for WasmControl {
}

impl SceneMessage for WasmUpdate {
}