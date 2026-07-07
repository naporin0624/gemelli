use crate::frame::Frame;
use crate::transform::config::{CropRect, TransformError};

pub fn crop(frame: &Frame, rect: CropRect) -> Result<Frame, TransformError> {
    if rect.width == 0 || rect.height == 0 {
        return Err(TransformError::CropZeroSize);
    }
    let right = rect.x.checked_add(rect.width);
    let bottom = rect.y.checked_add(rect.height);
    let fits = matches!(
        (right, bottom),
        (Some(r), Some(b)) if r <= frame.width() && b <= frame.height()
    );
    if !fits {
        return Err(TransformError::CropOutOfBounds {
            width: rect.width,
            height: rect.height,
            x: rect.x,
            y: rect.y,
            frame_width: frame.width(),
            frame_height: frame.height(),
        });
    }
    let mut data = Vec::new();
    for y in 0..rect.height {
        for x in 0..rect.width {
            let pixel = frame.pixel(rect.x + x, rect.y + y).unwrap_or([0, 0, 0, 0]);
            data.extend_from_slice(&pixel);
        }
    }
    Ok(Frame::from_validated(rect.width, rect.height, data))
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
    fn crops_the_requested_rect() {
        let frame = sample_frame();
        let rect = CropRect { width: 1, height: 2, x: 1, y: 1 };
        let result = crop(&frame, rect).unwrap();
        let expected = Frame::new(
            1,
            2,
            vec![
                13, 23, 33, 255, // (0,0) <- input (1,1)
                15, 25, 35, 255, // (0,1) <- input (1,2)
            ],
        )
        .unwrap();
        assert_eq!(result, expected);
    }

    #[test]
    fn rejects_zero_size() {
        let frame = sample_frame();
        let rect = CropRect { width: 0, height: 1, x: 0, y: 0 };
        assert_eq!(crop(&frame, rect), Err(TransformError::CropZeroSize));
    }

    #[test]
    fn rejects_rect_exceeding_bounds() {
        let frame = sample_frame();
        let rect = CropRect { width: 2, height: 1, x: 1, y: 2 };
        assert_eq!(
            crop(&frame, rect),
            Err(TransformError::CropOutOfBounds {
                width: 2,
                height: 1,
                x: 1,
                y: 2,
                frame_width: 2,
                frame_height: 3,
            })
        );
    }
}
