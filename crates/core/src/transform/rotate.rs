//! Clockwise rotation. R90/R270 swap width and height; R180 keeps them.
//!
//! For a source frame of size `width_in x height_in`:
//! - R90:  output(x, y) = input(y, height_in - 1 - x)
//! - R180: output(x, y) = input(width_in - 1 - x, height_in - 1 - y)
//! - R270: output(x, y) = input(width_in - 1 - y, x)

use crate::frame::Frame;
use crate::transform::config::Rotation;

pub fn rotate(frame: &Frame, rotation: Rotation) -> Frame {
    match rotation {
        Rotation::R0 => frame.clone(),
        Rotation::R90 => rotate_r90(frame),
        Rotation::R180 => rotate_r180(frame),
        Rotation::R270 => rotate_r270(frame),
    }
}

fn rotate_r90(frame: &Frame) -> Frame {
    let width_in = frame.width();
    let height_in = frame.height();
    let width_out = height_in;
    let height_out = width_in;
    let mut data = Vec::new();
    for y_out in 0..height_out {
        for x_out in 0..width_out {
            let pixel = frame.pixel(y_out, height_in - 1 - x_out).unwrap_or([0, 0, 0, 0]);
            data.extend_from_slice(&pixel);
        }
    }
    Frame::from_validated(width_out, height_out, data)
}

fn rotate_r180(frame: &Frame) -> Frame {
    let width = frame.width();
    let height = frame.height();
    let mut data = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let pixel = frame.pixel(width - 1 - x, height - 1 - y).unwrap_or([0, 0, 0, 0]);
            data.extend_from_slice(&pixel);
        }
    }
    Frame::from_validated(width, height, data)
}

fn rotate_r270(frame: &Frame) -> Frame {
    let width_in = frame.width();
    let height_in = frame.height();
    let width_out = height_in;
    let height_out = width_in;
    let mut data = Vec::new();
    for y_out in 0..height_out {
        for x_out in 0..width_out {
            let pixel = frame.pixel(width_in - 1 - y_out, x_out).unwrap_or([0, 0, 0, 0]);
            data.extend_from_slice(&pixel);
        }
    }
    Frame::from_validated(width_out, height_out, data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::Frame;

    fn sample_frame() -> Frame {
        let data = vec![
            10, 20, 30, 255, // (0,0)
            11, 21, 31, 255, // (1,0)
            12, 22, 32, 255, // (0,1)
            13, 23, 33, 255, // (1,1)
            14, 24, 34, 255, // (0,2)
            15, 25, 35, 255, // (1,2)
        ];
        Frame::new(2, 3, data).unwrap()
    }

    #[test]
    fn r0_is_identity() {
        let frame = sample_frame();
        assert_eq!(rotate(&frame, Rotation::R0), frame);
    }

    #[test]
    fn r90_rotates_clockwise_and_swaps_dimensions() {
        let frame = sample_frame();
        let rotated = rotate(&frame, Rotation::R90);
        let expected = Frame::new(
            3,
            2,
            vec![
                14, 24, 34, 255, // (0,0) <- input (0,2)
                12, 22, 32, 255, // (1,0) <- input (0,1)
                10, 20, 30, 255, // (2,0) <- input (0,0)
                15, 25, 35, 255, // (0,1) <- input (1,2)
                13, 23, 33, 255, // (1,1) <- input (1,1)
                11, 21, 31, 255, // (2,1) <- input (1,0)
            ],
        )
        .unwrap();
        assert_eq!(rotated, expected);
        assert_eq!(rotated.width(), frame.height());
        assert_eq!(rotated.height(), frame.width());
    }

    #[test]
    fn r180_reverses_both_axes() {
        let frame = sample_frame();
        let rotated = rotate(&frame, Rotation::R180);
        let expected = Frame::new(
            2,
            3,
            vec![
                15, 25, 35, 255, // (0,0) <- input (1,2)
                14, 24, 34, 255, // (1,0) <- input (0,2)
                13, 23, 33, 255, // (0,1) <- input (1,1)
                12, 22, 32, 255, // (1,1) <- input (0,1)
                11, 21, 31, 255, // (0,2) <- input (1,0)
                10, 20, 30, 255, // (1,2) <- input (0,0)
            ],
        )
        .unwrap();
        assert_eq!(rotated, expected);
    }

    #[test]
    fn r270_rotates_counterclockwise_and_swaps_dimensions() {
        let frame = sample_frame();
        let rotated = rotate(&frame, Rotation::R270);
        let expected = Frame::new(
            3,
            2,
            vec![
                11, 21, 31, 255, // (0,0) <- input (1,0)
                13, 23, 33, 255, // (1,0) <- input (1,1)
                15, 25, 35, 255, // (2,0) <- input (1,2)
                10, 20, 30, 255, // (0,1) <- input (0,0)
                12, 22, 32, 255, // (1,1) <- input (0,1)
                14, 24, 34, 255, // (2,1) <- input (0,2)
            ],
        )
        .unwrap();
        assert_eq!(rotated, expected);
        assert_eq!(rotated.width(), frame.height());
        assert_eq!(rotated.height(), frame.width());
    }
}
