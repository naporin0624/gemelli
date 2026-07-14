//! Spout DirectX publisher, Windows-only. The whole crate body is cfg-gated
//! below (not via a `[target.'cfg(...)'.dependencies]` table in downstream
//! Cargo.tomls) so release-please's Rust manifest updater — which cannot parse
//! `cfg()` target tables — can still bump this crate's version. Mirrors
//! crates/syphon.
#![cfg(target_os = "windows")]

mod ffi;
pub mod metrics;

use std::ffi::CString;
use std::ptr::NonNull;

use gemelli_core::frame::Frame;
use gemelli_core::publish::{PublishError, TexturePublisher};

/// CPU->Spout send strategy, selectable via `publish_mode` for A/B
/// benchmarking (see `examples/bench_spout_cpu.rs`). The numeric mapping
/// (`code`) matches the `mode` argument the C++ bridge
/// (`spout_bridge_send_bgra_mode`) dispatches on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendMode {
    /// DYNAMIC staging texture, filled with a row-by-row `memcpy` per
    /// scanline, then `SpoutDX::SendTexture`.
    StagingRowCopy,
    /// `SpoutDX::SendImage`, which uploads the CPU buffer straight into the
    /// shared texture via `UpdateSubresource` (no staging texture). This is
    /// the mode E2E-verified against a real Spout receiver on Windows
    /// hardware in gemelli-spout's original hardware-tested prototype; kept
    /// selectable for A/B runs alongside the newer strategies below.
    SendImage,
    /// DYNAMIC staging texture, filled with `SpoutCopy`'s SSE2 pitch-aware
    /// line copier instead of a scalar `memcpy` loop, then
    /// `SpoutDX::SendTexture`. Production default (see `publish`): fastest
    /// CPU time at 4K — where sustaining 60fps is hardest — and ties
    /// `StagingRowCopy` within noise at 1080p, per the reference
    /// implementation's benchmarks.
    StagingSse,
}

impl SendMode {
    pub fn code(self) -> u32 {
        match self {
            Self::StagingRowCopy => 0,
            Self::SendImage => 1,
            Self::StagingSse => 2,
        }
    }
}

/// Sender-only Spout DirectX publisher. Wraps the opaque bridge handle
/// returned by `spout_bridge_create`.
pub struct SpoutPublisher {
    handle: NonNull<ffi::SpoutBridgeHandle>,
}

// SAFETY: `SpoutBridgeHandle` owns a `spoutDX` sender (its D3D11 device and
// immediate context) plus a cached staging texture reused across sends.
// `SpoutPublisher` is not `Clone` and exposes no way to obtain a second
// handle to the same native object, so moving one to another thread (e.g.
// the capture/publish thread) never creates concurrent access from two
// threads at once. Mirrors SyphonPublisher.
unsafe impl Send for SpoutPublisher {}

impl SpoutPublisher {
    /// Creates a new Spout sender advertised under `server_name`.
    ///
    /// Spout sender names live in a fixed 256-byte buffer inside the SDK
    /// (`spoutSenderNames`); names longer than that are truncated by the SDK
    /// before being published, not by this binding.
    pub fn new(server_name: &str) -> Result<Self, PublishError> {
        let c_name = CString::new(server_name).map_err(|err| PublishError::ServerCreate {
            name: server_name.to_string(),
            reason: err.to_string(),
        })?;

        // SAFETY: `c_name` is a valid, NUL-terminated C string alive for the
        // duration of this call. `spout_bridge_create` copies the name into
        // its own sender before returning; it retains no pointer.
        let raw = unsafe { ffi::spout_bridge_create(c_name.as_ptr()) };

        let handle = NonNull::new(raw).ok_or_else(|| PublishError::ServerCreate {
            name: server_name.to_string(),
            reason: "spout_bridge_create returned a null handle".to_string(),
        })?;

        Ok(Self { handle })
    }

    /// Publishes `frame` using a specific send strategy. Exposed for A/B
    /// benchmarking; production callers should use `publish`, which always
    /// selects `SendMode::StagingSse`.
    pub fn publish_mode(&mut self, frame: &Frame, mode: SendMode) -> Result<(), PublishError> {
        let pitch = frame.width().checked_mul(4).ok_or_else(|| PublishError::Publish {
            reason: format!("frame width {} overflows pitch (width * 4)", frame.width()),
        })?;

        // SAFETY: `self.handle` was created by `spout_bridge_create` in `new`
        // and is not destroyed until `Drop::drop` (which takes `&mut self`,
        // so it cannot race this `&mut self` call). `frame.data()` is exactly
        // `width * height * 4` bytes (a `Frame` invariant), so
        // `pitch * height` never reads past its end. The bridge copies the
        // pixels into its own staging texture (or straight into the shared
        // texture for `SendImage`) before returning, so no aliasing or
        // use-after-free results.
        let rc = unsafe {
            ffi::spout_bridge_send_bgra_mode(
                self.handle.as_ptr(),
                frame.data().as_ptr(),
                frame.width(),
                frame.height(),
                pitch,
                mode.code(),
            )
        };

        if rc == 0 {
            Ok(())
        } else {
            Err(PublishError::Publish { reason: describe_bridge_error(rc) })
        }
    }
}

impl TexturePublisher for SpoutPublisher {
    fn publish(&mut self, frame: &Frame) -> Result<(), PublishError> {
        self.publish_mode(frame, SendMode::StagingSse)
    }
}

impl Drop for SpoutPublisher {
    fn drop(&mut self) {
        // SAFETY: `self.handle` is the handle created in `new` and has not
        // been destroyed yet — `drop` runs at most once per `SpoutPublisher`
        // and it is not `Clone`, so no other reference to it can be live
        // concurrently.
        unsafe { ffi::spout_bridge_destroy(self.handle.as_ptr()) };
    }
}

/// Maps a `spout_bridge_send_bgra_mode` return code (see `spout_bridge.h`)
/// to a message naming the failing stage, so a `PublishError::Publish`
/// reason reported from the field is enough to tell which part of the
/// native bridge failed without reproducing on Windows hardware.
fn describe_bridge_error(code: i32) -> String {
    let stage = match code {
        -1 => "bad arguments (null handle/pixels, zero width/height, or pitch < width*4)",
        -2 => "bridge handle not initialized (DirectX11/sender setup incomplete)",
        -3 => "staging texture allocation failed",
        -4 => "Map() of the staging texture failed",
        -5 => "the underlying SpoutDX send call failed",
        _ => "unknown bridge error",
    };
    format!("spout_bridge_send_bgra_mode returned {code} ({stage})")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_rejects_interior_nul() {
        let result = SpoutPublisher::new("bad\0name");

        assert!(matches!(result, Err(PublishError::ServerCreate { .. })));
    }

    #[test]
    fn send_mode_maps_to_bridge_dispatch_codes() {
        assert_eq!(SendMode::StagingRowCopy.code(), 0);
        assert_eq!(SendMode::SendImage.code(), 1);
        assert_eq!(SendMode::StagingSse.code(), 2);
    }

    #[test]
    #[ignore = "requires a Windows machine with a Spout receiver"]
    fn publish_one_solid_color_frame() {
        let width = 64_u32;
        let height = 64_u32;
        let pixel = [0_u8, 0, 255, 255]; // solid red, BGRA
        let len = usize::try_from(width)
            .and_then(|w| usize::try_from(height).map(|h| w * h * 4))
            .expect("64 * 64 * 4 fits in usize");
        let data = pixel.iter().copied().cycle().take(len).collect();
        let frame = Frame::new(width, height, data).expect("valid frame");

        let mut publisher = SpoutPublisher::new("gemelli-spout-smoke").expect("sender create");
        publisher.publish(&frame).expect("publish");

        // Give a receiver a moment to observe the frame before Drop tears the
        // sender down.
        std::thread::sleep(std::time::Duration::from_secs(5));
    }

    #[test]
    #[ignore = "requires a Windows machine with a Spout receiver"]
    fn publish_frame_with_unaligned_row_stride() {
        // width * 4 = 9832 bytes/row, not a multiple of 16 — mirrors the
        // syphon crate's regression case for a cropped, non-4-aligned width.
        let width = 2458_u32;
        let height = 100_u32;
        let pixel = [0_u8, 0, 255, 255]; // solid red, BGRA
        let len = usize::try_from(width)
            .and_then(|w| usize::try_from(height).map(|h| w * h * 4))
            .expect("2458 * 100 * 4 fits in usize");
        let data = pixel.iter().copied().cycle().take(len).collect();
        let frame = Frame::new(width, height, data).expect("valid frame");

        let mut publisher =
            SpoutPublisher::new("gemelli-spout-smoke-unaligned-stride").expect("sender create");
        publisher.publish(&frame).expect("publish");

        std::thread::sleep(std::time::Duration::from_secs(5));
    }

    #[test]
    #[ignore = "requires a Windows machine with a Spout receiver"]
    fn publish_after_resize() {
        fn solid_frame(width: u32, height: u32) -> Frame {
            let pixel = [0_u8, 0, 255, 255]; // solid red, BGRA
            let len = usize::try_from(width)
                .and_then(|w| usize::try_from(height).map(|h| w * h * 4))
                .expect("dimensions fit in usize");
            let data: Vec<u8> = pixel.iter().copied().cycle().take(len).collect();
            Frame::new(width, height, data).expect("valid frame")
        }

        let mut publisher =
            SpoutPublisher::new("gemelli-spout-smoke-resize").expect("sender create");

        // 640x480 -> 2458x100 (unaligned pitch) -> 640x480: exercises staging
        // texture reallocation both growing and shrinking.
        publisher.publish(&solid_frame(640, 480)).expect("publish 640x480");
        publisher.publish(&solid_frame(2458, 100)).expect("publish 2458x100");
        publisher.publish(&solid_frame(640, 480)).expect("publish 640x480 again");
    }
}
