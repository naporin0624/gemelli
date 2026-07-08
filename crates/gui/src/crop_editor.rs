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

    #[cfg_attr(not(test), allow(dead_code))]
    pub fn to_frame(&self, rect: egui::Rect) -> CropRect {
        let (scale_x, scale_y) = self.scale_factors();
        let x = to_frame_coord((rect.min.x - self.draw.min.x) / scale_x);
        let y = to_frame_coord((rect.min.y - self.draw.min.y) / scale_y);
        let width = to_frame_coord(rect.width() / scale_x);
        let height = to_frame_coord(rect.height() / scale_y);
        clamp_rect(CropRect { width, height, x, y }, self.frame_width, self.frame_height)
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

/// Drag interaction state. Exhaustive-matched everywhere it's consumed —
/// no `_` arm — so adding a sixth handle is a compile error at every call
/// site until it's handled, not a silent no-op.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
pub enum DragMode {
    Move,
    ResizeNw,
    ResizeNe,
    ResizeSw,
    ResizeSe,
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(not(test), allow(dead_code))]
pub struct DragState {
    pub mode: DragMode,
    pub start_rect: CropRect,
    pub start_pointer: egui::Pos2,
}

/// Given a drag delta in screen coords, produces the new (clamped)
/// `CropRect`. Each resize arm moves only its own corner, leaving the
/// opposite corner fixed; `Move` translates both corners equally.
#[cfg_attr(not(test), allow(dead_code))]
pub fn apply_drag(state: &DragState, mapping: &CropMapping, pointer: egui::Pos2) -> CropRect {
    let delta = pointer - state.start_pointer;
    let start_screen = mapping.to_screen(state.start_rect);
    let new_screen = match state.mode {
        DragMode::Move => start_screen.translate(delta),
        DragMode::ResizeNw => egui::Rect::from_min_max(start_screen.min + delta, start_screen.max),
        DragMode::ResizeNe => egui::Rect::from_min_max(
            egui::pos2(start_screen.min.x, start_screen.min.y + delta.y),
            egui::pos2(start_screen.max.x + delta.x, start_screen.max.y),
        ),
        DragMode::ResizeSw => egui::Rect::from_min_max(
            egui::pos2(start_screen.min.x + delta.x, start_screen.min.y),
            egui::pos2(start_screen.max.x, start_screen.max.y + delta.y),
        ),
        DragMode::ResizeSe => egui::Rect::from_min_max(start_screen.min, start_screen.max + delta),
    };
    mapping.to_frame(new_screen)
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

    #[test]
    fn to_frame_is_the_inverse_of_to_screen() {
        let mapping = fixture_mapping();
        let original = CropRect { width: 960, height: 540, x: 480, y: 270 };

        let round_tripped = mapping.to_frame(mapping.to_screen(original));

        // The scale factor (640/1920) has no exact f32 representation, but
        // to_frame_coord's round() absorbs that error at these magnitudes
        // — verified by running the real arithmetic (see the fixture note
        // above), not assumed. Assert within 1px per the contract, even
        // though this fixture happens to land exactly on `original`.
        assert!(round_tripped.x.abs_diff(original.x) <= 1);
        assert!(round_tripped.y.abs_diff(original.y) <= 1);
        assert!(round_tripped.width.abs_diff(original.width) <= 1);
        assert!(round_tripped.height.abs_diff(original.height) <= 1);
    }

    #[test]
    fn to_frame_clamps_a_rect_that_overhangs_the_draw_area() {
        let mapping = fixture_mapping();
        // Screen rect starting right at the draw origin but 3x too wide/
        // tall for the frame at this scale (960 screen px / (1/3 scale)
        // = 2880 frame px > 1920 frame_width) — must clamp into bounds.
        let overhanging = egui::Rect::from_min_size(mapping.draw.min, egui::vec2(960.0, 540.0));

        let frame_rect = mapping.to_frame(overhanging);

        assert_eq!(frame_rect, CropRect { width: 1920, height: 1080, x: 0, y: 0 });
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

#[cfg(test)]
mod apply_drag_tests {
    use gemelli_core::transform::CropRect;

    use super::{CropMapping, DragMode, DragState, apply_drag};

    fn fixture_mapping() -> CropMapping {
        CropMapping {
            frame_width: 1920,
            frame_height: 1080,
            draw: egui::Rect::from_min_size(egui::pos2(100.0, 50.0), egui::vec2(640.0, 360.0)),
        }
    }

    // start_rect's screen projection is (260,140)-(580,320) under
    // fixture_mapping — see Cycle 1's to_screen test.
    fn start_rect() -> CropRect {
        CropRect { width: 960, height: 540, x: 480, y: 270 }
    }

    #[test]
    fn move_translates_without_changing_size() {
        let mapping = fixture_mapping();
        let start_pointer = egui::pos2(420.0, 230.0); // center of the screen rect
        let state = DragState { mode: DragMode::Move, start_rect: start_rect(), start_pointer };

        let result = apply_drag(&state, &mapping, egui::pos2(435.0, 239.0)); // +15,+9 screen px

        assert_eq!(result, CropRect { width: 960, height: 540, x: 525, y: 297 });
    }

    #[test]
    fn resize_se_grows_from_the_fixed_top_left_corner() {
        let mapping = fixture_mapping();
        let start_pointer = egui::pos2(580.0, 320.0); // screen rect's max corner
        let state = DragState { mode: DragMode::ResizeSe, start_rect: start_rect(), start_pointer };

        let result = apply_drag(&state, &mapping, egui::pos2(610.0, 350.0)); // +30,+30 screen px

        assert_eq!(result, CropRect { width: 1050, height: 630, x: 480, y: 270 });
    }

    #[test]
    fn resize_nw_moves_the_origin_and_shrinks_from_the_fixed_bottom_right_corner() {
        let mapping = fixture_mapping();
        let start_pointer = egui::pos2(260.0, 140.0); // screen rect's min corner
        let state = DragState { mode: DragMode::ResizeNw, start_rect: start_rect(), start_pointer };

        let result = apply_drag(&state, &mapping, egui::pos2(290.0, 170.0)); // +30,+30 screen px

        assert_eq!(result, CropRect { width: 870, height: 450, x: 570, y: 360 });
    }

    #[test]
    fn resize_ne_moves_the_top_edge_and_grows_the_right_edge() {
        let mapping = fixture_mapping();
        let start_pointer = egui::pos2(580.0, 140.0); // top-right corner
        let state = DragState { mode: DragMode::ResizeNe, start_rect: start_rect(), start_pointer };

        let result = apply_drag(&state, &mapping, egui::pos2(610.0, 110.0)); // +30 right, -30 up screen px

        assert_eq!(result, CropRect { width: 1050, height: 630, x: 480, y: 180 });
    }

    #[test]
    fn resize_sw_moves_the_left_edge_and_grows_the_bottom_edge() {
        let mapping = fixture_mapping();
        let start_pointer = egui::pos2(260.0, 320.0); // bottom-left corner
        let state = DragState { mode: DragMode::ResizeSw, start_rect: start_rect(), start_pointer };

        let result = apply_drag(&state, &mapping, egui::pos2(230.0, 350.0)); // -30 left, +30 down screen px

        assert_eq!(result, CropRect { width: 1050, height: 630, x: 390, y: 270 });
    }
}
