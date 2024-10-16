use super::module::*;
use super::wasm_control::*;

use flo_scene::*;

use futures::prelude::*;

use std::collections::{HashMap};
use std::sync::*;

///
/// A subprogram that loads and runs subprograms written in WASM
///
pub async fn wasm_control_subprogram(input: InputStream<WasmControl>, context: SceneContext) {
    let mut input = input;

    let mut modules = HashMap::new();
    let mut targets = HashMap::new();

    while let Some(instruction) = input.next().await {
        use WasmControl::*;

        match instruction {
            LoadModule(module_id, module_bytes, update_target) => {
                // Load the module as a bare module
                let new_module = WasmModule::load_bare_module(&module_bytes);

                // Open a connection to the update target
                let mut update_stream = update_target.clone().and_then(|target| context.send(target).ok());

                match new_module {
                    Ok(new_module) => {
                        // Store the new module
                        modules.insert(module_id, Arc::new(Mutex::new(new_module)));
                        targets.insert(module_id, update_target.clone());

                        // Tell the target about the new module
                        if let Some(update_stream) = &mut update_stream {
                            update_stream.send(WasmUpdate::ModuleLoaded(module_id)).await.ok();
                        }
                    }

                    Err(err) => {
                        // Tell the target about the failure
                        if let Some(update_stream) = &mut update_stream {
                            update_stream.send(WasmUpdate::CouldNotLoadModule(module_id, err)).await.ok();
                        }
                    }
                }
            }

            RunModule(module_id, program_id) => {
                if let (Some(module), Some(update_target)) = (modules.get(&module_id), targets.get(&module_id)) {
                    // Obtain our own copies of the module and the update stream
                    let module          = Arc::clone(module);
                    let update_stream   = update_target.clone().and_then(|target| context.send(target).ok());

                    // Start the module running
                    let runtime = module.lock().unwrap().start_guest(program_id);

                    match runtime {
                        Ok(runtime) => {
                            // TODO: Create streams to run the program

                            // TODO: run as a subprogram via the streams

                            // TODO: notify the update stream that we're running
                            // TODO: way to notify the update stream that we've finished running
                        }

                        Err(err) => {
                            if let Some(mut update_stream) = update_stream {
                                update_stream.send(WasmUpdate::CouldNotStartSubProgram(module_id, program_id, err)).await.ok();
                            }
                        }
                    }
                } else {
                    // Module is not loaded (TODO: need to send this as an error somewhere)
                }
            }
        }
    }
}
