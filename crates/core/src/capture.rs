use nokhwa::utils::{CameraIndex, CameraInfo};
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

#[allow(dead_code)]
fn index_number(index: &CameraIndex) -> u32 {
    index.as_index().unwrap_or(0)
}

#[allow(dead_code)]
fn to_device_info(info: &CameraInfo) -> DeviceInfo {
    DeviceInfo { index: index_number(info.index()), name: info.human_name() }
}

#[allow(dead_code)]
fn devices_from(infos: Vec<CameraInfo>) -> Result<Vec<DeviceInfo>, CaptureError> {
    if infos.is_empty() {
        return Err(CaptureError::NoDevices);
    }

    Ok(infos.iter().map(to_device_info).collect())
}

#[cfg(test)]
mod tests {
    use super::{
        CaptureError, CaptureSource, DeviceInfo, devices_from, index_number, rgb_to_bgra,
        to_device_info,
    };
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

    #[test]
    fn index_number_reads_numeric_index() {
        let index = nokhwa::utils::CameraIndex::Index(3);

        assert_eq!(index_number(&index), 3);
    }

    #[test]
    fn index_number_falls_back_to_zero_for_non_numeric_index() {
        let index = nokhwa::utils::CameraIndex::String("ipcam-1".to_string());

        assert_eq!(index_number(&index), 0);
    }

    #[test]
    fn to_device_info_copies_index_and_name() {
        let info = nokhwa::utils::CameraInfo::new(
            "Logi C920",
            "USB Video Class",
            "",
            nokhwa::utils::CameraIndex::Index(1),
        );

        let device = to_device_info(&info);

        assert_eq!(device, DeviceInfo { index: 1, name: "Logi C920".to_string() });
    }

    #[test]
    fn devices_from_empty_list_is_no_devices() {
        let result = devices_from(vec![]);

        assert!(matches!(result, Err(CaptureError::NoDevices)));
    }

    #[test]
    fn devices_from_maps_every_entry() {
        let infos = vec![
            nokhwa::utils::CameraInfo::new("Cam A", "", "", nokhwa::utils::CameraIndex::Index(0)),
            nokhwa::utils::CameraInfo::new("Cam B", "", "", nokhwa::utils::CameraIndex::Index(1)),
        ];

        let devices = devices_from(infos).expect("non-empty list maps");

        assert_eq!(
            devices,
            vec![
                DeviceInfo { index: 0, name: "Cam A".to_string() },
                DeviceInfo { index: 1, name: "Cam B".to_string() },
            ]
        );
    }
}
