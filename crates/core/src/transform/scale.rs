//! Nearest-neighbor scaling. `ScaleSpec::Exact` picks the target size
//! directly; `ScaleSpec::Factor` multiplies both dimensions.

use crate::frame::Frame;
use crate::transform::config::{ScaleSpec, TransformError};

pub fn scale(frame: &Frame, spec: ScaleSpec) -> Result<Frame, TransformError> {
    let (width_out, height_out) = target_dims(frame, spec)?;
    let mut data = Vec::new();
    for y_out in 0..height_out {
        let y_src = nearest_index(y_out, height_out, frame.height());
        for x_out in 0..width_out {
            let x_src = nearest_index(x_out, width_out, frame.width());
            let pixel = frame.pixel(x_src, y_src).unwrap_or([0, 0, 0, 0]);
            data.extend_from_slice(&pixel);
        }
    }
    Ok(Frame::from_validated(width_out, height_out, data))
}

fn target_dims(frame: &Frame, spec: ScaleSpec) -> Result<(u32, u32), TransformError> {
    match spec {
        ScaleSpec::Exact { width, height } => Ok((width, height)),
        ScaleSpec::Factor(factor) => {
            let width = scale_dimension(frame.width(), factor);
            let height = scale_dimension(frame.height(), factor);
            Ok((width, height))
        }
    }
}

fn nearest_index(dst: u32, dst_len: u32, src_len: u32) -> u32 {
    let dst = u64::from(dst);
    let dst_len = u64::from(dst_len).max(1); // defensive: scale() only ever passes a validated non-zero dst_len
    let src_len = u64::from(src_len);
    let src = dst * src_len / dst_len;
    u32::try_from(src).unwrap_or(u32::MAX)
}

#[allow(clippy::as_conversions)]
fn scale_dimension(dim: u32, factor: f64) -> u32 {
    // u32 -> f64 is lossless; f64 -> u32 has no infallible std conversion
    // (TryFrom<f64> does not exist), so this is core's one deliberate `as`
    // cast, applied only after round() + clamp() bound the value to u32.
    let scaled = (dim as f64) * factor;
    scaled.round().clamp(1.0, u32::MAX as f64) as u32
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::Frame;

    fn sample_frame() -> Frame {
        let data = vec![
            10, 20, 30, 255, // (0,0) A
            11, 21, 31, 255, // (1,0) B
            12, 22, 32, 255, // (0,1) C
            13, 23, 33, 255, // (1,1) D
            14, 24, 34, 255, // (0,2) E
            15, 25, 35, 255, // (1,2) F
        ];
        Frame::new(2, 3, data).unwrap()
    }

    #[test]
    fn exact_upscale_duplicates_pixels() {
        let frame = sample_frame();
        let result = scale(&frame, ScaleSpec::Exact { width: 4, height: 6 }).unwrap();
        let a = [10, 20, 30, 255];
        let b = [11, 21, 31, 255];
        let c = [12, 22, 32, 255];
        let d = [13, 23, 33, 255];
        let e = [14, 24, 34, 255];
        let f = [15, 25, 35, 255];
        let expected_data = [
            a, a, b, b, // row0
            a, a, b, b, // row1
            c, c, d, d, // row2
            c, c, d, d, // row3
            e, e, f, f, // row4
            e, e, f, f, // row5
        ]
        .concat();
        let expected = Frame::new(4, 6, expected_data).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn factor_downscale_picks_nearest_source_pixel() {
        let frame = sample_frame();
        let result = scale(&frame, ScaleSpec::Factor(0.5)).unwrap();
        let expected = Frame::new(
            1,
            2,
            vec![
                10, 20, 30, 255, // (0,0) <- input (0,0) A
                12, 22, 32, 255, // (0,1) <- input (0,1) C
            ],
        )
        .unwrap();
        assert_eq!(result, expected);
    }
}
