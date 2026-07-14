use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{ApiBackend, CameraIndex, CameraInfo, RequestedFormat, RequestedFormatType};
use nokhwa::{Camera, NokhwaError};
use thiserror::Error;

use crate::frame::Frame;
use crate::selector::{DeviceId, device_line};

pub trait CaptureSource {
    fn next_frame(&mut self) -> Result<Frame, CaptureError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceInfo {
    pub index: u32,
    pub name: String,
    pub id: Option<DeviceId>,
}

#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("no capture devices found")]
    NoDevices,
    #[error("failed to open device {device}: {reason}")]
    OpenFailed { device: String, reason: String },
    #[error("failed to read frame: {reason}")]
    FrameRead { reason: String },
    #[error("unsupported camera pixel format: {format}")]
    FormatUnsupported { format: String },
    #[error("failed to query devices: {reason}")]
    QueryFailed { reason: String },
}

/// Converts tightly-packed RGB8 to tightly-packed BGRA8 with opaque alpha.
fn rgb_to_bgra(rgb: &[u8], width: u32, height: u32) -> Vec<u8> {
    let pixel_count =
        usize::try_from(width).unwrap_or(0).saturating_mul(usize::try_from(height).unwrap_or(0));
    let mut bgra = Vec::with_capacity(pixel_count.saturating_mul(4));

    for pixel in rgb.chunks_exact(3) {
        bgra.extend_from_slice(&[pixel[2], pixel[1], pixel[0], 255]);
    }

    bgra
}

fn to_device_info(position: u32, info: &CameraInfo) -> DeviceInfo {
    DeviceInfo { index: position, name: info.human_name(), id: DeviceId::new(&info.misc()) }
}

fn devices_from(infos: Vec<CameraInfo>) -> Result<Vec<DeviceInfo>, CaptureError> {
    if infos.is_empty() {
        return Err(CaptureError::NoDevices);
    }

    Ok(infos
        .iter()
        .enumerate()
        .map(|(position, info)| to_device_info(u32::try_from(position).unwrap_or(u32::MAX), info))
        .collect())
}

/// Picks the open path for a device: a stable id opens via
/// `AVCaptureDevice.deviceWithUniqueID` and survives index reshuffles; a
/// missing id falls back to the (possibly unstable) enumeration position.
fn camera_index(device: &DeviceInfo) -> CameraIndex {
    match &device.id {
        Some(id) => CameraIndex::String(id.as_str().to_owned()),
        None => CameraIndex::Index(device.index),
    }
}

fn open_failed(device: &DeviceInfo, error: NokhwaError) -> CaptureError {
    CaptureError::OpenFailed { device: device_line(device), reason: error.to_string() }
}

fn frame_read_failed(error: NokhwaError) -> CaptureError {
    CaptureError::FrameRead { reason: error.to_string() }
}

fn query_failed(error: NokhwaError) -> CaptureError {
    CaptureError::QueryFailed { reason: error.to_string() }
}

/// Ordered attempts for `Camera::new`: nokhwa treats `HighestFrameRate` as an
/// exact match, so a camera lacking that exact fps would otherwise fail to
/// open outright. Trying the exact fps first, then falling back to the
/// highest-resolution format, keeps the camera openable either way.
fn format_candidates(requested_fps: Option<u32>) -> Vec<RequestedFormatType> {
    match requested_fps {
        Some(fps) => {
            vec![
                RequestedFormatType::HighestFrameRate(fps),
                RequestedFormatType::AbsoluteHighestResolution,
            ]
        }
        None => vec![RequestedFormatType::AbsoluteHighestResolution],
    }
}

pub fn list_devices() -> Result<Vec<DeviceInfo>, CaptureError> {
    let infos = nokhwa::query(ApiBackend::Auto).map_err(query_failed)?;

    devices_from(infos)
}

pub struct NokhwaSource {
    camera: Camera,
}

impl NokhwaSource {
    /// Tries each candidate format in order (see `format_candidates`), opening the
    /// camera with the first one that succeeds. If every candidate fails, the last
    /// attempt's error is reported since it best reflects the final, most-relaxed
    /// request that the camera still refused.
    ///
    /// Callers resolve a `DeviceSelector` to a `DeviceInfo` first; this only opens
    /// the already-chosen device.
    pub fn open(device: &DeviceInfo, requested_fps: Option<u32>) -> Result<Self, CaptureError> {
        let mut attempts = format_candidates(requested_fps).into_iter();
        let Some(mut format_type) = attempts.next() else {
            return Err(CaptureError::OpenFailed {
                device: device_line(device),
                reason: "no capture format candidates".to_string(),
            });
        };

        loop {
            let requested = RequestedFormat::new::<RgbFormat>(format_type);
            match Camera::new(camera_index(device), requested) {
                Ok(mut camera) => {
                    camera.open_stream().map_err(|error| open_failed(device, error))?;
                    return Ok(Self { camera });
                }
                Err(error) => {
                    let Some(next_format) = attempts.next() else {
                        return Err(open_failed(device, error));
                    };
                    format_type = next_format;
                }
            }
        }
    }
}

impl CaptureSource for NokhwaSource {
    fn next_frame(&mut self) -> Result<Frame, CaptureError> {
        let buffer = self.camera.frame().map_err(frame_read_failed)?;
        let decoded = buffer.decode_image::<RgbFormat>().map_err(frame_read_failed)?;
        let width = decoded.width();
        let height = decoded.height();
        let bgra = rgb_to_bgra(decoded.as_raw(), width, height);

        Frame::new(width, height, bgra)
            .map_err(|error| CaptureError::FrameRead { reason: error.to_string() })
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CaptureError, CaptureSource, DeviceInfo, camera_index, devices_from, format_candidates,
        frame_read_failed, open_failed, query_failed, rgb_to_bgra, to_device_info,
    };
    use crate::frame::Frame;
    use crate::selector::DeviceId;

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
        let info = DeviceInfo { index: 2, name: "Logi C920".to_string(), id: None };

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
    fn to_device_info_carries_unique_id() {
        let info = nokhwa::utils::CameraInfo::new(
            "OBS Virtual Camera",
            "USB Video Class",
            "7626645E-1E13-4E6F-8B77-71B2A5B5F1C7",
            nokhwa::utils::CameraIndex::Index(1),
        );

        let device = to_device_info(0, &info);

        assert_eq!(
            device,
            DeviceInfo {
                index: 0,
                name: "OBS Virtual Camera".to_string(),
                id: DeviceId::new("7626645E-1E13-4E6F-8B77-71B2A5B5F1C7"),
            }
        );
    }

    #[test]
    fn to_device_info_empty_misc_is_none() {
        let info = nokhwa::utils::CameraInfo::new(
            "Logi C920",
            "USB Video Class",
            "",
            nokhwa::utils::CameraIndex::Index(0),
        );

        let device = to_device_info(0, &info);

        assert_eq!(device.id, None);
    }

    #[test]
    fn devices_from_empty_list_is_no_devices() {
        let result = devices_from(vec![]);

        assert!(matches!(result, Err(CaptureError::NoDevices)));
    }

    #[test]
    fn devices_from_assigns_positions() {
        let infos = vec![
            nokhwa::utils::CameraInfo::new(
                "Cam A",
                "",
                "id-a",
                nokhwa::utils::CameraIndex::String("id-a".to_string()),
            ),
            nokhwa::utils::CameraInfo::new(
                "Cam B",
                "",
                "id-b",
                nokhwa::utils::CameraIndex::String("id-b".to_string()),
            ),
        ];

        let devices = devices_from(infos).expect("non-empty list maps");

        assert_eq!(
            devices,
            vec![
                DeviceInfo { index: 0, name: "Cam A".to_string(), id: DeviceId::new("id-a") },
                DeviceInfo { index: 1, name: "Cam B".to_string(), id: DeviceId::new("id-b") },
            ]
        );
    }

    #[test]
    fn camera_index_prefers_id() {
        let device =
            DeviceInfo { index: 3, name: "Cam".to_string(), id: DeviceId::new("uuid-123") };

        let index = camera_index(&device);

        assert_eq!(index, nokhwa::utils::CameraIndex::String("uuid-123".to_string()));
    }

    #[test]
    fn camera_index_falls_back() {
        let device = DeviceInfo { index: 2, name: "Cam".to_string(), id: None };

        let index = camera_index(&device);

        assert_eq!(index, nokhwa::utils::CameraIndex::Index(2));
    }

    #[test]
    fn open_failed_labels_device() {
        let device =
            DeviceInfo { index: 0, name: "Logi C920".to_string(), id: DeviceId::new("uuid-1") };
        let error =
            nokhwa::NokhwaError::OpenDeviceError("0".to_string(), "device busy".to_string());

        let mapped = open_failed(&device, error);

        match mapped {
            CaptureError::OpenFailed { device: label, reason } => {
                assert!(label.contains("Logi C920"));
                assert!(label.contains("uuid-1"));
                assert!(reason.contains("device busy"));
            }
            other => panic!("expected OpenFailed, got {other:?}"),
        }
    }

    #[test]
    fn frame_read_failed_wraps_nokhwa_error_text() {
        let error = nokhwa::NokhwaError::ReadFrameError("timeout".to_string());

        let mapped = frame_read_failed(error);

        assert!(matches!(mapped, CaptureError::FrameRead { .. }));
    }

    #[test]
    fn query_failed_wraps_nokhwa_error_text() {
        let error = nokhwa::NokhwaError::GeneralError("permission denied".to_string());

        let mapped = query_failed(error);

        match mapped {
            CaptureError::QueryFailed { reason } => {
                assert!(reason.contains("permission denied"));
            }
            other => panic!("expected QueryFailed, got {other:?}"),
        }
    }

    #[test]
    fn format_candidates_prioritizes_resolution_when_fps_omitted() {
        let candidates = format_candidates(None);

        assert_eq!(candidates.len(), 1);
        assert!(matches!(
            candidates[0],
            nokhwa::utils::RequestedFormatType::AbsoluteHighestResolution
        ));
    }

    #[test]
    fn format_candidates_tries_exact_fps_before_resolution_fallback() {
        let candidates = format_candidates(Some(30));

        assert_eq!(candidates.len(), 2);
        assert!(matches!(candidates[0], nokhwa::utils::RequestedFormatType::HighestFrameRate(30)));
        assert!(matches!(
            candidates[1],
            nokhwa::utils::RequestedFormatType::AbsoluteHighestResolution
        ));
    }
}
