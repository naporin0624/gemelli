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
use crate::widgets;
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

/// `Rotation` <-> segmented-control index, in the `0° / 90° / 180° / 270°` cell order the design
/// doc specifies.
fn rotation_segment_index(rotation: Rotation) -> usize {
    match rotation {
        Rotation::R0 => 0,
        Rotation::R90 => 1,
        Rotation::R180 => 2,
        Rotation::R270 => 3,
    }
}

/// Inverse of `rotation_segment_index`. An index of 3 or greater clamps to R270 rather than
/// panicking — `segmented`'s `selected` is a plain `usize` with no compile-time bound tying it to
/// exactly 4 cells.
fn rotation_from_segment_index(index: usize) -> Rotation {
    match index {
        0 => Rotation::R0,
        1 => Rotation::R90,
        2 => Rotation::R180,
        _ => Rotation::R270,
    }
}

/// Re-clamps `crop` against a `width x height` frame, for when the capture device's frame size
/// changes underneath an active crop (e.g. switching to a camera with a different native
/// resolution). Returns `Some(new_rect)` only when clamping actually moved/shrank the rect —
/// `None` means `crop` still fits as-is, so the caller has nothing to push. Pure wrapper around
/// `crop_editor::clamp_rect`, which is itself pure and already covers the min-size/bounds
/// invariant; this just adds the "did it actually change" check `refresh_preview` needs to
/// decide whether a `push_transform` is warranted.
fn refit_crop(crop: CropRect, width: u32, height: u32) -> Option<CropRect> {
    let clamped = crate::crop_editor::clamp_rect(crop, width, height);
    (clamped != crop).then_some(clamped)
}

/// Empties `rx` of every message currently queued, discarding them. Extracted as its own
/// function (rather than inlined in `start_worker`) so the drain-vs-not-drain logic itself is
/// unit-testable without spawning a real capture thread — `start_worker`'s own body isn't
/// testable that way since it opens real camera/publisher resources.
fn drain_stale_errors(rx: &mpsc::Receiver<WorkerError>) {
    while rx.try_recv().is_ok() {}
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
    licenses: crate::licenses::LicensesWindow,

    fps: FpsMeter,
    last_frames_published: u64,
    texture: Option<egui::TextureHandle>,
    input_dims: Option<(u32, u32)>,
    output_dims: Option<(u32, u32)>,
    preview_dims: Option<(u32, u32)>,
    /// `(frames_published, preview_mode, dims)` as of the last `update_texture` upload.
    /// `refresh_preview` is called every repaint (up to display refresh rate) but the worker
    /// only publishes a new frame at capture rate, so most repaints see an identical frame —
    /// re-uploading it to the GPU every time is wasted work this key lets `refresh_preview`
    /// skip. Any of the three fields changing (new frame, mode swap between Output/CropEdit, or
    /// the frame's own dims changing) means the pixels to display are no longer the ones already
    /// on the texture.
    last_uploaded: Option<(u64, PreviewMode, (u32, u32))>,

    /// `None` when `menu::build_app_menu()` failed at startup (see `GemelliApp::new`)
    /// — the app still runs, just without a menu bar.
    menu: Option<crate::menu::AppMenu>,
}

impl GemelliApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        theme::apply_theme(&cc.egui_ctx);
        crate::fonts::install_fonts(&cc.egui_ctx);

        let menu = match crate::menu::build_app_menu() {
            Ok(menu) => Some(menu),
            Err(reason) => {
                eprintln!("gemelli-gui: failed to build app menu: {reason}");
                None
            }
        };

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
            licenses: crate::licenses::LicensesWindow::default(),
            fps: FpsMeter::new(),
            last_frames_published: 0,
            texture: None,
            input_dims: None,
            output_dims: None,
            preview_dims: None,
            last_uploaded: None,
            menu,
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
        // INVARIANT: `stop_worker` above blocks until the old worker thread is joined
        // (`WorkerHandle::stop`), so any `WorkerError` still sitting in `errors_rx` at this
        // point was necessarily sent by that now-dead thread — it cannot belong to the new
        // worker spawned below. Without this drain, `drain_errors` (the single consumption
        // point for this channel) would read that stale error on a later frame, re-raise the
        // banner, and set `self.worker = None`, killing the brand-new worker for an error
        // that no longer applies.
        drain_stale_errors(&self.errors_rx);
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

    /// Drains this frame's menu activations and applies each one. Exhaustive
    /// match over `MenuAction` — a new variant added upstream forces this match
    /// to be revisited instead of silently no-op'ing.
    fn poll_menu_actions(&mut self, ctx: &egui::Context) {
        let Some(menu) = &self.menu else { return };
        for action in menu.poll_actions() {
            match action {
                crate::menu::MenuAction::OpenLicenses => self.licenses.request_open(ctx),
            }
        }
    }

    fn refresh_preview(&mut self, ctx: &egui::Context) {
        let raw = self.shared.latest_raw.lock().unwrap_or_else(PoisonError::into_inner).clone();
        let output =
            self.shared.latest_output.lock().unwrap_or_else(PoisonError::into_inner).clone();

        let new_input_dims = raw.as_ref().map(|frame| (frame.width(), frame.height()));
        // The camera's frame size can change out from under an active crop (switching to a
        // device with a different native resolution): `crop_panel`'s numeric-edit path already
        // re-clamps on every keystroke (`CropAction::Edited`), but nothing previously revalidated
        // `self.crop` when *this* dimension change was the cause. An out-of-bounds crop then
        // fails `transform::apply` on every frame — the worker sends a `TransformError`,
        // `drain_errors` tears it down, and the GUI error-loops trying to restart with the same
        // bad crop. Only re-clamp when the dims actually changed (not every frame) and only push
        // when the clamp actually moved something (`refit_crop`'s `None` case).
        if let (Some((width, height)), Some(crop)) = (new_input_dims, self.crop)
            && new_input_dims != self.input_dims
            && let Some(refit) = refit_crop(crop, width, height)
        {
            self.crop = Some(refit);
            self.push_transform();
        }

        self.input_dims = new_input_dims;
        self.output_dims = output.as_ref().map(|frame| (frame.width(), frame.height()));

        let displayed = match self.preview_mode {
            PreviewMode::Output => output,
            PreviewMode::CropEdit => raw,
        };
        match displayed {
            Some(frame) => {
                let dims = (frame.width(), frame.height());
                self.preview_dims = Some(dims);

                // The worker publishes at capture rate, not display refresh rate, so most
                // calls here see the exact same frame as last time — re-uploading identical
                // pixels to the GPU on every repaint is pure waste. Re-upload only when the
                // key (frame count, mode, dims) actually moved.
                let published = self.shared.frames_published.load(Ordering::Relaxed);
                let key = (published, self.preview_mode, dims);
                if self.last_uploaded != Some(key) {
                    self.update_texture(ctx, &frame);
                    self.last_uploaded = Some(key);
                }
            }
            None => {
                self.preview_dims = None;
                self.last_uploaded = None;
            }
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

    fn controls_ui(&mut self, ui: &mut egui::Ui) {
        // Every control group is one `ROW_HEIGHT` row, label and control on the same line, so
        // this one spacing override is what produces the grid's vertical density — each
        // `labeled_row`/`action_button` call below is a top-level widget in this panel's default-
        // vertical layout and gets this gap for free, with no per-group `ui.add_space` needed.
        ui.spacing_mut().item_spacing.y = 5.0;
        // Combo boxes, `DragValue`s, sliders, and default egui buttons all clamp their own
        // minimum height to `interact_size.y` — overriding it here once is what makes every
        // non-custom-painted control in this row grid match the 24px rows `segmented`/
        // `icon_button` already paint at, instead of relying on font-size coincidence to land
        // near 24px.
        ui.spacing_mut().interact_size.y = widgets::ROW_HEIGHT;

        widgets::labeled_row(ui, "Device", |ui| {
            ui.horizontal(|ui| {
                // egui's `horizontal` layout has no "fill remaining space" primitive, so the
                // combo box can't just ask for "the rest" after the refresh button — it needs an
                // exact `.width()` up front, computed from the button's own fixed 24px lane.
                let refresh_lane = widgets::ROW_HEIGHT;
                let combo_width =
                    (ui.available_width() - refresh_lane - ui.spacing().item_spacing.x).max(0.0);
                let device_changed = sidebar::device_panel(
                    ui,
                    &self.devices,
                    &mut self.selected_device,
                    combo_width,
                );
                if sidebar::refresh_button(ui) {
                    self.reload_devices();
                }
                if device_changed && self.worker.is_some() {
                    self.start_worker();
                }
            });
        });

        widgets::labeled_row(ui, "Rotate", |ui| {
            let mut rotate_index = rotation_segment_index(self.rotation);
            widgets::segmented(
                ui,
                "rotate_segmented",
                &mut rotate_index,
                &[
                    widgets::CellContent::Text("0\u{b0}"),
                    widgets::CellContent::Text("90\u{b0}"),
                    widgets::CellContent::Text("180\u{b0}"),
                    widgets::CellContent::Text("270\u{b0}"),
                ],
            );
            let new_rotation = rotation_from_segment_index(rotate_index);
            if new_rotation != self.rotation {
                self.rotation = new_rotation;
                self.push_transform();
            }
        });

        widgets::labeled_row(ui, "Flip", |ui| {
            let mut flip_index = widgets::flip_segment_index(self.flip_h, self.flip_v);
            widgets::segmented(
                ui,
                "flip_segmented",
                &mut flip_index,
                &[
                    widgets::CellContent::Icon(widgets::IconKind::FlipNone),
                    widgets::CellContent::Icon(widgets::IconKind::FlipHorizontal),
                    widgets::CellContent::Icon(widgets::IconKind::FlipVertical),
                    widgets::CellContent::Icon(widgets::IconKind::FlipBoth),
                ],
            );
            let (new_flip_h, new_flip_v) = widgets::flip_from_segment_index(flip_index);
            if (new_flip_h, new_flip_v) != (self.flip_h, self.flip_v) {
                self.flip_h = new_flip_h;
                self.flip_v = new_flip_v;
                self.push_transform();
            }
        });

        widgets::labeled_row(ui, "Crop", |ui| {
            let mut crop_index = if self.crop.is_some() { 1 } else { 0 };
            widgets::segmented(
                ui,
                "crop_segmented",
                &mut crop_index,
                &[widgets::CellContent::Text("off"), widgets::CellContent::Text("edit\u{2026}")],
            );
            match (self.crop.is_some(), crop_index) {
                (false, 1) => match self.input_dims {
                    Some((frame_w, frame_h)) => {
                        self.crop = Some(crate::crop_editor::seed_rect(frame_w, frame_h));
                        self.preview_mode = PreviewMode::CropEdit;
                        self.push_transform();
                    }
                    None => {
                        self.banner =
                            Some("no frame yet — start capture before adding a crop".to_string());
                    }
                },
                (true, 0) => {
                    self.crop = None;
                    self.drag = None;
                    self.preview_mode = PreviewMode::Output;
                    self.push_transform();
                }
                _ => {}
            }
        });
        if let Some(rect) = self.crop {
            // Empty label: still reserves `LABEL_COLUMN_WIDTH` (see `labeled_row`'s doc comment)
            // so this detail row's DragValues align under the CROP control above, not under its
            // label.
            widgets::labeled_row(ui, "", |ui| match sidebar::crop_panel(ui, rect) {
                sidebar::CropAction::None => {}
                sidebar::CropAction::Edited(rect) => {
                    let clamped = match self.input_dims {
                        Some((frame_w, frame_h)) => {
                            crate::crop_editor::clamp_rect(rect, frame_w, frame_h)
                        }
                        None => rect,
                    };
                    self.crop = Some(clamped);
                    self.push_transform();
                }
            });
        }

        widgets::labeled_row(ui, "Scale", |ui| {
            let mut scale_index = sidebar::scale_mode_index(self.scale_input);
            widgets::segmented(
                ui,
                "scale_segmented",
                &mut scale_index,
                &[
                    widgets::CellContent::Text("off"),
                    widgets::CellContent::Text("factor"),
                    widgets::CellContent::Text("W\u{d7}H"),
                ],
            );
            let new_scale_input =
                sidebar::scale_input_for_mode_index(scale_index, self.scale_input);
            if new_scale_input != self.scale_input {
                self.scale_input = new_scale_input;
                self.push_transform();
            }
        });
        // Unlike CROP's detail row (gated on `self.crop.is_some()`), this can't gate on a
        // `Some`/`None` — `ScaleInput` has no such state, `Off` is one of its three ordinary
        // variants — so it gates on `!= Off` directly instead. Without this the row would always
        // reserve a 24px line even while SCALE is "off" and `scale_value_panel` draws nothing into
        // it, wasting vertical space in the compact grid for no visible benefit.
        if self.scale_input != ScaleInput::Off {
            widgets::labeled_row(ui, "", |ui| {
                if sidebar::scale_value_panel(ui, &mut self.scale_input) {
                    self.push_transform();
                }
            });
        }

        widgets::labeled_row(ui, "Server", |ui| {
            let server_name_committed = sidebar::server_name_panel(ui, &mut self.server_name);
            if server_name_committed && self.worker.is_some() {
                self.start_worker();
            }
        });

        let running = self.worker.as_ref().is_some_and(WorkerHandle::is_running);
        let (icon, action_label) = if running {
            (widgets::IconKind::Stop, "STOP PUBLISHING")
        } else {
            (widgets::IconKind::Play, "START PUBLISHING")
        };
        if widgets::action_button(ui, icon, action_label).clicked() {
            if running {
                self.stop_worker();
            } else {
                self.start_worker();
            }
        }
    }

    fn statusbar_ui(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let running = self.worker.as_ref().is_some_and(WorkerHandle::is_running);
            if running {
                ui.colored_label(theme::tokens::ACCENT, "\u{25cf} publishing");
            } else {
                ui.colored_label(theme::tokens::TEXT_SUBTLE, "\u{25cb} stopped");
            }
            ui.separator();

            ui.colored_label(theme::tokens::TEXT_MUTED, &self.server_name);
            ui.separator();

            // `self.output_dims` already mirrors `shared.latest_output`'s dims every frame (see
            // `refresh_preview`) — no separate `SharedState` read is needed here. Hidden entirely
            // (no placeholder text) until the worker has published its first output frame.
            if let Some((width, height)) = self.output_dims {
                ui.label(format!("{width}x{height}"));
                ui.separator();
            }

            let rate = self.fps.rate(Instant::now());
            ui.label(format!("{rate:.0} fps"));
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
        self.poll_menu_actions(ui.ctx());
        self.refresh_preview(ui.ctx());
        self.licenses.show(ui.ctx());

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

        egui::Panel::top("controls").show(ui, |ui| {
            self.controls_ui(ui);
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
    use std::sync::mpsc;

    use gemelli_core::capture::CaptureError;
    use gemelli_core::transform::{CropRect, Flip, Rotation, ScaleSpec, TransformConfig};

    use super::{
        build_transform, drain_stale_errors, flip_from_toggles, refit_crop,
        rotation_from_segment_index, rotation_segment_index,
    };
    use crate::sidebar::ScaleInput;
    use crate::worker::WorkerError;

    #[test]
    fn rotation_segment_index_covers_all_four_states_in_0_90_180_270_order() {
        assert_eq!(rotation_segment_index(Rotation::R0), 0);
        assert_eq!(rotation_segment_index(Rotation::R90), 1);
        assert_eq!(rotation_segment_index(Rotation::R180), 2);
        assert_eq!(rotation_segment_index(Rotation::R270), 3);
    }

    #[test]
    fn rotation_from_segment_index_is_the_exact_inverse() {
        assert_eq!(rotation_from_segment_index(0), Rotation::R0);
        assert_eq!(rotation_from_segment_index(1), Rotation::R90);
        assert_eq!(rotation_from_segment_index(2), Rotation::R180);
        assert_eq!(rotation_from_segment_index(3), Rotation::R270);
    }

    #[test]
    fn rotation_index_round_trips_for_every_state() {
        for rotation in [Rotation::R0, Rotation::R90, Rotation::R180, Rotation::R270] {
            let index = rotation_segment_index(rotation);
            assert_eq!(rotation_from_segment_index(index), rotation, "rotation={rotation:?}");
        }
    }

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

    #[test]
    fn drain_stale_errors_empties_every_queued_message() {
        let (tx, rx) = mpsc::channel();
        for _ in 0..3 {
            tx.send(WorkerError::Capture(CaptureError::FrameRead { reason: "stale".to_string() }))
                .unwrap();
        }

        drain_stale_errors(&rx);

        assert!(rx.try_recv().is_err(), "channel must be empty after draining");
    }

    #[test]
    fn drain_stale_errors_on_an_empty_channel_is_a_no_op() {
        let (_tx, rx) = mpsc::channel::<WorkerError>();

        drain_stale_errors(&rx); // must not block or panic
    }

    #[test]
    fn refit_crop_returns_none_when_the_rect_still_fits() {
        let crop = CropRect { width: 320, height: 240, x: 100, y: 100 };

        assert_eq!(refit_crop(crop, 1920, 1080), None);
    }

    #[test]
    fn refit_crop_returns_the_clamped_rect_when_the_new_frame_is_smaller() {
        // Device switch shrinks the frame from 1920x1080 to 640x480; the old crop
        // (960x540 at 800,400) no longer fits at all.
        let crop = CropRect { width: 960, height: 540, x: 800, y: 400 };

        let refit = refit_crop(crop, 640, 480);

        assert_eq!(refit, Some(crate::crop_editor::clamp_rect(crop, 640, 480)));
        assert_ne!(refit, Some(crop));
    }
}
