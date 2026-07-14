//! Syphon Metal publisher, macOS-only. The whole crate body is cfg-gated
//! below (not via a `[target.'cfg(...)'.dependencies]` table in downstream
//! Cargo.tomls) so release-please's Rust manifest updater — which cannot
//! parse `cfg()` target tables — can still bump this crate's version.
#![cfg(target_os = "macos")]

mod ffi;
pub mod metrics;

use std::ffi::CString;
use std::ptr::NonNull;

use gemelli_core::frame::Frame;
use gemelli_core::publish::{PublishError, TexturePublisher};

/// CPU->Syphon copy strategy, selectable only for A/B benchmarking. The
/// numeric mapping matches the `mode` the bridge dispatches on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SendMode {
    /// Allocates a fresh IOSurface + MTLTexture every call.
    PerFrameCopy,
    /// Reuses a cached IOSurface + MTLTexture across calls, reallocated only
    /// on a geometry change. Production default (see `publish`).
    PersistentCopy,
}

impl SendMode {
    pub fn code(self) -> u32 {
        match self {
            Self::PerFrameCopy => 0,
            Self::PersistentCopy => 1,
        }
    }
}

/// Sender-only Syphon Metal publisher. Wraps the opaque bridge handle
/// returned by `syphon_bridge_create`.
pub struct SyphonPublisher {
    handle: NonNull<ffi::SyphonBridgeHandle>,
}

// SAFETY: `SyphonBridgeHandle` owns a `SyphonMetalServer`, an `MTLDevice`,
// an `MTLCommandQueue`, and a cached IOSurface/MTLTexture pair reused across
// publishes. `SyphonPublisher` is not `Clone` and exposes no way to obtain a
// second handle to the same native object, so moving one to another thread
// (e.g. handing it to a dedicated capture/publish thread) never creates
// concurrent access from two threads at once. This mirrors the reference
// bridge's own `unsafe impl Send for Sender`.
unsafe impl Send for SyphonPublisher {}

impl SyphonPublisher {
    /// Creates a new Syphon Metal server advertised under `server_name`.
    pub fn new(server_name: &str) -> Result<Self, PublishError> {
        let c_name = CString::new(server_name).map_err(|err| PublishError::ServerCreate {
            name: server_name.to_string(),
            reason: err.to_string(),
        })?;

        // SAFETY: `c_name` is a valid, NUL-terminated C string that stays
        // alive for the duration of this call (it is not dropped until this
        // statement completes). `syphon_bridge_create` copies the bytes into
        // an `NSString` before returning, so it never retains the pointer.
        let raw = unsafe { ffi::syphon_bridge_create(c_name.as_ptr()) };

        let handle = NonNull::new(raw).ok_or_else(|| PublishError::ServerCreate {
            name: server_name.to_string(),
            reason: "syphon_bridge_create returned a null handle".to_string(),
        })?;

        Ok(Self { handle })
    }

    /// Publishes `frame` using a specific copy strategy. Exposed for A/B
    /// benchmarking (see `examples/bench_syphon_cpu.rs`); production callers
    /// should use `publish`, which always selects `SendMode::PersistentCopy`.
    pub fn publish_mode(&mut self, frame: &Frame, mode: SendMode) -> Result<(), PublishError> {
        let bytes_per_row = frame.width().checked_mul(4).ok_or_else(|| PublishError::Publish {
            reason: format!("frame width {} overflows bytes-per-row (width * 4)", frame.width()),
        })?;

        // SAFETY: `self.handle` was created by `syphon_bridge_create` in
        // `new` and is not destroyed until `Drop::drop` runs (which takes
        // `&mut self`, so it cannot race with this `&mut self` call).
        // `frame.data()` is a valid `&[u8]` of exactly `width * height * 4`
        // bytes (a `Frame` invariant enforced by `Frame::new`), so
        // `bytes_per_row * height` never reads past its end. The bridge only
        // reads the buffer for the duration of this call — it copies pixels
        // into a bridge-owned IOSurface (cached across calls) before
        // returning — so no aliasing or use-after-free is possible once this
        // call returns.
        let ok = unsafe {
            ffi::syphon_bridge_send_rgba_mode(
                self.handle.as_ptr(),
                frame.data().as_ptr(),
                frame.width(),
                frame.height(),
                bytes_per_row,
                mode.code(),
            )
        };

        if ok {
            Ok(())
        } else {
            Err(PublishError::Publish {
                reason: "syphon_bridge_send_rgba_mode returned false".to_string(),
            })
        }
    }
}

impl TexturePublisher for SyphonPublisher {
    fn publish(&mut self, frame: &Frame) -> Result<(), PublishError> {
        self.publish_mode(frame, SendMode::PersistentCopy)
    }
}

impl Drop for SyphonPublisher {
    fn drop(&mut self) {
        // SAFETY: `self.handle` is the handle created in `new` and has not
        // been destroyed yet — `drop` runs at most once per `SyphonPublisher`
        // and `SyphonPublisher` is not `Clone`, so no other reference to it
        // can be live concurrently.
        unsafe { ffi::syphon_bridge_destroy(self.handle.as_ptr()) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_rejects_interior_nul() {
        let result = SyphonPublisher::new("bad\0name");

        assert!(matches!(result, Err(PublishError::ServerCreate { .. })));
    }

    #[test]
    fn send_mode_maps_to_bridge_dispatch_codes() {
        assert_eq!(SendMode::PerFrameCopy.code(), 0);
        assert_eq!(SendMode::PersistentCopy.code(), 1);
    }

    #[test]
    #[ignore = "requires a real macOS GPU session; run manually with --ignored"]
    fn publish_one_solid_color_frame() {
        use gemelli_core::frame::Frame;

        let width = 64_u32;
        let height = 64_u32;
        let pixel = [0_u8, 0, 255, 255]; // solid red, BGRA
        let len = usize::try_from(width)
            .and_then(|w| usize::try_from(height).map(|h| w * h * 4))
            .expect("64 * 64 * 4 fits in usize");
        let data = pixel.iter().copied().cycle().take(len).collect();
        let frame = Frame::new(width, height, data).expect("valid frame");

        let mut publisher = SyphonPublisher::new("gemelli-smoke-test").expect("server create");

        publisher.publish(&frame).expect("publish");

        // Give a receiving app a moment to observe the frame before the
        // server is torn down by `publisher`'s Drop at the end of this test.
        std::thread::sleep(std::time::Duration::from_secs(5));
    }

    #[test]
    #[ignore = "requires a real macOS GPU session; run manually with --ignored"]
    fn publish_frame_with_unaligned_row_stride() {
        use gemelli_core::frame::Frame;

        // width * 4 = 9832 bytes/row, not a multiple of 16 — reproduces the
        // reported Metal validation crash on a cropped, non-4-aligned width.
        let width = 2458_u32;
        let height = 100_u32;
        let pixel = [0_u8, 0, 255, 255]; // solid red, BGRA
        let len = usize::try_from(width)
            .and_then(|w| usize::try_from(height).map(|h| w * h * 4))
            .expect("2458 * 100 * 4 fits in usize");
        let data = pixel.iter().copied().cycle().take(len).collect();
        let frame = Frame::new(width, height, data).expect("valid frame");

        let mut publisher =
            SyphonPublisher::new("gemelli-smoke-test-unaligned-stride").expect("server create");

        publisher.publish(&frame).expect("publish");

        // Give a receiving app a moment to observe the frame before the
        // server is torn down by `publisher`'s Drop at the end of this test.
        std::thread::sleep(std::time::Duration::from_secs(5));
    }

    #[test]
    #[ignore = "requires a real macOS GPU session; run manually with --ignored"]
    fn publish_many_frames() {
        use gemelli_core::frame::Frame;

        let width = 64_u32;
        let height = 64_u32;
        let pixel = [0_u8, 0, 255, 255]; // solid red, BGRA
        let len = usize::try_from(width)
            .and_then(|w| usize::try_from(height).map(|h| w * h * 4))
            .expect("64 * 64 * 4 fits in usize");
        let data: Vec<u8> = pixel.iter().copied().cycle().take(len).collect();
        let frame = Frame::new(width, height, data).expect("valid frame");

        let mut publisher =
            SyphonPublisher::new("gemelli-smoke-test-many-frames").expect("server create");

        for _ in 0..300 {
            publisher.publish(&frame).expect("publish");
        }
    }

    #[test]
    #[ignore = "requires a real macOS GPU session; run manually with --ignored"]
    fn publish_after_resize() {
        use gemelli_core::frame::Frame;

        fn solid_frame(width: u32, height: u32) -> Frame {
            let pixel = [0_u8, 0, 255, 255]; // solid red, BGRA
            let len = usize::try_from(width)
                .and_then(|w| usize::try_from(height).map(|h| w * h * 4))
                .expect("dimensions fit in usize");
            let data: Vec<u8> = pixel.iter().copied().cycle().take(len).collect();
            Frame::new(width, height, data).expect("valid frame")
        }

        let mut publisher =
            SyphonPublisher::new("gemelli-smoke-test-resize").expect("server create");

        // 640x480 -> 2458x100 (unaligned pitch) -> 640x480: exercises cache
        // reallocation both growing and shrinking, including the aligned-
        // pitch path for a non-4-aligned width.
        publisher.publish(&solid_frame(640, 480)).expect("publish 640x480");
        publisher.publish(&solid_frame(2458, 100)).expect("publish 2458x100");
        publisher.publish(&solid_frame(640, 480)).expect("publish 640x480 again");
    }
}
