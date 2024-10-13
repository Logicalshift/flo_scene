use flo_scene::wasm_rt::*;

#[no_mangle]
pub fn buffer_contents_are_1234(buffer: BufferHandle) -> bool {
    let buffer = claim_buffer(buffer);

    if buffer.len() != 4 || buffer[0] != 1 || buffer[1] != 2 || buffer[2] != 3 || buffer[3] != 4 {
        false
    } else {
        true
    }
}
