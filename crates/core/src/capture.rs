use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{
    ApiBackend, CameraFormat, CameraIndex, CameraInfo, FrameFormat, RequestedFormat,
    RequestedFormatType,
};
use nokhwa::{Camera, NokhwaError};
use thiserror::Error;

use crate::frame::Frame;

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

fn index_number(index: &CameraIndex) -> u32 {
    index.as_index().unwrap_or(0)
}

fn to_device_info(info: &CameraInfo) -> DeviceInfo {
    DeviceInfo { index: index_number(info.index()), name: info.human_name() }
}

fn devices_from(infos: Vec<CameraInfo>) -> Result<Vec<DeviceInfo>, CaptureError> {
    if infos.is_empty() {
        return Err(CaptureError::NoDevices);
    }

    Ok(infos.iter().map(to_device_info).collect())
}

fn open_failed(index: u32, error: NokhwaError) -> CaptureError {
    CaptureError::OpenFailed { index, reason: error.to_string() }
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
    pub fn open(index: u32, requested_fps: Option<u32>) -> Result<Self, CaptureError> {
        let mut attempts = format_candidates(requested_fps).into_iter();
        let Some(mut format_type) = attempts.next() else {
            return Err(CaptureError::OpenFailed {
                index,
                reason: "no capture format candidates".to_string(),
            });
        };

        // Open the camera first (any working format) so we can enumerate its
        // real formats, then switch to a fast MJPEG one before streaming.
        let mut camera = loop {
            let requested = RequestedFormat::new::<RgbFormat>(format_type);
            match Camera::new(CameraIndex::Index(index), requested) {
                Ok(camera) => break camera,
                Err(error) => {
                    let Some(next_format) = attempts.next() else {
                        return Err(open_failed(index, error));
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

        camera.open_stream().map_err(|error| open_failed(index, error))?;
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
        MAX_MJPEG_WIDTH, devices_from, format_candidates, frame_read_failed, index_number,
        open_failed, query_failed, rgb_to_bgra, select_mjpeg_format, to_device_info,
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

    #[test]
    fn open_failed_wraps_nokhwa_error_text() {
        let error =
            nokhwa::NokhwaError::OpenDeviceError("0".to_string(), "device busy".to_string());

        let mapped = open_failed(0, error);

        assert!(matches!(mapped, CaptureError::OpenFailed { index: 0, .. }));
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

    #[test]
    #[ignore = "requires a real camera; run manually with \
                `cargo test -p gemelli-core opens_camera_as_mjpeg -- --ignored --nocapture`"]
    fn opens_camera_as_mjpeg() {
        let source = super::NokhwaSource::open(0, None).expect("camera opens");
        let format = source.camera.camera_format();
        println!(
            "negotiated: {}x{} @ {}fps {:?}",
            format.width(),
            format.height(),
            format.frame_rate(),
            format.format()
        );
        assert_eq!(format.format(), nokhwa::utils::FrameFormat::MJPEG);
    }
}
