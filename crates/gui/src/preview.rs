//! BGRA8 -> RGBA8 conversion, `egui::ColorImage` construction, and
//! letterbox layout for the live preview panel.

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
}
