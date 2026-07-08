//! Pure screen<->frame geometry for the crop-drag UI: coordinate
//! conversion, clamping, and the corner/move drag state machine. No egui
//! widget code lives here — `sidebar.rs`/`app.rs` call these functions
//! and draw the result.

use gemelli_core::transform::CropRect;

/// The only `as` casts in this module: f32 (egui screen space) <-> u32
/// (frame pixel space). `f32 -> u32` has no infallible `TryFrom` in std
/// (same gap core's `transform::scale::scale_dimension` hits for
/// `f64 -> u32`), so `round()` + `clamp()` bound the value into u32's
/// range before the cast. `u32 -> f32` has no infallible std conversion
/// either (`From<u32> for f32` doesn't exist — only `u8`/`u16` do); frame
/// dimensions never approach 2^24 px, so the precision loss the cast can
/// introduce above that threshold is immaterial here.
#[cfg_attr(not(test), allow(dead_code))]
#[allow(clippy::as_conversions)]
fn to_frame_coord(v: f32) -> u32 {
    v.round().clamp(0.0, u32::MAX as f32) as u32
}

#[cfg_attr(not(test), allow(dead_code))]
#[allow(clippy::as_conversions)]
fn to_screen_coord(v: u32) -> f32 {
    v as f32
}

/// Maps a `CropRect` (frame pixel coords) into the preview draw rect
/// (screen coords) and back.
#[cfg_attr(not(test), allow(dead_code))]
pub struct CropMapping {
    pub frame_width: u32,
    pub frame_height: u32,
    pub draw: egui::Rect,
}

impl CropMapping {
    fn scale_factors(&self) -> (f32, f32) {
        (
            self.draw.width() / to_screen_coord(self.frame_width),
            self.draw.height() / to_screen_coord(self.frame_height),
        )
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn to_screen(&self, rect: CropRect) -> egui::Rect {
        let (scale_x, scale_y) = self.scale_factors();
        let min = egui::pos2(
            self.draw.min.x + to_screen_coord(rect.x) * scale_x,
            self.draw.min.y + to_screen_coord(rect.y) * scale_y,
        );
        let size = egui::vec2(
            to_screen_coord(rect.width) * scale_x,
            to_screen_coord(rect.height) * scale_y,
        );
        egui::Rect::from_min_size(min, size)
    }
}

const MIN_CROP_SIDE: u32 = 16;

/// Normalizes a (possibly drag-produced) rect: at least `16x16` frame
/// px, fully inside `[0, frame_width) x [0, frame_height)`. Width/height
/// are bounded first so the position clamp's `frame_width - width`
/// subtraction can never underflow.
#[cfg_attr(not(test), allow(dead_code))]
pub fn clamp_rect(rect: CropRect, frame_width: u32, frame_height: u32) -> CropRect {
    let width = rect.width.clamp(MIN_CROP_SIDE.min(frame_width), frame_width);
    let height = rect.height.clamp(MIN_CROP_SIDE.min(frame_height), frame_height);
    let x = rect.x.min(frame_width - width);
    let y = rect.y.min(frame_height - height);
    CropRect { width, height, x, y }
}

#[cfg(test)]
mod coord_tests {
    use super::{to_frame_coord, to_screen_coord};

    #[test]
    fn to_frame_coord_rounds_to_nearest() {
        assert_eq!(to_frame_coord(479.6), 480);
        assert_eq!(to_frame_coord(479.4), 479);
    }

    #[test]
    fn to_frame_coord_clamps_negative_to_zero() {
        assert_eq!(to_frame_coord(-3.0), 0);
    }

    #[test]
    fn to_screen_coord_is_a_lossless_widening() {
        assert_eq!(to_screen_coord(480), 480.0_f32);
        assert_eq!(to_screen_coord(0), 0.0_f32);
    }
}

#[cfg(test)]
mod crop_mapping_tests {
    use gemelli_core::transform::CropRect;

    use super::CropMapping;

    fn fixture_mapping() -> CropMapping {
        CropMapping {
            frame_width: 1920,
            frame_height: 1080,
            draw: egui::Rect::from_min_size(egui::pos2(100.0, 50.0), egui::vec2(640.0, 360.0)),
        }
    }

    #[test]
    fn to_screen_scales_and_offsets_by_the_draw_rect() {
        let mapping = fixture_mapping();
        let rect = CropRect { width: 960, height: 540, x: 480, y: 270 };

        let screen = mapping.to_screen(rect);

        assert_eq!(screen.min, egui::pos2(260.0, 140.0));
        assert_eq!(screen.max, egui::pos2(580.0, 320.0));
    }
}

#[cfg(test)]
mod clamp_rect_tests {
    use gemelli_core::transform::CropRect;

    use super::clamp_rect;

    const FRAME_W: u32 = 1920;
    const FRAME_H: u32 = 1080;

    #[test]
    fn below_minimum_size_grows_to_16x16() {
        let rect = CropRect { width: 10, height: 10, x: 0, y: 0 };
        assert_eq!(
            clamp_rect(rect, FRAME_W, FRAME_H),
            CropRect { width: 16, height: 16, x: 0, y: 0 }
        );
    }

    #[test]
    fn overflow_right_edge_slides_x_left() {
        let rect = CropRect { width: 100, height: 100, x: 1900, y: 0 };
        assert_eq!(
            clamp_rect(rect, FRAME_W, FRAME_H),
            CropRect { width: 100, height: 100, x: 1820, y: 0 }
        );
    }

    #[test]
    fn overflow_bottom_edge_slides_y_up() {
        let rect = CropRect { width: 100, height: 100, x: 0, y: 1060 };
        assert_eq!(
            clamp_rect(rect, FRAME_W, FRAME_H),
            CropRect { width: 100, height: 100, x: 0, y: 980 }
        );
    }

    #[test]
    fn oversize_both_dimensions_shrinks_to_full_frame() {
        let rect = CropRect { width: 3000, height: 3000, x: 0, y: 0 };
        assert_eq!(
            clamp_rect(rect, FRAME_W, FRAME_H),
            CropRect { width: FRAME_W, height: FRAME_H, x: 0, y: 0 }
        );
    }

    #[test]
    fn already_valid_rect_is_unchanged() {
        let rect = CropRect { width: 960, height: 540, x: 480, y: 270 };
        assert_eq!(clamp_rect(rect, FRAME_W, FRAME_H), rect);
    }
}
