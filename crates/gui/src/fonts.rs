//! Registers LINE Seed JP as the app's proportional/monospace font so Japanese UI labels
//! (`sidebar.rs`, `app.rs`) render with real glyphs instead of egui's tofu boxes.

use std::sync::Arc;

/// Installs LINE Seed JP into `ctx`'s font set, ahead of egui's built-in fonts.
pub fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        "LINESeedJP".to_owned(),
        // Compile-time dependency on `scripts/fetch-fonts.sh` having been run first: that
        // script downloads the LINE Seed release and writes it to vendor/fonts/. If it
        // hasn't been run, this `include_bytes!` fails the build with a plain file-not-found
        // error, which is the intended signal (matches the naporin0624/bucatini precedent) —
        // no extra error handling needed here.
        Arc::new(egui::FontData::from_static(include_bytes!(
            "../../../vendor/fonts/LINESeedJP-Regular.ttf"
        ))),
    );

    fonts
        .families
        .entry(egui::FontFamily::Proportional)
        .or_default()
        .insert(0, "LINESeedJP".to_owned());
    fonts.families.entry(egui::FontFamily::Monospace).or_default().push("LINESeedJP".to_owned());

    ctx.set_fonts(fonts);
}
