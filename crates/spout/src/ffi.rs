use std::os::raw::c_char;

/// Opaque handle to the native Spout bridge (defined only in
/// `cpp/spout_bridge.cpp`). Rust never reads through it, only holds pointers.
#[repr(C)]
pub struct SpoutBridgeHandle {
    _private: [u8; 0],
}

// Edition 2024 requires FFI declaration blocks to be `unsafe extern "C"`.
unsafe extern "C" {
    pub fn spout_bridge_create(name: *const c_char) -> *mut SpoutBridgeHandle;

    /// `pixels` must point to at least `pitch * height` readable, initialized
    /// bytes. The bridge copies them before returning.
    pub fn spout_bridge_send_bgra(
        handle: *mut SpoutBridgeHandle,
        pixels: *const u8,
        width: u32,
        height: u32,
        pitch: u32,
    ) -> bool;

    pub fn spout_bridge_destroy(handle: *mut SpoutBridgeHandle);
}
