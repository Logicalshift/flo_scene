use crate::control_streams::*;
use crate::module::*;
use crate::wasm_control::*;

use flo_scene::*;
use flo_scene::guest::*;

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

            RunModule(module_id, program_id, WasmMaxInputWaiting(max_input_waiting)) => {
                if let (Some(module), Some(update_target)) = (modules.get(&module_id), targets.get(&module_id)) {
                    // Obtain our own copies of the module and the update stream
                    let module              = Arc::clone(module);
                    let mut update_stream   = update_target.clone().and_then(|target| context.send(target).ok());

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

                            if let (Some(program_handle), Some(guest_stream_id)) = (program_handle, stream_id) {
                                // Add the 'start' message back to the results stream
                                let results = stream::once(future::ready(GuestResult::CreateSubprogram(program_id, program_handle, guest_stream_id.clone())))
                                    .chain(results);

                                // Run as a subprogram via the streams
                                let host_stream_id = StreamId::with_serialization_type(guest_stream_id.0);
                                if let Some(host_stream_id) = host_stream_id {
                                    if let Ok(start_message) = host_stream_id.run_host_subprogram_postcard(program_id, max_input_waiting, actions, results) {
                                        // TODO: it's possible that this will fail as well if the control program is not running
                                        context.send_message(start_message).await.ok();
                                    } else {
                                        todo!("Could not create message for some reason");
                                    }
                                } else {
                                    // TODO: we can still run the subprogram anonymously if we have a way to pass on encoded messages of the appropriate type (this needs a new way to identify streams though)
                                    // TODO: alternatively, report an error
                                    todo!("No known stream ID for this program")
                                }

                                // Notify the update stream that we're running
                                if let Some(update_stream) = &mut update_stream {
                                    update_stream.send(WasmUpdate::RunningModule(module_id, program_id)).await.unwrap();
                                }

                                // TODO: way to notify the update stream that we've finished running
                                // TODO: way to use other encodings
                            } else {
                                // The main program failed to start
                                todo!("Program failed to start for some reason");
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
