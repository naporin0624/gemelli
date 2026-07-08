//! WCAG 2.1 AA color tokens for the gemelli GUI — Cannelloni palette — plus
//! the contrast-ratio calculation used to prove them (see `tokens` below).

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

/// Dark-theme color tokens converted from Cannelloni's `panda.config.ts` oklch
/// primitives to sRGB `Color32`. Every token's contrast ratio against the
/// background(s) it is meant to sit on is proved by the tests in this module —
/// see the design doc (`docs/superpowers/specs/2026-07-08-distribution-prep-design.md`,
/// section 1) for the oklch source and the hand-computed numbers behind each choice.
pub mod tokens {
    use egui::Color32;

    /// Window background — Cannelloni `dark.canvas` (oklch 0.180 0 0).
    pub const BG_BASE: Color32 = Color32::from_rgb(18, 18, 18);
    /// Panel/sidebar/status-bar background — Cannelloni `dark.subtle` (oklch 0.225 0 0).
    pub const BG_PANEL: Color32 = Color32::from_rgb(28, 28, 28);
    /// Expanded-row background — Cannelloni `dark.muted` (oklch 0.270 0 0). Not yet
    /// consumed: reserved for the licenses window's expanded-entry background
    /// (design doc section 3, a later task) — `allow(dead_code)` until that call
    /// site exists.
    #[allow(dead_code)]
    pub const BG_MUTED: Color32 = Color32::from_rgb(38, 38, 38);

    /// Primary text — Cannelloni `gray.1` (oklch 0.952 0.004 265).
    pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(238, 239, 242);
    /// Secondary text — Cannelloni `gray.6` (oklch 0.845 0.008 265).
    pub const TEXT_MUTED: Color32 = Color32::from_rgb(201, 204, 209);
    /// Idle-state text — Cannelloni `gray.8` (oklch 0.700 0.013 265). Paired with
    /// the "○ stopped" text label at the call site — never color alone (WCAG 1.4.1).
    pub const TEXT_SUBTLE: Color32 = Color32::from_rgb(154, 158, 167);

    /// Publishing state / links / selection fill — Cannelloni `neon.blue`
    /// (oklch 0.700 0.235 260). Paired with the "● publishing" text label at the
    /// call site — never color alone (WCAG 1.4.1).
    /// Used as both a UI-component color (selection fill, 3.0:1 threshold) and as
    /// text (hyperlinks, the publishing label), so it is proved against the
    /// stricter 4.5:1 normal-text threshold.
    pub const ACCENT: Color32 = Color32::from_rgb(57, 150, 255);
    /// Hover-fill only — Cannelloni `neon.blueHover` (oklch 0.650 0.235 260). Not
    /// yet consumed: applying this to `Visuals::widgets.hovered` fills is future
    /// widget-hover styling work, out of this task's scope — `allow(dead_code)`
    /// until that call site exists.
    #[allow(dead_code)]
    pub const ACCENT_HOVER: Color32 = Color32::from_rgb(39, 133, 255);
    /// Slider-fill only — Cannelloni `neon.cyan` (oklch 0.820 0.130 200). Not yet
    /// consumed: no slider exists in the GUI yet — `allow(dead_code)` until one
    /// does. Proved at the 3.0:1 non-text threshold (WCAG 1.4.11) since it will
    /// only ever fill a widget, never render as text.
    #[cfg_attr(not(test), allow(dead_code))]
    pub const ACCENT_ALT: Color32 = Color32::from_rgb(52, 221, 229);

    /// Danger/error text. Deliberate deviation from the Cannelloni primitive
    /// `oklch(0.650 0.250 25)`: at that lightness this color only reaches
    /// 4.497:1 on `BG_PANEL` — just under the 4.5:1 AA threshold (Cannelloni
    /// itself only ever draws error text on `canvas`, not `subtle`, so the
    /// primitive never had to clear this bar). `gemelli`'s error banner renders
    /// on `BG_PANEL` (see `app.rs`), so L is bumped to 0.660, landing at 4.57:1.
    pub const DANGER: Color32 = Color32::from_rgb(255, 41, 57);

    /// 2px interactive-widget outline (`apply_theme`) — Cannelloni `dark.border`
    /// (oklch 0.520 0 0). Proved at the 3.0:1 non-text/UI-component threshold
    /// (WCAG 1.4.11), since it is a stroke, never text.
    pub const BORDER: Color32 = Color32::from_rgb(105, 105, 105);
    /// Non-informational divider lines only — Cannelloni `dark.borderSubtle`
    /// (oklch 0.380 0 0). Not yet consumed: reserved for the licenses window's
    /// hairline dividers (design doc section 3, a later task) — `allow(dead_code)`
    /// until that call site exists. No contrast proof needed: WCAG 1.4.11 exempts
    /// purely decorative, non-informational separators.
    #[allow(dead_code)]
    pub const BORDER_SUBTLE: Color32 = Color32::from_rgb(66, 66, 66);

    /// Crop-rect stroke. Drawn as a dual stroke (black outline + white core) at
    /// the `preview_ui` call site, since no single color has a provable contrast
    /// ratio against arbitrary live video content.
    pub const CROP_OVERLAY: Color32 = Color32::WHITE;
}

/// Applies the `tokens` palette to `ctx`'s `Visuals`. Called once at startup
/// from `GemelliApp::new`.
pub fn apply_theme(ctx: &egui::Context) {
    let mut visuals = egui::Visuals::dark();
    visuals.window_fill = tokens::BG_BASE;
    visuals.panel_fill = tokens::BG_PANEL;
    visuals.override_text_color = Some(tokens::TEXT_PRIMARY);
    visuals.weak_text_color = Some(tokens::TEXT_MUTED);
    visuals.hyperlink_color = tokens::ACCENT;

    // Inverted selection: Cannelloni draws the selected/active state as a solid
    // ACCENT fill with dark (`fg.onSolid`-equivalent) text on top, not the usual
    // light-text-on-dark-fill pairing. egui 0.35 renders selected-widget text
    // using `selection.stroke` as its fg_stroke color — `override_text_color`
    // does not reach selected widgets (verified in egui 0.35 source) — so
    // setting that to `BG_BASE` is what makes the text read as "dark on blue".
    visuals.selection.bg_fill = tokens::ACCENT;
    visuals.selection.stroke = egui::Stroke::new(1.0, tokens::BG_BASE);

    // Neo-brutalist "terminal-print" identity: no rounded corners anywhere,
    // on any widget interaction state, window, or menu/popup.
    visuals.window_corner_radius = egui::CornerRadius::ZERO;
    visuals.menu_corner_radius = egui::CornerRadius::ZERO;
    for widget in [
        &mut visuals.widgets.noninteractive,
        &mut visuals.widgets.inactive,
        &mut visuals.widgets.hovered,
        &mut visuals.widgets.active,
        &mut visuals.widgets.open,
    ] {
        widget.corner_radius = egui::CornerRadius::ZERO;
    }

    // 2px ink border on every *interactive* widget state. `noninteractive` is
    // deliberately left alone here — it is egui's passive-chrome style (window
    // outlines, separators), not an interactive widget, and is out of this
    // task's scope.
    let border_stroke = egui::Stroke::new(2.0, tokens::BORDER);
    visuals.widgets.inactive.bg_stroke = border_stroke;
    visuals.widgets.hovered.bg_stroke = border_stroke;
    visuals.widgets.active.bg_stroke = border_stroke;
    visuals.widgets.open.bg_stroke = border_stroke;

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
    fn text_primary_meets_normal_text_contrast_on_bg_panel() {
        assert!(contrast_ratio(tokens::TEXT_PRIMARY, tokens::BG_PANEL) >= 4.5);
    }

    #[test]
    fn text_muted_meets_normal_text_contrast_on_bg_base() {
        assert!(contrast_ratio(tokens::TEXT_MUTED, tokens::BG_BASE) >= 4.5);
    }

    #[test]
    fn text_muted_meets_normal_text_contrast_on_bg_panel() {
        assert!(contrast_ratio(tokens::TEXT_MUTED, tokens::BG_PANEL) >= 4.5);
    }

    #[test]
    fn text_subtle_meets_normal_text_contrast_on_bg_base() {
        // TEXT_SUBTLE renders as the "○ stopped" *text* label (statusbar_ui), so
        // it must clear the 4.5:1 normal-text threshold, not a 3.0:1 UI-component
        // threshold.
        assert!(contrast_ratio(tokens::TEXT_SUBTLE, tokens::BG_BASE) >= 4.5);
    }

    #[test]
    fn text_subtle_meets_normal_text_contrast_on_bg_panel() {
        assert!(contrast_ratio(tokens::TEXT_SUBTLE, tokens::BG_PANEL) >= 4.5);
    }

    #[test]
    fn accent_meets_normal_text_contrast_on_bg_base() {
        // ACCENT renders as the "● publishing" text label and as hyperlink text,
        // so — like TEXT_SUBTLE above — it is held to the 4.5:1 normal-text bar.
        assert!(contrast_ratio(tokens::ACCENT, tokens::BG_BASE) >= 4.5);
    }

    #[test]
    fn accent_meets_normal_text_contrast_on_bg_panel() {
        assert!(contrast_ratio(tokens::ACCENT, tokens::BG_PANEL) >= 4.5);
    }

    #[test]
    fn inverted_selection_text_meets_normal_text_contrast() {
        // `apply_theme` paints selected text as BG_BASE on an ACCENT fill (the
        // inverted-selection scheme) — prove that pairing directly, in the
        // order it is actually rendered.
        assert!(contrast_ratio(tokens::BG_BASE, tokens::ACCENT) >= 4.5);
    }

    #[test]
    fn danger_meets_normal_text_contrast_on_bg_base() {
        assert!(contrast_ratio(tokens::DANGER, tokens::BG_BASE) >= 4.5);
    }

    /// The banner (`app.rs`'s `DANGER`-colored error label) renders on `BG_PANEL`, not
    /// `BG_BASE` — `egui::Panel::top` inherits `visuals.panel_fill`. Retargets the contrast
    /// proof at the surface `DANGER` actually sits on; the `BG_BASE` assertion above is kept
    /// too since it still holds and other `DANGER` usages may sit on the window background.
    #[test]
    fn danger_meets_normal_text_contrast_on_bg_panel() {
        assert!(contrast_ratio(tokens::DANGER, tokens::BG_PANEL) >= 4.5);
    }

    #[test]
    fn border_meets_non_text_contrast_on_bg_base() {
        // WCAG 1.4.11 non-text/UI-component threshold — BORDER is only ever a
        // stroke, never text.
        assert!(contrast_ratio(tokens::BORDER, tokens::BG_BASE) >= 3.0);
    }

    #[test]
    fn border_meets_non_text_contrast_on_bg_panel() {
        assert!(contrast_ratio(tokens::BORDER, tokens::BG_PANEL) >= 3.0);
    }

    #[test]
    fn accent_alt_meets_non_text_contrast_on_bg_base() {
        assert!(contrast_ratio(tokens::ACCENT_ALT, tokens::BG_BASE) >= 3.0);
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
        assert_eq!(visuals.hyperlink_color, tokens::ACCENT);
    }

    #[test]
    fn apply_theme_inverts_selection_colors() {
        let ctx = egui::Context::default();
        apply_theme(&ctx);
        let visuals = ctx.global_style().visuals.clone();
        assert_eq!(visuals.selection.bg_fill, tokens::ACCENT);
        assert_eq!(visuals.selection.stroke.color, tokens::BG_BASE);
    }

    #[test]
    fn apply_theme_zeroes_all_corner_radii() {
        let ctx = egui::Context::default();
        apply_theme(&ctx);
        let visuals = ctx.global_style().visuals.clone();
        assert_eq!(visuals.window_corner_radius, egui::CornerRadius::ZERO);
        assert_eq!(visuals.menu_corner_radius, egui::CornerRadius::ZERO);
        assert_eq!(visuals.widgets.noninteractive.corner_radius, egui::CornerRadius::ZERO);
        assert_eq!(visuals.widgets.inactive.corner_radius, egui::CornerRadius::ZERO);
        assert_eq!(visuals.widgets.hovered.corner_radius, egui::CornerRadius::ZERO);
        assert_eq!(visuals.widgets.active.corner_radius, egui::CornerRadius::ZERO);
        assert_eq!(visuals.widgets.open.corner_radius, egui::CornerRadius::ZERO);
    }

    #[test]
    fn apply_theme_sets_border_stroke_on_interactive_widgets() {
        let ctx = egui::Context::default();
        apply_theme(&ctx);
        let visuals = ctx.global_style().visuals.clone();
        let expected = egui::Stroke::new(2.0, tokens::BORDER);
        assert_eq!(visuals.widgets.inactive.bg_stroke, expected);
        assert_eq!(visuals.widgets.hovered.bg_stroke, expected);
        assert_eq!(visuals.widgets.active.bg_stroke, expected);
        assert_eq!(visuals.widgets.open.bg_stroke, expected);
    }
}
