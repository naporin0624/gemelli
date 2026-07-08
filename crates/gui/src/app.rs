//! `GemelliApp`: the eframe root. This task only wires the window shell —
//! device/transform/worker state lands in a later task.

pub struct GemelliApp {}

impl GemelliApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        Self {}
    }
}

impl eframe::App for GemelliApp {
    // `logic` (state-only, called before painting) is optional and defaults
    // to a no-op; this app has no per-frame state yet, so only the required
    // `ui` method (painting) is implemented.
    fn ui(&mut self, ui: &mut eframe::egui::Ui, _frame: &mut eframe::Frame) {
        eframe::egui::CentralPanel::default().show(ui, |ui| {
            ui.heading("gemelli");
        });
    }
}
