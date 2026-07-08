use thiserror::Error;

use crate::frame::Frame;

pub trait CaptureSource {
    fn next_frame(&mut self) -> Result<Frame, CaptureError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub index: u32,
    pub name: String,
}

#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("no capture devices found")]
    NoDevices,
    #[error("device index {index} not found ({available} devices available)")]
    DeviceNotFound { index: u32, available: usize },
    #[error("failed to open device {index}: {reason}")]
    OpenFailed { index: u32, reason: String },
    #[error("failed to read frame: {reason}")]
    FrameRead { reason: String },
    #[error("unsupported camera pixel format: {format}")]
    FormatUnsupported { format: String },
}

/// Converts tightly-packed RGB8 to tightly-packed BGRA8 with opaque alpha.
#[allow(dead_code)]
fn rgb_to_bgra(rgb: &[u8], width: u32, height: u32) -> Vec<u8> {
    let pixel_count =
        usize::try_from(width).unwrap_or(0).saturating_mul(usize::try_from(height).unwrap_or(0));
    let mut bgra = Vec::with_capacity(pixel_count.saturating_mul(4));

    for pixel in rgb.chunks_exact(3) {
        bgra.extend_from_slice(&[pixel[2], pixel[1], pixel[0], 255]);
    }

    bgra
}

#[cfg(test)]
mod tests {
    use super::{CaptureError, CaptureSource, DeviceInfo, rgb_to_bgra};
    use crate::frame::Frame;

    struct RecordingSource {
        frames: Vec<Frame>,
        calls: u32,
    }

    impl CaptureSource for RecordingSource {
        fn next_frame(&mut self) -> Result<Frame, CaptureError> {
            self.calls += 1;
            self.frames
                .pop()
                .ok_or_else(|| CaptureError::FrameRead { reason: "exhausted".to_string() })
        }
    }

    #[test]
    fn dyn_capture_source_returns_recorded_frame() {
        let frame = Frame::new(1, 1, vec![10, 20, 30, 255]).expect("valid frame");
        let mut recording = RecordingSource { frames: vec![frame.clone()], calls: 0 };
        // This binding is the object-safety proof: CaptureSource has no generic
        // methods and takes `&mut self`, so it coerces to a trait object.
        let source: &mut dyn CaptureSource = &mut recording;

        let result = source.next_frame().expect("frame available");

        assert_eq!(result, frame);
        assert_eq!(recording.calls, 1);
    }

    #[test]
    fn device_info_holds_index_and_name() {
        let info = DeviceInfo { index: 2, name: "Logi C920".to_string() };

        assert_eq!(info.index, 2);
        assert_eq!(info.name, "Logi C920");
    }

    #[test]
    fn rgb_to_bgra_swizzles_channels_and_adds_opaque_alpha() {
        let rgb = vec![
            10, 20, 30, // pixel (0,0): R=10 G=20 B=30
            40, 50, 60, // pixel (1,0): R=40 G=50 B=60
        ];

        let bgra = rgb_to_bgra(&rgb, 2, 1);

        assert_eq!(bgra, vec![30, 20, 10, 255, 60, 50, 40, 255]);
    }
}
