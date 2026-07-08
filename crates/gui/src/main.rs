//! GUI entry point for the webcam -> Spout/Syphon sharing tool.

use std::process::ExitCode;

mod app;
mod crop_editor;
mod fps_meter;
mod preview;
mod theme;
mod worker;

fn main() -> ExitCode {
    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 700.0])
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
