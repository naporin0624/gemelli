//! BGRA8 -> RGBA8 conversion, `egui::ColorImage` construction, and
//! letterbox layout for the live preview panel.

use gemelli_core::frame::Frame;

/// BGRA8 -> RGBA8 byte swizzle (the channel order `egui::ColorImage`
/// expects). Pure; any trailing bytes that don't form a full BGRA pixel
/// are dropped, matching `Frame`'s own tightly-packed invariant.
pub fn bgra_to_rgba(bgra: &[u8]) -> Vec<u8> {
    bgra.chunks_exact(4).flat_map(|pixel| [pixel[2], pixel[1], pixel[0], pixel[3]]).collect()
}

/// `Frame` -> `egui::ColorImage` (thin wrapper over `bgra_to_rgba`).
pub fn color_image(frame: &Frame) -> egui::ColorImage {
    // u32 -> usize: no `From<u32> for usize` exists in std (checked with
    // rustc directly — only `From<u8>`/`From<u16>` do), so this reuses the
    // same fallible-but-practically-infallible `try_from` + `unwrap_or`
    // idiom `Frame::new` itself already uses in crates/core/src/frame.rs;
    // usize is >= 32 bits on every platform this workspace targets.
    let width = usize::try_from(frame.width()).unwrap_or(usize::MAX);
    let height = usize::try_from(frame.height()).unwrap_or(usize::MAX);
    let rgba = bgra_to_rgba(frame.data());
    egui::ColorImage::from_rgba_unmultiplied([width, height], &rgba)
}

/// gui's isolated `as`-cast, mirroring gemelli-core's `scale_dimension`
/// precedent (crates/core/src/transform/scale.rs): u32 -> f32 has no
/// lossless std conversion (`f32::from(u32)` does not exist — checked with
/// rustc directly — because f32's 24-bit mantissa can't represent every
/// u32 value), and camera/window dimensions never remotely approach 2^24,
/// so one documented cast site is preferable to threading a fallible
/// conversion through the render path.
#[allow(clippy::as_conversions)]
fn dim_to_f32(v: u32) -> f32 {
    v as f32
}

/// Letterbox-fits a `frame_width x frame_height` frame into `avail`,
/// preserving aspect ratio and centering the result.
pub fn fit_rect(frame_width: u32, frame_height: u32, avail: egui::Rect) -> egui::Rect {
    let frame_w = dim_to_f32(frame_width);
    let frame_h = dim_to_f32(frame_height);
    let avail_w = avail.width();
    let avail_h = avail.height();

    // Compare frame_w/frame_h against avail_w/avail_h via
    // cross-multiplication instead of dividing twice, so the constrained
    // dimension comes from a single multiply + divide (exact for
    // realistic frame/window sizes) rather than compounding two
    // divisions' rounding error.
    let (draw_w, draw_h) = if frame_w * avail_h > avail_w * frame_h {
        (avail_w, avail_w * frame_h / frame_w)
    } else {
        (avail_h * frame_w / frame_h, avail_h)
    };

    let offset_x = (avail_w - draw_w) / 2.0;
    let offset_y = (avail_h - draw_h) / 2.0;
    let min = avail.min + egui::vec2(offset_x, offset_y);

    egui::Rect::from_min_size(min, egui::vec2(draw_w, draw_h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bgra_to_rgba_swaps_red_and_blue_channels() {
        let bgra = [10u8, 20, 30, 40, 15, 25, 35, 45]; // two BGRA pixels
        assert_eq!(bgra_to_rgba(&bgra), vec![30, 20, 10, 40, 35, 25, 15, 45]);
    }

    #[test]
    fn bgra_to_rgba_empty_input_is_empty() {
        assert_eq!(bgra_to_rgba(&[]), Vec::<u8>::new());
    }

    #[test]
    fn color_image_reports_frame_size() {
        let data = vec![10, 20, 30, 255, 11, 21, 31, 255]; // 2x1 BGRA
        let frame = Frame::new(2, 1, data).unwrap();
        let image = color_image(&frame);
        assert_eq!(image.size, [2, 1]);
    }

    #[test]
    fn color_image_swizzles_pixel_bytes() {
        let data = vec![10, 20, 30, 255, 11, 21, 31, 255]; // 2x1 BGRA
        let frame = Frame::new(2, 1, data).unwrap();
        let image = color_image(&frame);
        assert_eq!(image.as_raw(), [30, 20, 10, 255, 31, 21, 11, 255].as_slice());
    }

    fn avail() -> egui::Rect {
        egui::Rect::from_min_size(egui::pos2(100.0, 50.0), egui::vec2(800.0, 600.0))
    }

    #[test]
    fn fit_rect_letterboxes_a_wide_frame_into_a_narrower_rect() {
        // 1920x1080 (16:9) is wider-than-avail (4:3) -> width-constrained,
        // centered with top/bottom bars.
        let rect = fit_rect(1920, 1080, avail());
        assert_eq!(
            rect,
            egui::Rect::from_min_size(egui::pos2(100.0, 125.0), egui::vec2(800.0, 450.0))
        );
    }

    #[test]
    fn fit_rect_pillarboxes_a_tall_frame_into_a_wider_rect() {
        // 1080x1920 (9:16) is taller-than-avail -> height-constrained,
        // centered with left/right bars.
        let rect = fit_rect(1080, 1920, avail());
        assert_eq!(
            rect,
            egui::Rect::from_min_size(egui::pos2(331.25, 50.0), egui::vec2(337.5, 600.0))
        );
    }

    #[test]
    fn fit_rect_matching_aspect_fills_avail_with_no_offset() {
        // Same 4:3 aspect as avail -> exact fill, zero letterbox offset.
        let rect = fit_rect(800, 600, avail());
        assert_eq!(rect, avail());
    }
}
