//!
//! The functions that are exported to the host in order to allow it to communicate with the guest
//!

use super::types::*;

///
/// Sends a message to a guest subprogram in a runtime
///
#[no_mangle]
pub extern "C" fn scene_guest_postcard_send_message(runtime: GuestRuntimeHandle, target: GuestSubProgramHandle, postcard_data: BufferHandle) {
    todo!()
}

///
/// Indicates to a guest subprogram that it is safe to send to a sink
///
#[no_mangle]
pub extern "C" fn scene_guest_postcard_sink_ready(runtime: GuestRuntimeHandle, sink: HostSinkHandle) {
    todo!()
}

///
/// Indicates to aguest subprogram that an error ocurred while connecting a sink
///
#[no_mangle]
pub extern "C" fn scene_guest_postcard_sink_connection_error(runtime: GuestRuntimeHandle, sink: HostSinkHandle, postcard_error: BufferHandle) {
    todo!()
}

///
/// Indicates to a guest subprogram that an error ocurred while sending data to a sink
///
#[no_mangle]
pub extern "C" fn scene_guest_postcard_sink_send_error(runtime: GuestRuntimeHandle, sink: HostSinkHandle, postcard_error: BufferHandle) {
    todo!()
}

///
/// Performs the poll_awake runtime operation, filling a buffer, and returning it to the host. The return format is always using the 'postcard' serialziation
/// here.
///
/// The host should call scene_free_buffer on this buffer
///
#[no_mangle]
pub extern "C" fn scene_guest_postcard_poll_awake(runtime: GuestRuntimeHandle) -> BufferHandle {
    todo!()
}

///
/// Sends a message to a stream on a guest
///
#[no_mangle]
pub extern "C" fn scene_guest_postcard_send_stream(runtime: GuestRuntimeHandle, stream_id: i32, message: BufferHandle) {
    todo!()
}

///
/// Indicates that a host stream is ready to receive another message
///
#[no_mangle]
pub extern "C" fn scene_guest_postcard_ready_stream(runtime: GuestRuntimeHandle, stream_id: i32) {
    todo!()
}

///
/// Indicates that a stream has been closed on the host side
///
#[no_mangle]
pub extern "C" fn scene_guest_postcard_close_stream(runtime: GuestRuntimeHandle, stream_id: i32) {
    todo!()
}
