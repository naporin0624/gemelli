//! GUI entry point for the webcam -> Spout/Syphon sharing tool.

use std::process::ExitCode;

mod app;
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
    let options = eframe::NativeOptions {
        // Sizes measured against the compact label-left controls grid (`app::controls_ui`):
        // controls panel 206px + statusbar 22px = 228px of fixed chrome. Initial height is
        // exactly that chrome plus a 16:9 preview at the initial width (360 * 9/16 = 202.5,
        // rounded up to keep the preview from being truncated below its exact 16:9 slice) —
        // the smallest default that still shows a full-aspect preview with no slack. Min
        // height instead leaves just the >=120px preview floor, which lands below the initial
        // height now that chrome is a measured constant rather than an estimate.
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([360.0, 431.0])
            .with_min_inner_size([300.0, 350.0])
            .with_title("gemelli"),
        ..Default::default()
    };

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
