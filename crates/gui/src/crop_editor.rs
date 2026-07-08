//! Pure screen<->frame geometry for the crop-drag UI: coordinate
//! conversion, clamping, and the corner/move drag state machine. No egui
//! widget code lives here — `sidebar.rs`/`app.rs` call these functions
//! and draw the result.

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
