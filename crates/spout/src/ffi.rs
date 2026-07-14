use std::os::raw::c_char;

/// Opaque handle to the native Spout bridge (defined only in
/// `cpp/spout_bridge.cpp`). Rust never reads through it, only holds pointers.
#[repr(C)]
pub struct SpoutBridgeHandle {
    _private: [u8; 0],
}

// Edition 2024 requires FFI declaration blocks to be written as `unsafe
// extern "C"` (RFC 3484) — every item inside remains individually unsafe to
// call, same as a pre-2024 bare `extern "C"` block.
unsafe extern "C" {
    pub fn spout_bridge_create(name: *const c_char) -> *mut SpoutBridgeHandle;

    /// `pixels` must point to at least `pitch * height` readable, initialized
    /// bytes. The bridge copies them (into a cached staging texture, or
    /// straight into the shared texture for `SendImage`) before returning,
    /// and retains no pointer afterward. `mode` selects the send strategy —
    /// see `SendMode` in `lib.rs` for the numeric mapping and
    /// `spout_bridge.h` for the staged return codes.
    pub fn spout_bridge_send_bgra_mode(
        handle: *mut SpoutBridgeHandle,
        pixels: *const u8,
        width: u32,
        height: u32,
        pitch: u32,
        mode: u32,
    ) -> i32;

    pub fn spout_bridge_destroy(handle: *mut SpoutBridgeHandle);
}
