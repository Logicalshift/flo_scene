use flo_scene::uuid_impl::*;

use uuid::{Uuid};
use serde::*;

///
/// Identifies module loaded by a WASM control program
///
#[derive(Serialize, Deserialize, Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WasmModuleId {
    Id(Uuid),
}

impl WasmModuleId {
    ///
    /// Creates a new WASM module ID with a unique ID
    ///
    pub fn new() -> WasmModuleId {
        WasmModuleId::Id(new_uuid())
    }
}
