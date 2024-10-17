use crate::control_streams::*;
use crate::module::*;
use crate::wasm_control::*;

use flo_scene::*;
use flo_scene::guest::*;
use flo_scene::programs::*;

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
                            // Create streams to run the program
                            let (actions, results) = create_module_streams(module, runtime);

                            // Read results until we see the subprogram start
                            let mut results         = results;
                            let mut program_handle  = None;
                            let mut stream_id       = None;
                            while let Some(msg) = results.next().await {
                                match msg {
                                    GuestResult::CreateSubprogram(main_program_id, main_program_handle, host_stream_id) => {
                                        if main_program_id != program_id {
                                            // The program that started was different to the one we expected
                                        }

                                        program_handle  = Some(main_program_handle);
                                        stream_id       = Some(host_stream_id);

                                        break;
                                    }

                                    // Can't process any other messages
                                    _ => { }
                                }
                            }

                            if let (Some(program_handle), Some(stream_id)) = (program_handle, stream_id) {
                                // Add the 'start' message back to the results stream
                                let results = stream::once(future::ready(GuestResult::CreateSubprogram(program_id, program_handle, stream_id.clone())))
                                    .chain(results);

                                // Run as a subprogram via the streams
                                // TODO: use the host stream ID to determine the input type here
                                context.send_message(SceneControl::start_program(program_id, move |input: InputStream<()>, context| async move {
                                    run_host_subprogram(input, context, GuestPostcardEncoder, actions, results).await;
                                }, 20)).await.unwrap();

                                // TODO: notify the update stream that we're running
                                // TODO: way to notify the update stream that we've finished running
                                // TODO: way to use other encodings
                                // TODO: way to configure the input buffer size (we're just using 20 at the moment)
                            } else {
                                // The main program failed to start
                                todo!();
                            }
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
