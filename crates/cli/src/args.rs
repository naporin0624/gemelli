//! CLI argument definition and value parsing.
//!
//! Value parsers here reject anything the core `transform` module cannot represent
//! (e.g. non-90-degree rotations) before a `TransformConfig` is ever built, so `core`
//! never has to handle CLI-shaped invalid input.

use clap::Parser;
use gemelli_core::transform::{CropRect, Flip, Rotation, ScaleSpec, TransformConfig};

#[derive(Debug, Parser)]
#[command(name = "gemelli")]
pub struct Args {
    /// Camera device index; omit to select interactively (TTY only)
    pub device: Option<u32>,

    #[arg(long)]
    pub list_devices: bool,

    #[arg(long, default_value = "0", value_parser = parse_rotation)]
    pub rotate: Rotation,

    #[arg(long, value_parser = parse_flip)]
    pub flip: Option<Flip>,

    #[arg(long, value_parser = parse_crop)]
    pub crop: Option<CropRect>,

    #[arg(long, value_parser = parse_scale)]
    pub scale: Option<ScaleSpec>,

    #[arg(long, default_value = "gemelli")]
    pub server_name: String,

    #[arg(long, value_parser = parse_fps)]
    pub fps: Option<u32>,
}

impl Args {
    pub fn transform_config(&self) -> TransformConfig {
        TransformConfig {
            crop: self.crop,
            rotation: self.rotate,
            flip: self.flip.unwrap_or_default(),
            scale: self.scale,
        }
    }
}

fn parse_rotation(input: &str) -> Result<Rotation, String> {
    match input {
        "0" => Ok(Rotation::R0),
        "90" => Ok(Rotation::R90),
        "180" => Ok(Rotation::R180),
        "270" => Ok(Rotation::R270),
        other => Err(format!("invalid rotation \"{other}\" (expected one of: 0, 90, 180, 270)")),
    }
}

fn parse_flip(input: &str) -> Result<Flip, String> {
    match input {
        "h" => Ok(Flip::Horizontal),
        "v" => Ok(Flip::Vertical),
        "hv" => Ok(Flip::Both),
        other => Err(format!("invalid flip \"{other}\" (expected one of: h, v, hv)")),
    }
}

fn parse_fps(input: &str) -> Result<u32, String> {
    let Ok(fps) = input.parse::<u32>() else {
        return Err(format!("invalid fps \"{input}\" (expected a positive integer)"));
    };

    if fps == 0 {
        return Err("invalid fps \"0\" (must be greater than 0)".to_string());
    }

    Ok(fps)
}

fn crop_format_error(input: &str) -> String {
    format!("invalid crop \"{input}\" (expected format WxH+X+Y, e.g. 1280x720+320+180)")
}

fn parse_crop(input: &str) -> Result<CropRect, String> {
    let Some((size, coords)) = input.split_once('+') else {
        return Err(crop_format_error(input));
    };

    let Some((width, height)) = size.split_once('x') else {
        return Err(crop_format_error(input));
    };

    let Ok(width) = width.parse::<u32>() else {
        return Err(crop_format_error(input));
    };
    let Ok(height) = height.parse::<u32>() else {
        return Err(crop_format_error(input));
    };

    let Some((x, y)) = coords.split_once('+') else {
        return Err(crop_format_error(input));
    };

    let Ok(x) = x.parse::<u32>() else {
        return Err(crop_format_error(input));
    };
    let Ok(y) = y.parse::<u32>() else {
        return Err(crop_format_error(input));
    };

    if width == 0 || height == 0 {
        return Err(format!("crop dimensions must be non-zero (got {width}x{height})"));
    }

    Ok(CropRect { width, height, x, y })
}

fn scale_format_error(input: &str) -> String {
    format!("invalid scale \"{input}\" (expected WxH or a positive factor, e.g. 960x540 or 0.5)")
}

fn parse_scale(input: &str) -> Result<ScaleSpec, String> {
    if let Some((width, height)) = input.split_once('x') {
        let Ok(width) = width.parse::<u32>() else {
            return Err(scale_format_error(input));
        };
        let Ok(height) = height.parse::<u32>() else {
            return Err(scale_format_error(input));
        };
        if width == 0 || height == 0 {
            return Err(format!("scale dimensions must be non-zero (got {width}x{height})"));
        }
        return Ok(ScaleSpec::Exact { width, height });
    }

    let Ok(factor) = input.parse::<f64>() else {
        return Err(scale_format_error(input));
    };
    if !factor.is_finite() || factor <= 0.0 {
        return Err(format!("scale factor must be finite and positive (got {factor})"));
    }

    Ok(ScaleSpec::Factor(factor))
}

#[cfg(test)]
mod parse_fps_tests {
    use super::*;

    #[test]
    fn accepts_positive_values() {
        assert_eq!(parse_fps("1"), Ok(1));
        assert_eq!(parse_fps("30"), Ok(30));
    }

    #[test]
    fn rejects_zero_with_helpful_message() {
        let Err(message) = parse_fps("0") else {
            panic!("expected \"0\" to be rejected");
        };
        assert!(message.contains("0"), "message: {message}");
    }

    #[test]
    fn rejects_garbage_input() {
        let Err(message) = parse_fps("abc") else {
            panic!("expected \"abc\" to be rejected");
        };
        assert!(message.contains("abc"), "message: {message}");
    }
}

#[cfg(test)]
mod parse_rotation_tests {
    use super::*;

    #[test]
    fn accepts_valid_values() {
        let cases = [
            ("0", Rotation::R0),
            ("90", Rotation::R90),
            ("180", Rotation::R180),
            ("270", Rotation::R270),
        ];

        for (input, expected) in cases {
            assert_eq!(parse_rotation(input), Ok(expected), "input: {input}");
        }
    }

    #[test]
    fn rejects_invalid_value_with_helpful_message() {
        let Err(message) = parse_rotation("45") else {
            panic!("expected \"45\" to be rejected");
        };
        assert!(message.contains("0"), "message: {message}");
        assert!(message.contains("90"), "message: {message}");
        assert!(message.contains("180"), "message: {message}");
        assert!(message.contains("270"), "message: {message}");
    }
}

#[cfg(test)]
mod parse_flip_tests {
    use super::*;

    #[test]
    fn accepts_valid_values() {
        let cases = [("h", Flip::Horizontal), ("v", Flip::Vertical), ("hv", Flip::Both)];

        for (input, expected) in cases {
            assert_eq!(parse_flip(input), Ok(expected), "input: {input}");
        }
    }

    #[test]
    fn rejects_invalid_value_with_helpful_message() {
        let Err(message) = parse_flip("x") else {
            panic!("expected \"x\" to be rejected");
        };
        assert!(message.contains("h"), "message: {message}");
        assert!(message.contains("v"), "message: {message}");
        assert!(message.contains("hv"), "message: {message}");
    }
}

#[cfg(test)]
mod parse_crop_tests {
    use super::*;

    #[test]
    fn accepts_valid_crop_specs() {
        let cases = [
            ("1280x720+320+180", CropRect { width: 1280, height: 720, x: 320, y: 180 }),
            ("100x200+0+0", CropRect { width: 100, height: 200, x: 0, y: 0 }),
        ];

        for (input, expected) in cases {
            assert_eq!(parse_crop(input), Ok(expected), "input: {input}");
        }
    }

    #[test]
    fn rejects_zero_width_or_height() {
        let Err(message) = parse_crop("0x10+0+0") else {
            panic!("expected \"0x10+0+0\" to be rejected");
        };
        assert!(message.contains("non-zero"), "message: {message}");
    }

    #[test]
    fn rejects_malformed_spec() {
        for input in ["1280x720", "1280+320+180", "abcx720+0+0", ""] {
            assert!(parse_crop(input).is_err(), "expected \"{input}\" to be rejected");
        }
    }
}

#[cfg(test)]
mod parse_scale_tests {
    use super::*;

    #[test]
    fn accepts_valid_scale_specs() {
        assert_eq!(parse_scale("960x540"), Ok(ScaleSpec::Exact { width: 960, height: 540 }));
        assert_eq!(parse_scale("0.5"), Ok(ScaleSpec::Factor(0.5)));
        assert_eq!(parse_scale("2"), Ok(ScaleSpec::Factor(2.0)));
    }

    #[test]
    fn rejects_non_positive_factor() {
        let Err(message) = parse_scale("-0.5") else {
            panic!("expected \"-0.5\" to be rejected");
        };
        assert!(message.contains("positive"), "message: {message}");
    }

    #[test]
    fn rejects_garbage_input() {
        let Err(message) = parse_scale("abc") else {
            panic!("expected \"abc\" to be rejected");
        };
        assert!(message.contains("abc"), "message: {message}");
    }
}

#[cfg(test)]
mod args_parse_tests {
    use super::*;

    #[test]
    fn parses_device_and_options() {
        let Ok(args) = Args::try_parse_from(["prog", "0", "--rotate", "90", "--flip", "h"]) else {
            panic!("expected successful parse");
        };
        assert_eq!(args.device, Some(0));
        assert_eq!(args.rotate, Rotation::R90);
        assert_eq!(args.flip, Some(Flip::Horizontal));
    }

    #[test]
    fn rejects_invalid_rotate_with_usage_error() {
        let Err(error) = Args::try_parse_from(["prog", "--rotate", "45"]) else {
            panic!("expected parse failure for --rotate 45");
        };
        assert_eq!(error.exit_code(), 2);
    }

    #[test]
    fn rejects_fps_zero_with_usage_error() {
        let Err(error) = Args::try_parse_from(["prog", "0", "--fps", "0"]) else {
            panic!("expected parse failure for --fps 0");
        };
        assert_eq!(error.exit_code(), 2);
    }

    #[test]
    fn accepts_fps_one() {
        let Ok(args) = Args::try_parse_from(["prog", "0", "--fps", "1"]) else {
            panic!("expected successful parse for --fps 1");
        };
        assert_eq!(args.fps, Some(1));
    }
}

#[cfg(test)]
mod transform_config_tests {
    use super::*;

    fn base_args() -> Args {
        Args {
            device: None,
            list_devices: false,
            rotate: Rotation::R0,
            flip: None,
            crop: None,
            scale: None,
            server_name: "gemelli".to_string(),
            fps: None,
        }
    }

    #[test]
    fn defaults_flip_to_keep_when_absent() {
        let config = base_args().transform_config();
        assert_eq!(config.flip, Flip::Keep);
    }

    #[test]
    fn carries_rotation_crop_and_scale_through() {
        let mut args = base_args();
        args.rotate = Rotation::R90;
        args.crop = Some(CropRect { width: 100, height: 100, x: 0, y: 0 });
        args.scale = Some(ScaleSpec::Factor(0.5));

        let config = args.transform_config();

        assert_eq!(config.rotation, Rotation::R90);
        assert_eq!(config.crop, Some(CropRect { width: 100, height: 100, x: 0, y: 0 }));
        assert_eq!(config.scale, Some(ScaleSpec::Factor(0.5)));
    }
}
