//! GUI application state and the eframe painting loop.

use std::sync::atomic::Ordering;
use std::sync::{Arc, PoisonError, mpsc};
use std::time::Instant;

use gemelli_core::capture::{self, DeviceInfo};
use gemelli_core::frame::Frame;
use gemelli_core::transform::{CropRect, Flip, Rotation, TransformConfig};

use crate::fps_meter::FpsMeter;
use crate::preview;
use crate::sidebar::{self, ScaleInput};
use crate::theme;
use crate::worker::{SharedState, WorkerError, WorkerHandle, WorkerSpec, spawn_worker};

/// (h, v) toggle state -> `Flip`. Exhaustive over all four bool pairs — no `_` arm, so a new
/// `Flip` variant added upstream would force this match to be revisited instead of silently
/// falling through.
fn flip_from_toggles(h: bool, v: bool) -> Flip {
    match (h, v) {
        (false, false) => Flip::Keep,
        (true, false) => Flip::Horizontal,
        (false, true) => Flip::Vertical,
        (true, true) => Flip::Both,
    }
}

/// Rebuilds the full `TransformConfig` from the sidebar's current widget state. Called after
/// every widget edit that affects the transform chain; the result is stored into
/// `shared.transform` by `GemelliApp::push_transform`.
fn build_transform(
    crop: Option<CropRect>,
    rotation: Rotation,
    flip_h: bool,
    flip_v: bool,
    scale_input: ScaleInput,
) -> TransformConfig {
    TransformConfig {
        crop,
        rotation,
        flip: flip_from_toggles(flip_h, flip_v),
        scale: sidebar::scale_from_input(scale_input),
    }
}

/// What the big preview pane currently shows. `Output` = the transformed frame (identical to
/// what Syphon publishes); `CropEdit` = the raw pre-transform frame with a draggable crop
/// overlay (Task 7), since crop coordinates are 1:1 with the raw frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewMode {
    Output,
    CropEdit,
}

pub struct GemelliApp {
    shared: Arc<SharedState>,
    worker: Option<WorkerHandle>,
    errors_tx: mpsc::Sender<WorkerError>,
    errors_rx: mpsc::Receiver<WorkerError>,

    devices: Vec<DeviceInfo>,
    selected_device: usize,
    requested_fps: Option<u32>,
    server_name: String,

    rotation: Rotation,
    flip_h: bool,
    flip_v: bool,
    scale_input: ScaleInput,
    crop: Option<CropRect>,
    drag: Option<crate::crop_editor::DragState>,

    preview_mode: PreviewMode,
    banner: Option<String>,

    fps: FpsMeter,
    last_frames_published: u64,
    texture: Option<egui::TextureHandle>,
    input_dims: Option<(u32, u32)>,
    output_dims: Option<(u32, u32)>,
    preview_dims: Option<(u32, u32)>,
}

impl GemelliApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::apply_theme(&cc.egui_ctx);
        crate::fonts::install_fonts(&cc.egui_ctx);

        let (devices, banner) = match capture::list_devices() {
            Ok(devices) => (devices, None),
            Err(error) => (Vec::new(), Some(error.to_string())),
        };
        let (errors_tx, errors_rx) = mpsc::channel();
        let shared = Arc::new(SharedState::new(TransformConfig::default()));

        Self {
            shared,
            worker: None,
            errors_tx,
            errors_rx,
            devices,
            selected_device: 0,
            requested_fps: None,
            server_name: "gemelli".to_string(),
            rotation: Rotation::R0,
            flip_h: false,
            flip_v: false,
            scale_input: ScaleInput::default(),
            crop: None,
            drag: None,
            preview_mode: PreviewMode::Output,
            banner,
            fps: FpsMeter::new(),
            last_frames_published: 0,
            texture: None,
            input_dims: None,
            output_dims: None,
            preview_dims: None,
        }
    }

    fn push_transform(&self) {
        let config =
            build_transform(self.crop, self.rotation, self.flip_h, self.flip_v, self.scale_input);
        self.shared.transform.store(Arc::new(config));
    }

    fn reload_devices(&mut self) {
        match capture::list_devices() {
            Ok(devices) => {
                self.devices = devices;
                if self.selected_device >= self.devices.len() {
                    self.selected_device = 0;
                }
            }
            Err(error) => self.banner = Some(error.to_string()),
        }
    }

    fn stop_worker(&mut self) {
        if let Some(mut worker) = self.worker.take() {
            worker.stop();
        }
    }

    fn start_worker(&mut self) {
        self.stop_worker();
        self.banner = None;
        let Some(device) = self.devices.get(self.selected_device) else {
            self.banner = Some("no capture device selected — refresh the device list".to_string());
            return;
        };
        let spec = WorkerSpec {
            device_index: device.index,
            requested_fps: self.requested_fps,
            server_name: self.server_name.clone(),
        };
        self.worker = Some(spawn_worker(spec, Arc::clone(&self.shared), self.errors_tx.clone()));
    }

    /// THE single consumption point for `errors_rx` — `worker::run_capture` sends an error and
    /// then returns, ending its thread, so receiving one here means the worker is no longer
    /// running even though nothing else told us so directly.
    fn drain_errors(&mut self) {
        while let Ok(error) = self.errors_rx.try_recv() {
            self.banner = Some(error.to_string());
            self.worker = None;
        }
    }

    fn refresh_preview(&mut self, ctx: &egui::Context) {
        let raw = self.shared.latest_raw.lock().unwrap_or_else(PoisonError::into_inner).clone();
        let output =
            self.shared.latest_output.lock().unwrap_or_else(PoisonError::into_inner).clone();

        self.input_dims = raw.as_ref().map(|frame| (frame.width(), frame.height()));
        self.output_dims = output.as_ref().map(|frame| (frame.width(), frame.height()));

        let displayed = match self.preview_mode {
            PreviewMode::Output => output,
            PreviewMode::CropEdit => raw,
        };
        match displayed {
            Some(frame) => {
                self.preview_dims = Some((frame.width(), frame.height()));
                self.update_texture(ctx, &frame);
            }
            None => self.preview_dims = None,
        }

        self.tick_fps();
    }

    fn update_texture(&mut self, ctx: &egui::Context, frame: &Frame) {
        let image = preview::color_image(frame);
        match &mut self.texture {
            Some(texture) => texture.set(image, egui::TextureOptions::LINEAR),
            None => {
                self.texture =
                    Some(ctx.load_texture("preview", image, egui::TextureOptions::LINEAR))
            }
        }
    }

    fn tick_fps(&mut self) {
        let published = self.shared.frames_published.load(Ordering::Relaxed);
        let delta = published.saturating_sub(self.last_frames_published);
        self.last_frames_published = published;
        let now = Instant::now();
        for _ in 0..delta {
            self.fps.record(now);
        }
    }

    fn sidebar_ui(&mut self, ui: &mut egui::Ui) {
        ui.heading("Device");
        ui.horizontal(|ui| {
            let device_changed =
                sidebar::device_panel(ui, &self.devices, &mut self.selected_device);
            if sidebar::refresh_button(ui) {
                self.reload_devices();
            }
            if device_changed && self.worker.is_some() {
                self.start_worker();
            }
        });

        ui.add_space(8.0);
        ui.heading("Rotate");
        if sidebar::rotate_panel(ui, &mut self.rotation) {
            self.push_transform();
        }

        ui.add_space(8.0);
        ui.heading("Flip");
        if sidebar::flip_panel(ui, &mut self.flip_h, &mut self.flip_v) {
            self.push_transform();
        }

        ui.add_space(8.0);
        ui.heading("Crop");
        let crop_action =
            crate::sidebar::crop_panel(ui, self.crop, self.preview_mode == PreviewMode::CropEdit);
        match crop_action {
            crate::sidebar::CropAction::None => {}
            crate::sidebar::CropAction::ToggleEdit => {
                self.preview_mode = match self.preview_mode {
                    PreviewMode::Output => PreviewMode::CropEdit,
                    PreviewMode::CropEdit => PreviewMode::Output,
                };
            }
            crate::sidebar::CropAction::Add => match self.input_dims {
                Some((frame_w, frame_h)) => {
                    self.crop = Some(crate::crop_editor::seed_rect(frame_w, frame_h));
                    self.push_transform();
                }
                None => {
                    self.banner =
                        Some("no frame yet — start capture before adding a crop".to_string());
                }
            },
            crate::sidebar::CropAction::Clear => {
                self.crop = None;
                self.drag = None;
                self.push_transform();
            }
            crate::sidebar::CropAction::Edited(rect) => {
                let clamped = match self.input_dims {
                    Some((frame_w, frame_h)) => {
                        crate::crop_editor::clamp_rect(rect, frame_w, frame_h)
                    }
                    None => rect,
                };
                self.crop = Some(clamped);
                self.push_transform();
            }
        }

        ui.add_space(8.0);
        ui.heading("Scale");
        if sidebar::scale_panel(ui, &mut self.scale_input) {
            self.push_transform();
        }

        ui.add_space(8.0);
        ui.heading("Server name");
        let server_name_committed = sidebar::server_name_panel(ui, &mut self.server_name);
        if server_name_committed && self.worker.is_some() {
            self.start_worker();
        }

        ui.add_space(8.0);
        let running = self.worker.as_ref().is_some_and(WorkerHandle::is_running);
        if sidebar::transport_button(ui, running) {
            if running {
                self.stop_worker();
            } else {
                self.start_worker();
            }
        }
    }

    fn statusbar_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let dims_text = match (self.input_dims, self.output_dims) {
                (Some((iw, ih)), Some((ow, oh))) => format!("{iw}x{ih} -> {ow}x{oh}"),
                (Some((iw, ih)), None) => format!("{iw}x{ih} -> --"),
                (None, _) => "no signal".to_string(),
            };
            ui.label(dims_text);
            ui.separator();

            let rate = self.fps.rate(Instant::now());
            ui.label(format!("{rate:.0} fps"));
            ui.separator();

            let running = self.worker.as_ref().is_some_and(WorkerHandle::is_running);
            if running {
                ui.colored_label(theme::tokens::ACCENT_PUBLISH, "\u{25cf} publishing");
            } else {
                ui.colored_label(theme::tokens::ACCENT_IDLE, "\u{25cb} stopped");
            }
        });
    }

    fn preview_ui(&mut self, ui: &mut egui::Ui) {
        let avail = ui.available_rect_before_wrap();
        let (Some(texture), Some((frame_w, frame_h))) = (&self.texture, self.preview_dims) else {
            ui.centered_and_justified(|ui| {
                ui.label("No preview — start capture to see the feed");
            });
            return;
        };
        let draw = preview::fit_rect(frame_w, frame_h, avail);
        // `egui::Image::new(&TextureHandle)` defaults to `ImageFit::Exact(tex.size)`
        // (the texture's native pixel size), ignoring the rect `ui.put` allocates it
        // into — so a 1080p texture drawn into a smaller `draw` rect overflowed and
        // got clipped by the panel instead of being scaled down to fit. `draw` is
        // already the aspect-correct letterboxed size from `fit_rect`, so forcing
        // the widget to exactly that size just makes it honor the computed layout.
        ui.put(draw, egui::Image::new(texture).fit_to_exact_size(draw.size()));

        if self.preview_mode == PreviewMode::CropEdit
            && let Some(rect) = self.crop
        {
            let mapping = crate::crop_editor::CropMapping {
                frame_width: frame_w,
                frame_height: frame_h,
                draw,
            };
            let rect_screen = mapping.to_screen(rect);

            // Dual-stroke overlay (contract token note): a wider black halo painted first,
            // then a thinner CROP_OVERLAY (white) line on the same edge, so the rect reads
            // against both bright and dark video content.
            let painter = ui.painter_at(draw);
            painter.rect_stroke(
                rect_screen,
                0.0,
                egui::Stroke::new(3.0, egui::Color32::BLACK),
                egui::StrokeKind::Middle,
            );
            painter.rect_stroke(
                rect_screen,
                0.0,
                egui::Stroke::new(1.0, theme::tokens::CROP_OVERLAY),
                egui::StrokeKind::Middle,
            );

            let response =
                ui.interact(draw, ui.id().with("crop_overlay"), egui::Sense::click_and_drag());

            if response.drag_started()
                && let Some(pointer) = response.interact_pointer_pos()
                && let Some(mode) = crate::crop_editor::hit_test(rect_screen, pointer)
            {
                self.drag = Some(crate::crop_editor::DragState {
                    mode,
                    start_rect: rect,
                    start_pointer: pointer,
                });
            }

            if response.dragged()
                && let (Some(drag), Some(pointer)) = (&self.drag, response.interact_pointer_pos())
            {
                let updated = crate::crop_editor::apply_drag(drag, &mapping, pointer);
                self.crop = Some(updated);
                self.push_transform();
            }

            if response.drag_stopped() {
                self.drag = None;
            }
        }
    }
}

impl eframe::App for GemelliApp {
    // `logic` (state-only, called before painting) is optional and defaults to a no-op; all of
    // this app's state updates happen inline with painting inside `ui`, so `logic` is unused.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.drain_errors();
        self.refresh_preview(ui.ctx());

        if let Some(message) = self.banner.clone() {
            egui::Panel::top("banner").show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(theme::tokens::DANGER, &message);
                    if ui.button("Dismiss").clicked() {
                        self.banner = None;
                    }
                });
            });
        }

        egui::Panel::left("sidebar").resizable(false).min_size(220.0).show(ui, |ui| {
            self.sidebar_ui(ui);
        });

        egui::Panel::bottom("statusbar").show(ui, |ui| {
            self.statusbar_ui(ui);
        });

        egui::CentralPanel::default().show(ui, |ui| {
            self.preview_ui(ui);
        });

        // The capture thread pushes frames asynchronously (SharedState), not through egui's own
        // event loop, so nothing else would trigger a repaint once idle — request one every
        // frame to keep the preview and fps counter live.
        ui.ctx().request_repaint();
    }
}

#[cfg(test)]
mod tests {
    use gemelli_core::transform::{CropRect, Flip, Rotation, ScaleSpec, TransformConfig};

    use super::{build_transform, flip_from_toggles};
    use crate::sidebar::ScaleInput;

    #[test]
    fn flip_from_toggles_covers_all_four_combinations() {
        let cases = [
            (false, false, Flip::Keep),
            (true, false, Flip::Horizontal),
            (false, true, Flip::Vertical),
            (true, true, Flip::Both),
        ];

        for (h, v, expected) in cases {
            assert_eq!(flip_from_toggles(h, v), expected, "h={h} v={v}");
        }
    }

    #[test]
    fn build_transform_assembles_all_fields() {
        let crop = Some(CropRect { width: 100, height: 80, x: 10, y: 5 });
        let config = build_transform(crop, Rotation::R90, true, false, ScaleInput::Factor(0.5));

        assert_eq!(
            config,
            TransformConfig {
                crop,
                rotation: Rotation::R90,
                flip: Flip::Horizontal,
                scale: Some(ScaleSpec::Factor(0.5)),
            }
        );
    }

    #[test]
    fn build_transform_defaults_to_no_op() {
        let config = build_transform(None, Rotation::R0, false, false, ScaleInput::Off);

        assert_eq!(config, TransformConfig::default());
    }
}
