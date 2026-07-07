pub mod config;
pub mod crop;
pub mod flip;
pub mod rotate;
pub mod scale;

pub use config::{CropRect, Flip, Rotation, ScaleSpec, TransformConfig, TransformError};

use crate::frame::Frame;

pub fn apply(frame: &Frame, config: &TransformConfig) -> Result<Frame, TransformError> {
    let cropped = match config.crop {
        Some(rect) => crop::crop(frame, rect)?,
        None => frame.clone(),
    };
    let rotated = rotate::rotate(&cropped, config.rotation);
    let flipped = flip::flip(&rotated, config.flip);
    let scaled = match config.scale {
        Some(spec) => scale::scale(&flipped, spec)?,
        None => flipped,
    };
    Ok(scaled)
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
    fn apply_runs_crop_then_rotate_then_flip_then_scale() {
        let frame = sample_frame();
        let config = TransformConfig {
            crop: Some(CropRect { width: 2, height: 2, x: 0, y: 0 }),
            rotation: Rotation::R90,
            flip: Flip::Horizontal,
            scale: Some(ScaleSpec::Exact { width: 4, height: 4 }),
        };
        let result = apply(&frame, &config).unwrap();

        let a = [10, 20, 30, 255];
        let b = [11, 21, 31, 255];
        let c = [12, 22, 32, 255];
        let d = [13, 23, 33, 255];
        let expected_data = [
            a, a, c, c, // row0
            a, a, c, c, // row1
            b, b, d, d, // row2
            b, b, d, d, // row3
        ]
        .concat();
        let expected = Frame::new(4, 4, expected_data).unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn apply_with_default_config_is_identity() {
        let frame = sample_frame();
        let result = apply(&frame, &TransformConfig::default()).unwrap();
        assert_eq!(result, frame);
    }
}
