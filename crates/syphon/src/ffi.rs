use std::os::raw::c_char;

/// Opaque handle to the native bridge (defined only in `cpp/syphon_bridge.mm`).
/// The zero-sized field is the standard pattern for an FFI opaque type: Rust
/// never constructs or reads through this struct, only holds pointers to it.
#[repr(C)]
pub struct SyphonBridgeHandle {
    _private: [u8; 0],
}

// Edition 2024 requires FFI declaration blocks to be written as `unsafe
// extern "C"` (RFC 3484) — every item inside remains individually unsafe to
// call, same as a pre-2024 bare `extern "C"` block.
unsafe extern "C" {
    pub fn syphon_bridge_create(server_name: *const c_char) -> *mut SyphonBridgeHandle;

    /// `pixels` must point to at least `bytes_per_row * height` readable,
    /// initialized bytes. The bridge copies them into its own `IOSurface`
    /// before returning and retains no pointer afterward.
    pub fn syphon_bridge_send_rgba(
        handle: *mut SyphonBridgeHandle,
        pixels: *const u8,
        width: u32,
        height: u32,
        bytes_per_row: u32,
    ) -> bool;

    pub fn syphon_bridge_destroy(handle: *mut SyphonBridgeHandle);
}
