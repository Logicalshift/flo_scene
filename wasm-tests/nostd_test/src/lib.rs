#![no_std]

extern crate alloc;

use flo_scene_nostd::*;

extern crate wee_alloc;

// Use `wee_alloc` as the global allocator.
#[global_allocator]
static ALLOC: wee_alloc::WeeAlloc = wee_alloc::WeeAlloc::INIT;

///
/// Creates a subprogram running in a guest runtime
///
#[no_mangle]
pub extern "C" fn start_test_subprogram() -> GuestRuntimeHandle {
    /*
    // Start a runtime with a default subprogram that just echoes messages back again
    let runtime = GuestRuntime::with_default_subprogram(SubProgramId::new(), |input, context| async move {
        let mut input = input;
        let mut sender = context.send(()).unwrap();

        while let Some(msg) = input.next().await {
            let msg: SampleMessage = msg;

            sender.send(msg).await.unwrap();
        }
    });

    // Register using postcard as the encoding scheme
    register_postcard_runtime(runtime)
    */
    GuestRuntimeHandle(0)
}
