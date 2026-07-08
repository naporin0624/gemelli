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
