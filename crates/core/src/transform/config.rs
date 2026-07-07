#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Rotation {
    #[default]
    R0,
    R90,
    R180,
    R270,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Flip {
    #[default]
    Keep,
    Horizontal,
    Vertical,
    Both,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CropRect {
    pub width: u32,
    pub height: u32,
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScaleSpec {
    Exact { width: u32, height: u32 },
    Factor(f64),
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TransformConfig {
    pub crop: Option<CropRect>,
    pub rotation: Rotation,
    pub flip: Flip,
    pub scale: Option<ScaleSpec>,
}

// NOTE: `Eq` intentionally dropped vs. the interface contract's literal
// derive list — `ScaleFactorInvalid.factor: f64` cannot implement `Eq`
// (NaN != NaN), so `#[derive(Eq)]` here would not compile.
#[derive(Debug, PartialEq, thiserror::Error)]
pub enum TransformError {
    #[error("crop rect {width}x{height}+{x}+{y} exceeds frame bounds {frame_width}x{frame_height}")]
    CropOutOfBounds { width: u32, height: u32, x: u32, y: u32, frame_width: u32, frame_height: u32 },
    #[error("crop dimensions must be non-zero")]
    CropZeroSize,
    #[error("scale result must be non-zero (got {width}x{height})")]
    ScaleToZero { width: u32, height: u32 },
    #[error("scale factor must be finite and positive (got {factor})")]
    ScaleFactorInvalid { factor: f64 },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotation_defaults_to_r0() {
        assert_eq!(Rotation::default(), Rotation::R0);
    }

    #[test]
    fn flip_defaults_to_keep() {
        assert_eq!(Flip::default(), Flip::Keep);
    }

    #[test]
    fn transform_config_defaults_to_no_op() {
        let config = TransformConfig::default();
        assert_eq!(config.crop, None);
        assert_eq!(config.rotation, Rotation::R0);
        assert_eq!(config.flip, Flip::Keep);
        assert_eq!(config.scale, None);
    }
}
