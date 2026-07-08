//! WCAG 2.1 AA color tokens for the gemelli GUI, plus the contrast-ratio
//! calculation used to prove them (see `tokens` below).

use egui::Color32;

/// WCAG 2.1 relative-luminance contrast ratio between two colors.
/// Formula: <https://www.w3.org/TR/WCAG21/#dfn-contrast-ratio>.
///
/// Only exercised by this module's tests (there is no lib target to export
/// it from), hence `allow(dead_code)` outside `cfg(test)`.
#[cfg_attr(not(test), allow(dead_code))]
pub fn contrast_ratio(a: Color32, b: Color32) -> f64 {
    let luminance_a = relative_luminance(a);
    let luminance_b = relative_luminance(b);
    let (lighter, darker) = if luminance_a >= luminance_b {
        (luminance_a, luminance_b)
    } else {
        (luminance_b, luminance_a)
    };
    (lighter + 0.05) / (darker + 0.05)
}

#[cfg_attr(not(test), allow(dead_code))]
fn relative_luminance(color: Color32) -> f64 {
    let red = linearize(color.r());
    let green = linearize(color.g());
    let blue = linearize(color.b());
    0.2126 * red + 0.7152 * green + 0.0722 * blue
}

#[cfg_attr(not(test), allow(dead_code))]
fn linearize(channel: u8) -> f64 {
    let normalized = f64::from(channel) / 255.0;
    if normalized <= 0.03928 {
        normalized / 12.92
    } else {
        ((normalized + 0.055) / 1.055).powf(2.4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui::Color32;

    #[test]
    fn black_and_white_ratio_is_21() {
        let ratio = contrast_ratio(Color32::WHITE, Color32::BLACK);
        assert!((ratio - 21.0).abs() < 0.01, "expected ~21.0, got {ratio}");
    }

    #[test]
    fn same_color_ratio_is_1() {
        let gray = Color32::from_rgb(100, 100, 100);
        let ratio = contrast_ratio(gray, gray);
        assert!((ratio - 1.0).abs() < 0.0001, "expected 1.0, got {ratio}");
    }

    #[test]
    fn ratio_is_symmetric_in_argument_order() {
        let a = Color32::from_rgb(230, 230, 230);
        let b = Color32::from_rgb(26, 27, 30);
        assert!((contrast_ratio(a, b) - contrast_ratio(b, a)).abs() < 1e-9);
    }
}
