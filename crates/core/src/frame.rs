//! Frame type: BGRA8 tightly-packed pixel buffer.
//!
//! Layout: `data` is `width * height * 4` bytes, row-major, BGRA8 per pixel
//! (no padding — stride is always `width * 4`). Pixel (x, y) starts at byte
//! offset `(y * width + x) * 4`.

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    width: u32,
    height: u32,
    data: Vec<u8>,
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum FrameError {
    #[error("frame data length {actual} does not match {width}x{height}x4 = {expected}")]
    DataLengthMismatch { width: u32, height: u32, expected: usize, actual: usize },
    #[error("frame dimensions must be non-zero (got {width}x{height})")]
    ZeroDimension { width: u32, height: u32 },
}

impl Frame {
    pub fn new(width: u32, height: u32, data: Vec<u8>) -> Result<Self, FrameError> {
        if width == 0 || height == 0 {
            return Err(FrameError::ZeroDimension { width, height });
        }
        let width_len = usize::try_from(width).unwrap_or(usize::MAX);
        let height_len = usize::try_from(height).unwrap_or(usize::MAX);
        let expected = width_len
            .checked_mul(height_len)
            .and_then(|pixels| pixels.checked_mul(4))
            .unwrap_or(usize::MAX);
        if data.len() != expected {
            return Err(FrameError::DataLengthMismatch {
                width,
                height,
                expected,
                actual: data.len(),
            });
        }
        Ok(Self { width, height, data })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let width_len = usize::try_from(self.width).unwrap_or(usize::MAX);
        let x_len = usize::try_from(x).unwrap_or(usize::MAX);
        let y_len = usize::try_from(y).unwrap_or(usize::MAX);
        let idx = y_len.checked_mul(width_len)?.checked_add(x_len)?.checked_mul(4)?;
        let end = idx.checked_add(4)?;
        let bytes = self.data.get(idx..end)?;
        Some([bytes[0], bytes[1], bytes[2], bytes[3]])
    }

    /// Builds a `Frame` without re-validating length/non-zero invariants.
    /// Callers (transform functions) already derive `width`/`height` from a
    /// source `Frame` and push exactly `width * height * 4` bytes, so the
    /// `new` round-trip would only re-check what the caller already proved.
    pub(crate) fn from_validated(width: u32, height: u32, data: Vec<u8>) -> Self {
        Self { width, height, data }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn new_accepts_matching_length() {
        let frame = sample_frame();
        assert_eq!(frame.width(), 2);
        assert_eq!(frame.height(), 3);
        assert_eq!(frame.data().len(), 24);
    }

    #[test]
    fn new_rejects_length_mismatch() {
        let result = Frame::new(2, 3, vec![0; 10]);
        assert_eq!(
            result,
            Err(FrameError::DataLengthMismatch { width: 2, height: 3, expected: 24, actual: 10 })
        );
    }

    #[test]
    fn new_rejects_zero_dimension() {
        let result = Frame::new(0, 3, vec![]);
        assert_eq!(result, Err(FrameError::ZeroDimension { width: 0, height: 3 }));
    }

    #[test]
    fn pixel_returns_bgra_bytes_at_position() {
        let frame = sample_frame();
        assert_eq!(frame.pixel(0, 0), Some([10, 20, 30, 255]));
        assert_eq!(frame.pixel(1, 0), Some([11, 21, 31, 255]));
        assert_eq!(frame.pixel(0, 1), Some([12, 22, 32, 255]));
        assert_eq!(frame.pixel(1, 1), Some([13, 23, 33, 255]));
        assert_eq!(frame.pixel(0, 2), Some([14, 24, 34, 255]));
        assert_eq!(frame.pixel(1, 2), Some([15, 25, 35, 255]));
    }

    #[test]
    fn pixel_returns_none_out_of_bounds() {
        let frame = sample_frame();
        assert_eq!(frame.pixel(2, 0), None);
        assert_eq!(frame.pixel(0, 3), None);
    }

    #[test]
    fn from_validated_skips_length_check() {
        let frame = Frame::from_validated(2, 1, vec![1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(frame.width(), 2);
        assert_eq!(frame.height(), 1);
        assert_eq!(frame.pixel(1, 0), Some([5, 6, 7, 8]));
    }
}
