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

/// Dark-theme color tokens. Every token's contrast ratio against the
/// background(s) it is meant to sit on is proved by the tests in this
/// module — see the plan doc for the hand-computed numbers behind each
/// choice.
pub mod tokens {
    // Several tokens (e.g. `ACCENT_IDLE`, `DANGER`, `CROP_OVERLAY`) aren't
    // consumed by production code until later GUI tasks wire up the status
    // label, danger button, and crop editor; until then this module's
    // contrast-proof tests are their only caller, so they'd otherwise be
    // flagged dead in the non-test build.
    #![cfg_attr(not(test), allow(dead_code))]

    use egui::Color32;

    /// Window background. Deliberately not pure black — `#1a1b1e`.
    pub const BG_BASE: Color32 = Color32::from_rgb(26, 27, 30);
    /// Sidebar/status-bar background — a hair lighter than `BG_BASE` so
    /// panels read as a distinct layer.
    pub const BG_PANEL: Color32 = Color32::from_rgb(33, 34, 38);
    pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(230, 230, 230);
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(160, 160, 168);
    /// Publishing state. Paired with the "● publishing" text label at the
    /// call site — never color alone (WCAG 1.4.1).
    pub const ACCENT_PUBLISH: Color32 = Color32::from_rgb(61, 220, 132);
    /// Idle state. Paired with the "○ stopped" text label at the call site.
    pub const ACCENT_IDLE: Color32 = Color32::from_rgb(125, 133, 144);
    pub const DANGER: Color32 = Color32::from_rgb(255, 107, 107);
    /// Crop-rect stroke. Drawn as a dual stroke (black outline + white
    /// core) at the crop_editor.rs call site, since no single color has a
    /// provable contrast ratio against arbitrary live video content. Not
    /// exercised by a contrast test (see module doc), so it needs its own
    /// unconditional `allow`.
    #[allow(dead_code)]
    pub const CROP_OVERLAY: Color32 = Color32::WHITE;
}

/// Applies the `tokens` palette to `ctx`'s `Visuals`. Called once at
/// startup from `GemelliApp::new`.
pub fn apply_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.window_fill = tokens::BG_BASE;
    visuals.panel_fill = tokens::BG_PANEL;
    visuals.override_text_color = Some(tokens::TEXT_PRIMARY);
    visuals.weak_text_color = Some(tokens::TEXT_MUTED);
    visuals.hyperlink_color = tokens::ACCENT_PUBLISH;
    visuals.selection.bg_fill = tokens::ACCENT_PUBLISH;
    visuals.selection.stroke = egui::Stroke::new(1.0, tokens::TEXT_PRIMARY);
    ctx.set_visuals(visuals);
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

    #[test]
    fn text_primary_meets_normal_text_contrast_on_bg_base() {
        assert!(contrast_ratio(tokens::TEXT_PRIMARY, tokens::BG_BASE) >= 4.5);
    }

    #[test]
    fn text_muted_meets_normal_text_contrast_on_bg_base() {
        assert!(contrast_ratio(tokens::TEXT_MUTED, tokens::BG_BASE) >= 4.5);
    }

    #[test]
    fn danger_meets_normal_text_contrast_on_bg_base() {
        assert!(contrast_ratio(tokens::DANGER, tokens::BG_BASE) >= 4.5);
    }

    #[test]
    fn accent_publish_meets_ui_component_contrast_on_bg_panel() {
        assert!(contrast_ratio(tokens::ACCENT_PUBLISH, tokens::BG_PANEL) >= 3.0);
    }

    #[test]
    fn accent_idle_meets_ui_component_contrast_on_bg_panel() {
        assert!(contrast_ratio(tokens::ACCENT_IDLE, tokens::BG_PANEL) >= 3.0);
    }

    #[test]
    fn apply_theme_sets_dark_mode_and_token_fills() {
        let ctx = egui::Context::default();
        apply_theme(&ctx);
        let visuals = ctx.global_style().visuals.clone();
        assert!(visuals.dark_mode);
        assert_eq!(visuals.window_fill, tokens::BG_BASE);
        assert_eq!(visuals.panel_fill, tokens::BG_PANEL);
        assert_eq!(visuals.override_text_color, Some(tokens::TEXT_PRIMARY));
        assert_eq!(visuals.weak_text_color, Some(tokens::TEXT_MUTED));
    }
}
