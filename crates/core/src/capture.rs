use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{
    ApiBackend, CameraFormat, CameraIndex, CameraInfo, FrameFormat, RequestedFormat,
    RequestedFormatType,
};
use nokhwa::{Camera, NokhwaError};
use thiserror::Error;

use crate::frame::Frame;
use crate::selector::{DeviceId, device_line};

/// Resolution cap for the auto-selected MJPEG format. High frame rate is
/// preferred over resolution, so we never pick a huge low-fps mode.
const MAX_MJPEG_WIDTH: u32 = 1920;
const MAX_MJPEG_HEIGHT: u32 = 1080;

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
/// Preallocates the output and writes each pixel by index so the swizzle
/// vectorizes, instead of pushing a fresh 4-byte array per pixel.
fn rgb_to_bgra(rgb: &[u8], width: u32, height: u32) -> Vec<u8> {
    let pixel_count =
        usize::try_from(width).unwrap_or(0).saturating_mul(usize::try_from(height).unwrap_or(0));
    let mut bgra = vec![0_u8; pixel_count.saturating_mul(4)];

    for (src, dst) in rgb.chunks_exact(3).zip(bgra.chunks_exact_mut(4)) {
        dst[0] = src[2];
        dst[1] = src[1];
        dst[2] = src[0];
        dst[3] = 255;
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

/// Chooses the best MJPEG format for high throughput: MJPEG only, within the
/// resolution cap, preferring the highest frame rate (tie-break: larger area).
/// With `requested_fps`, prefers the frame rate closest to it instead. Returns
/// `None` when the camera exposes no MJPEG format within the cap.
fn select_mjpeg_format(
    formats: &[CameraFormat],
    requested_fps: Option<u32>,
    max_width: u32,
    max_height: u32,
) -> Option<CameraFormat> {
    let area = |f: &CameraFormat| u64::from(f.width()) * u64::from(f.height());

    formats
        .iter()
        .copied()
        .filter(|f| f.format() == FrameFormat::MJPEG)
        .filter(|f| f.width() <= max_width && f.height() <= max_height)
        .max_by(|a, b| match requested_fps {
            Some(fps) => {
                // Closest frame rate wins; larger area breaks ties.
                b.frame_rate()
                    .abs_diff(fps)
                    .cmp(&a.frame_rate().abs_diff(fps))
                    .then(area(a).cmp(&area(b)))
            }
            None => a.frame_rate().cmp(&b.frame_rate()).then(area(a).cmp(&area(b))),
        })
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
    /// request that the camera still refused. Once open, it switches to a
    /// high-frame-rate MJPEG format when available (see `select_mjpeg_format`)
    /// before starting the stream — best-effort, so a camera that cannot
    /// enumerate or refuses the switch keeps the format it opened with.
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

        // Open the camera first (any working format) so we can enumerate its
        // real formats, then switch to a fast MJPEG one before streaming.
        let mut camera = loop {
            let requested = RequestedFormat::new::<RgbFormat>(format_type);
            match Camera::new(camera_index(device), requested) {
                Ok(camera) => break camera,
                Err(error) => {
                    let Some(next_format) = attempts.next() else {
                        return Err(open_failed(device, error));
                    };
                    format_type = next_format;
                }
            }
        };

        // Prefer a high-frame-rate MJPEG format: nokhwa's uncompressed (YUYV)
        // decode path is ~15x slower than its MJPEG (mozjpeg) path. Best-effort
        // — if the camera cannot enumerate or refuses the switch, keep the
        // format it opened with rather than failing the whole open.
        if let Ok(formats) = camera.compatible_camera_formats()
            && let Some(best) =
                select_mjpeg_format(&formats, requested_fps, MAX_MJPEG_WIDTH, MAX_MJPEG_HEIGHT)
        {
            let request = RequestedFormat::new::<RgbFormat>(RequestedFormatType::Exact(best));
            if camera.set_camera_requset(request).is_err() {
                // Camera refused the MJPEG switch; the opened format stays.
            }
        }

        camera.open_stream().map_err(|error| open_failed(device, error))?;
        Ok(Self { camera })
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
    use nokhwa::utils::Resolution;

    use super::{
        CameraFormat, CaptureError, CaptureSource, DeviceInfo, FrameFormat, MAX_MJPEG_HEIGHT,
        MAX_MJPEG_WIDTH, camera_index, devices_from, format_candidates, frame_read_failed,
        open_failed, query_failed, rgb_to_bgra, select_mjpeg_format, to_device_info,
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

    #[test]
    fn rgb_to_bgra_handles_multiple_rows_and_returns_exact_length() {
        // 2x2: four distinct RGB pixels, row-major.
        let rgb = vec![
            1, 2, 3, 4, 5, 6, // row 0: (R1 G2 B3) (R4 G5 B6)
            7, 8, 9, 10, 11, 12, // row 1: (R7 G8 B9) (R10 G11 B12)
        ];

        let bgra = rgb_to_bgra(&rgb, 2, 2);

        assert_eq!(bgra.len(), 2 * 2 * 4);
        assert_eq!(
            bgra,
            vec![
                3, 2, 1, 255, 6, 5, 4, 255, // row 0 → BGRA
                9, 8, 7, 255, 12, 11, 10, 255, // row 1 → BGRA
            ]
        );
    }

    #[test]
    #[ignore = "micro-benchmark; run manually with `cargo test -p gemelli-core \
                bench_rgb_to_bgra -- --ignored --nocapture`"]
    fn bench_rgb_to_bgra() {
        let (w, h) = (1920_u32, 1080_u32);
        let len = usize::try_from(w).unwrap() * usize::try_from(h).unwrap() * 3;
        let rgb = vec![128_u8; len];

        let start = std::time::Instant::now();
        let iters = 100;
        for _ in 0..iters {
            let out = rgb_to_bgra(&rgb, w, h);
            std::hint::black_box(&out);
        }
        let per = start.elapsed().as_secs_f64() * 1000.0 / f64::from(iters);
        println!("rgb_to_bgra {w}x{h}: {per:.2} ms/frame");
    }

    fn fmt(w: u32, h: u32, format: FrameFormat, fps: u32) -> CameraFormat {
        CameraFormat::new(Resolution::new(w, h), format, fps)
    }

    #[test]
    fn select_prefers_highest_frame_rate_mjpeg_when_no_fps_requested() {
        let formats = vec![
            fmt(1920, 1080, FrameFormat::MJPEG, 30),
            fmt(1280, 720, FrameFormat::MJPEG, 60),
            fmt(1920, 1080, FrameFormat::YUYV, 60),
        ];
        let chosen = select_mjpeg_format(&formats, None, MAX_MJPEG_WIDTH, MAX_MJPEG_HEIGHT)
            .expect("an MJPEG format is available");
        assert_eq!(chosen.frame_rate(), 60);
        assert_eq!(chosen.format(), FrameFormat::MJPEG);
        assert_eq!((chosen.width(), chosen.height()), (1280, 720));
    }

    #[test]
    fn select_breaks_frame_rate_ties_on_larger_resolution() {
        let formats =
            vec![fmt(1280, 720, FrameFormat::MJPEG, 60), fmt(1920, 1080, FrameFormat::MJPEG, 60)];
        let chosen =
            select_mjpeg_format(&formats, None, MAX_MJPEG_WIDTH, MAX_MJPEG_HEIGHT).unwrap();
        assert_eq!((chosen.width(), chosen.height()), (1920, 1080));
    }

    #[test]
    fn select_excludes_mjpeg_above_the_resolution_cap() {
        let formats = vec![fmt(2304, 1296, FrameFormat::MJPEG, 30)];
        assert_eq!(select_mjpeg_format(&formats, None, MAX_MJPEG_WIDTH, MAX_MJPEG_HEIGHT), None);
    }

    #[test]
    fn select_returns_none_when_no_mjpeg_present() {
        let formats = vec![fmt(1920, 1080, FrameFormat::YUYV, 60)];
        assert_eq!(select_mjpeg_format(&formats, None, MAX_MJPEG_WIDTH, MAX_MJPEG_HEIGHT), None);
    }

    #[test]
    fn select_picks_closest_frame_rate_to_requested_fps() {
        let formats = vec![
            fmt(1920, 1080, FrameFormat::MJPEG, 60),
            fmt(1920, 1080, FrameFormat::MJPEG, 30),
            fmt(1280, 720, FrameFormat::MJPEG, 24),
        ];
        let chosen =
            select_mjpeg_format(&formats, Some(30), MAX_MJPEG_WIDTH, MAX_MJPEG_HEIGHT).unwrap();
        assert_eq!(chosen.frame_rate(), 30);
    }
}
