use crate::error::*;

use wasmer::*;

///
/// A WASM module loaded by the control subprogram
///
pub struct WasmModule {
    store:      Store,
    module:     Module,
    instance:   Instance,
}

impl WasmModule {
    ///
    /// Loads a 'bare' module with the default runtime
    ///
    pub fn load_bare_module(module_bytes: &[u8]) -> Result<Self, WasmSubprogramError> {
        let mut store   = Store::default();
        let module      = Module::new(&store, &module_bytes)?;
        let imports     = Self::bare_imports();
        let instance    = Instance::new(&mut store, &module, &imports)?;

        Ok(WasmModule { store, module, instance })
    }

    ///
    /// The default set of imports for a 'bare' module
    ///
    fn bare_imports() -> Imports {
        imports! { }
    }
}
