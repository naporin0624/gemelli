//! Spout DirectX publisher, Windows-only. The whole crate body is cfg-gated
//! below (not via a `[target.'cfg(...)'.dependencies]` table in downstream
//! Cargo.tomls) so release-please's Rust manifest updater — which cannot parse
//! `cfg()` target tables — can still bump this crate's version. Mirrors
//! crates/syphon.
#![cfg(target_os = "windows")]

mod ffi;

use std::ffi::CString;
use std::ptr::NonNull;

use gemelli_core::frame::Frame;
use gemelli_core::publish::{PublishError, TexturePublisher};

/// Sender-only Spout DirectX publisher. Wraps the opaque bridge handle
/// returned by `spout_bridge_create`.
pub struct SpoutPublisher {
    handle: NonNull<ffi::SpoutBridgeHandle>,
}

// SAFETY: `SpoutBridgeHandle` owns a `spoutDX` sender (its D3D11 device +
// immediate context). `SpoutPublisher` is not `Clone` and exposes no way to
// obtain a second handle to the same native object, so moving one to another
// thread (e.g. the capture/publish thread) never creates concurrent access
// from two threads at once. Mirrors SyphonPublisher.
unsafe impl Send for SpoutPublisher {}

impl SpoutPublisher {
    /// Creates a new Spout sender advertised under `server_name`.
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
}

impl TexturePublisher for SpoutPublisher {
    fn publish(&mut self, frame: &Frame) -> Result<(), PublishError> {
        let pitch = frame.width().checked_mul(4).ok_or_else(|| PublishError::Publish {
            reason: format!("frame width {} overflows pitch (width * 4)", frame.width()),
        })?;

        // SAFETY: `self.handle` was created by `spout_bridge_create` in `new`
        // and is not destroyed until `Drop::drop` (which takes `&mut self`, so
        // it cannot race this `&mut self` call). `frame.data()` is exactly
        // `width * height * 4` bytes (a `Frame` invariant), so `pitch * height`
        // never reads past its end. The bridge copies the pixels into its own
        // texture before returning, so no aliasing or use-after-free results.
        let ok = unsafe {
            ffi::spout_bridge_send_bgra(
                self.handle.as_ptr(),
                frame.data().as_ptr(),
                frame.width(),
                frame.height(),
                pitch,
            )
        };

        if ok {
            Ok(())
        } else {
            Err(PublishError::Publish {
                reason: "spout_bridge_send_bgra returned false".to_string(),
            })
        }
    }
}

impl Drop for SpoutPublisher {
    fn drop(&mut self) {
        // SAFETY: `self.handle` is the handle created in `new` and has not been
        // destroyed yet — `drop` runs at most once per `SpoutPublisher` and it
        // is not `Clone`, so no other reference to it can be live concurrently.
        unsafe { ffi::spout_bridge_destroy(self.handle.as_ptr()) };
    }
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
    #[ignore = "requires a real Windows GPU session; run manually with \
                `cargo test -p gemelli-spout -- --ignored` and observe the frame \
                in a Spout receiver (e.g. SpoutReceiver / OBS Spout2 source)"]
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
}
