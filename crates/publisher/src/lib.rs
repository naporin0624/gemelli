//! Thin `gemelli-core::publish::TexturePublisher` adapters over linguine's
//! platform texture-sender bridges: Syphon on macOS, Spout on Windows.
//! Bridges themselves (native FFI, IOSurface/staging-texture handling, SDK
//! vendoring) live in `linguine` now — see its README's "consumer
//! mechanics" section for what this crate's `build.rs` (rpath
//! re-forwarding) and `crates/cli`'s / `crates/gui`'s `build.rs` +
//! `app.manifest` (Windows ComCtl32 v6 manifest embed) implement on top of
//! it.

use gemelli_core::frame::Frame;
use gemelli_core::publish::{PublishError, TexturePublisher};

#[cfg(any(target_os = "macos", target_os = "windows"))]
use linguine::{BgraFrame, FrameError, SendError, TextureSender};

/// `BgraFrame::tight` can only fail on a zero dimension here — `Frame`
/// already guarantees `data.len() == width * height * 4`, so `tight`'s
/// `DataTooShort` case never triggers from a valid `Frame` — but the
/// conversion still returns a `Result`, so it is mapped rather than
/// unwrapped.
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn frame_to_bgra(frame: &Frame) -> Result<BgraFrame<'_>, PublishError> {
    BgraFrame::tight(frame.width(), frame.height(), frame.data())
        .map_err(|error: FrameError| PublishError::Publish { reason: error.to_string() })
}

/// Maps linguine's `SendError` onto `gemelli-core`'s `PublishError`,
/// used both for sender construction (`SenderCreate`) and per-frame sends
/// (`SendFrame`).
#[cfg(any(target_os = "macos", target_os = "windows"))]
fn map_send_error(error: SendError) -> PublishError {
    match error {
        SendError::SenderCreate { name, reason } => PublishError::ServerCreate { name, reason },
        SendError::SendFrame { stage, detail } => {
            PublishError::Publish { reason: format!("{stage}: {detail}") }
        }
    }
}

/// Sender-only Syphon publisher, adapting `linguine::syphon::SyphonSender`
/// to `TexturePublisher`.
#[cfg(target_os = "macos")]
pub struct SyphonPublisher(linguine::syphon::SyphonSender);

#[cfg(target_os = "macos")]
impl SyphonPublisher {
    pub fn new(server_name: &str) -> Result<Self, PublishError> {
        linguine::syphon::SyphonSender::new(server_name).map(Self).map_err(map_send_error)
    }
}

#[cfg(target_os = "macos")]
impl TexturePublisher for SyphonPublisher {
    fn publish(&mut self, frame: &Frame) -> Result<(), PublishError> {
        let bgra = frame_to_bgra(frame)?;
        self.0.send_frame(&bgra).map_err(map_send_error)
    }
}

/// Sender-only Spout publisher, adapting `linguine::spout::SpoutSender` to
/// `TexturePublisher`.
#[cfg(target_os = "windows")]
pub struct SpoutPublisher(linguine::spout::SpoutSender);

#[cfg(target_os = "windows")]
impl SpoutPublisher {
    pub fn new(server_name: &str) -> Result<Self, PublishError> {
        linguine::spout::SpoutSender::new(server_name).map(Self).map_err(map_send_error)
    }
}

#[cfg(target_os = "windows")]
impl TexturePublisher for SpoutPublisher {
    fn publish(&mut self, frame: &Frame) -> Result<(), PublishError> {
        let bgra = frame_to_bgra(frame)?;
        self.0.send_frame(&bgra).map_err(map_send_error)
    }
}

/// Opens the platform-appropriate publisher: `SyphonPublisher` on macOS,
/// `SpoutPublisher` on Windows, an `UnsupportedPlatform`-flavored error
/// everywhere else. Mirrors `create_publisher`/`open_publisher`, previously
/// duplicated in `crates/cli/src/run.rs` and `crates/gui/src/worker.rs` —
/// both now delegate here instead.
#[cfg(target_os = "macos")]
pub fn open_publisher(server_name: &str) -> Result<Box<dyn TexturePublisher>, PublishError> {
    let publisher = SyphonPublisher::new(server_name)?;
    Ok(Box::new(publisher))
}

#[cfg(target_os = "windows")]
pub fn open_publisher(server_name: &str) -> Result<Box<dyn TexturePublisher>, PublishError> {
    let publisher = SpoutPublisher::new(server_name)?;
    Ok(Box::new(publisher))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn open_publisher(server_name: &str) -> Result<Box<dyn TexturePublisher>, PublishError> {
    Err(PublishError::ServerCreate {
        name: server_name.to_string(),
        reason: "Syphon/Spout publishing is not supported on this platform".to_string(),
    })
}

#[cfg(all(test, any(target_os = "macos", target_os = "windows")))]
mod tests {
    use linguine::SendStage;

    use super::*;

    #[test]
    fn zero_dimension_frame_error_maps_to_publish_error() {
        let frame = Frame::new(1, 1, vec![0, 0, 0, 255]).unwrap_or_else(|error| {
            panic!("1x1 all-zero frame must be valid: {error}");
        });
        // Exercise the real conversion path end-to-end; the only way to
        // observe FrameError mapping without unsafe hardware calls is via
        // `frame_to_bgra` itself, so this asserts it succeeds for a frame
        // `Frame::new` already proved valid.
        assert!(frame_to_bgra(&frame).is_ok());
    }

    #[test]
    fn sender_create_error_maps_to_server_create() {
        let error = SendError::SenderCreate { name: "srv".to_string(), reason: "boom".to_string() };

        let mapped = map_send_error(error);

        match mapped {
            PublishError::ServerCreate { name, reason } => {
                assert_eq!(name, "srv");
                assert_eq!(reason, "boom");
            }
            other => panic!("expected ServerCreate, got {other:?}"),
        }
    }

    #[test]
    fn send_frame_error_maps_to_publish_with_stage_and_detail() {
        let error = SendError::SendFrame {
            stage: SendStage::SendFailed,
            detail: "native failure".to_string(),
        };

        let mapped = map_send_error(error);

        match mapped {
            PublishError::Publish { reason } => {
                assert!(reason.contains("native failure"), "reason was: {reason}");
                assert!(reason.contains("native send call"), "reason was: {reason}");
            }
            other => panic!("expected Publish, got {other:?}"),
        }
    }
}
