use uuid::{Uuid};
use serde::*;

///
/// Identifies module loaded by a WASM control program
///
#[derive(Serialize, Deserialize, Copy, Clone, Debug)]
pub enum WasmModuleId {
    Id(Uuid),
}

impl WasmModuleId {
    ///
    /// Creates a new WASM module ID with a unique ID
    ///
    pub fn new() -> WasmModuleId {
        WasmModuleId::Id(Uuid::new_v4())
    }
}
