//! GUI entry point for the webcam -> Spout/Syphon sharing tool.

use std::process::ExitCode;

mod app;
mod config;
mod crop_editor;
mod fonts;
mod fps_meter;
mod licenses;
mod menu;
mod preview;
mod sidebar;
mod theme;
mod widgets;
mod worker;

fn main() -> ExitCode {
    // Sizes measured against the compact label-left controls grid (`app::controls_ui`):
    // controls panel 206px + statusbar 22px = 228px of fixed chrome. Initial height is
    // exactly that chrome plus a 16:9 preview at the initial width (360 * 9/16 = 202.5,
    // rounded up to keep the preview from being truncated below its exact 16:9 slice) —
    // the smallest default that still shows a full-aspect preview with no slack. Min
    // height instead leaves just the >=120px preview floor (228 + 120 = 348, rounded to
    // 350), which stays below the initial height at every width.
    let mut viewport = eframe::egui::ViewportBuilder::default()
        .with_inner_size([360.0, 431.0])
        .with_min_inner_size([300.0, 350.0])
        .with_title("gemelli");

    match app_icon() {
        Ok(icon) => viewport = viewport.with_icon(icon),
        Err(reason) => eprintln!("gemelli-gui: failed to load app icon: {reason}"),
    }

    let options = eframe::NativeOptions { viewport, ..Default::default() };

    let result = eframe::run_native(
        "gemelli",
        options,
        Box::new(|cc| Ok(Box::new(app::GemelliApp::new(cc)))),
    );

    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(reason) => {
            eprintln!("gemelli-gui failed to run: {reason}");
            ExitCode::FAILURE
        }
    }
}

fn app_icon() -> Result<eframe::egui::IconData, String> {
    eframe::icon_data::from_png_bytes(include_bytes!("../assets/icon.png"))
        .map_err(|reason| reason.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_icon_decodes_the_bundled_1024_square_png() {
        let icon = app_icon().unwrap();

        assert_eq!(icon.width, 1024);
        assert_eq!(icon.height, 1024);
        assert_eq!(icon.rgba.len(), 1024 * 1024 * 4);
    }
}
