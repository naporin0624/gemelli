//! BGRA8 -> RGBA8 conversion, `egui::ColorImage` construction, and
//! letterbox layout for the live preview panel.

use gemelli_core::frame::Frame;

/// BGRA8 -> RGBA8 byte swizzle (the channel order `egui::ColorImage`
/// expects). Pure; any trailing bytes that don't form a full BGRA pixel
/// are dropped, matching `Frame`'s own tightly-packed invariant.
///
/// Not yet called from production code (wired up when the preview panel
/// consumes it in a later task), hence `allow(dead_code)` outside `cfg(test)`
/// — same pattern as `theme::contrast_ratio`.
#[cfg_attr(not(test), allow(dead_code))]
pub fn bgra_to_rgba(bgra: &[u8]) -> Vec<u8> {
    bgra.chunks_exact(4).flat_map(|pixel| [pixel[2], pixel[1], pixel[0], pixel[3]]).collect()
}

/// `Frame` -> `egui::ColorImage` (thin wrapper over `bgra_to_rgba`).
///
/// Not yet called from production code (wired up when the preview panel
/// consumes it in a later task), hence `allow(dead_code)` outside `cfg(test)`
/// — same pattern as `theme::contrast_ratio`.
#[cfg_attr(not(test), allow(dead_code))]
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
}
