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
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([400.0, 860.0])
            .with_min_inner_size([360.0, 640.0])
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
