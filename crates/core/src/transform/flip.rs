//! Mirror flips. Dimensions never change.
//!
//! - Horizontal: output(x, y) = input(width - 1 - x, y)
//! - Vertical:   output(x, y) = input(x, height - 1 - y)
//! - Both:       output(x, y) = input(width - 1 - x, height - 1 - y)

use crate::frame::Frame;
use crate::transform::config::Flip;

pub fn flip(frame: &Frame, direction: Flip) -> Frame {
    match direction {
        Flip::Keep => frame.clone(),
        Flip::Horizontal => flip_horizontal(frame),
        Flip::Vertical => flip_vertical(frame),
        Flip::Both => flip_both(frame),
    }
}

fn flip_horizontal(frame: &Frame) -> Frame {
    let width = frame.width();
    let height = frame.height();
    let mut data = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let pixel = frame.pixel(width - 1 - x, y).unwrap_or([0, 0, 0, 0]);
            data.extend_from_slice(&pixel);
        }
    }
    Frame::from_validated(width, height, data)
}

fn flip_vertical(frame: &Frame) -> Frame {
    let width = frame.width();
    let height = frame.height();
    let mut data = Vec::new();
    for y in 0..height {
        for x in 0..width {
            let pixel = frame.pixel(x, height - 1 - y).unwrap_or([0, 0, 0, 0]);
            data.extend_from_slice(&pixel);
        }
    }
    Frame::from_validated(width, height, data)
}

fn flip_both(frame: &Frame) -> Frame {
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
    fn keep_is_identity() {
        let frame = sample_frame();
        assert_eq!(flip(&frame, Flip::Keep), frame);
    }

    #[test]
    fn horizontal_mirrors_columns() {
        let frame = sample_frame();
        let flipped = flip(&frame, Flip::Horizontal);
        let expected = Frame::new(
            2,
            3,
            vec![
                11, 21, 31, 255, // (0,0) <- input (1,0)
                10, 20, 30, 255, // (1,0) <- input (0,0)
                13, 23, 33, 255, // (0,1) <- input (1,1)
                12, 22, 32, 255, // (1,1) <- input (0,1)
                15, 25, 35, 255, // (0,2) <- input (1,2)
                14, 24, 34, 255, // (1,2) <- input (0,2)
            ],
        )
        .unwrap();
        assert_eq!(flipped, expected);
    }

    #[test]
    fn vertical_mirrors_rows() {
        let frame = sample_frame();
        let flipped = flip(&frame, Flip::Vertical);
        let expected = Frame::new(
            2,
            3,
            vec![
                14, 24, 34, 255, // (0,0) <- input (0,2)
                15, 25, 35, 255, // (1,0) <- input (1,2)
                12, 22, 32, 255, // (0,1) <- input (0,1)
                13, 23, 33, 255, // (1,1) <- input (1,1)
                10, 20, 30, 255, // (0,2) <- input (0,0)
                11, 21, 31, 255, // (1,2) <- input (1,0)
            ],
        )
        .unwrap();
        assert_eq!(flipped, expected);
    }

    #[test]
    fn both_mirrors_rows_and_columns() {
        let frame = sample_frame();
        let flipped = flip(&frame, Flip::Both);
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
        assert_eq!(flipped, expected);
    }
}
