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

    // The bridge also exports a plain `syphon_bridge_send_rgba` (see
    // cpp/syphon_bridge.h) that delegates to this entry with mode 1 — it
    // exists for non-Rust C callers. This crate always selects a mode
    // explicitly (`publish` picks `SendMode::PersistentCopy`), so only the
    // mode-parameterized entry is bound here; binding the plain one too
    // would leave it dead code with no Rust call site.
    /// `pixels` must point to at least `bytes_per_row * height` readable,
    /// initialized bytes. The bridge copies them into a bridge-owned
    /// IOSurface (cached across calls) before returning and retains no
    /// pointer afterward. `mode` selects the copy strategy; unknown values
    /// fall back to per-frame allocation.
    pub fn syphon_bridge_send_rgba_mode(
        handle: *mut SyphonBridgeHandle,
        pixels: *const u8,
        width: u32,
        height: u32,
        bytes_per_row: u32,
        mode: u32,
    ) -> bool;

    pub fn syphon_bridge_destroy(handle: *mut SyphonBridgeHandle);
}
