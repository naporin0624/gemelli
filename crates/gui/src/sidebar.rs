//! Left-panel widgets. Each function borrows only the fields it needs — none of them touch
//! `SharedState` or `WorkerHandle` directly; `app.rs` owns all side effects.

use gemelli_core::capture::DeviceInfo;
use gemelli_core::transform::{CropRect, ScaleSpec};

const SCALE_FACTOR_MIN: f64 = 0.1;
const SCALE_FACTOR_MAX: f64 = 2.0;

/// Scale widget's own input shape — mutually exclusive Off / Factor / Exact, mirroring the three
/// controls the sidebar shows. Maps down to `Option<ScaleSpec>` via `scale_from_input`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) enum ScaleInput {
    #[default]
    Off,
    Factor(f64),
    Exact {
        width: u32,
        height: u32,
    },
}

/// Pure mapping from the widget's input shape to core's `ScaleSpec`. Zero WxH clamps to 1x1
/// rather than collapsing to `Off`: a `DragValue` can pass through `0` transiently while the
/// user drags it up from empty, and treating that as "turn scaling off" would silently discard
/// the in-progress edit and flip the mode radio buttons out from under them. Clamping keeps the
/// widget's mode selection authoritative; only the numeric value is defended.
pub(crate) fn scale_from_input(input: ScaleInput) -> Option<ScaleSpec> {
    match input {
        ScaleInput::Off => None,
        ScaleInput::Factor(factor) => {
            Some(ScaleSpec::Factor(factor.clamp(SCALE_FACTOR_MIN, SCALE_FACTOR_MAX)))
        }
        ScaleInput::Exact { width, height } => {
            Some(ScaleSpec::Exact { width: width.max(1), height: height.max(1) })
        }
    }
}

/// Device combo box, sized to `width` so the caller can reserve a fixed lane for the refresh
/// button beside it. Returns `true` if the selection changed this frame.
pub(crate) fn device_panel(
    ui: &mut egui::Ui,
    devices: &[DeviceInfo],
    selected: &mut usize,
    width: f32,
) -> bool {
    let previous = *selected;
    egui::ComboBox::from_id_salt("device_select")
        .width(width)
        .selected_text(devices.get(*selected).map_or("No devices", |d| d.name.as_str()))
        .show_ui(ui, |ui| {
            for (index, device) in devices.iter().enumerate() {
                ui.selectable_value(selected, index, device.name.as_str());
            }
        });
    *selected != previous
}

pub(crate) fn refresh_button(ui: &mut egui::Ui) -> bool {
    ui.button("\u{27f3}").clicked()
}

/// Scale widget's mode as a segmented-control index, in the `off / factor / W×H` cell order the
/// design doc specifies.
pub(crate) fn scale_mode_index(input: ScaleInput) -> usize {
    match input {
        ScaleInput::Off => 0,
        ScaleInput::Factor(_) => 1,
        ScaleInput::Exact { .. } => 2,
    }
}

/// Inverse of `scale_mode_index`, applied against the *previous* `ScaleInput` rather than
/// producing a bare default: re-selecting the mode already active is a no-op (its numeric value
/// is preserved), and only switching mode away-and-back resets the value, so a user nudging the
/// segmented control back and forth doesn't lose an in-progress factor/WxH edit.
pub(crate) fn scale_input_for_mode_index(index: usize, previous: ScaleInput) -> ScaleInput {
    match index {
        0 => ScaleInput::Off,
        1 => match previous {
            ScaleInput::Factor(factor) => ScaleInput::Factor(factor),
            ScaleInput::Off | ScaleInput::Exact { .. } => ScaleInput::Factor(1.0),
        },
        _ => match previous {
            ScaleInput::Exact { width, height } => ScaleInput::Exact { width, height },
            ScaleInput::Off | ScaleInput::Factor(_) => {
                ScaleInput::Exact { width: 960, height: 540 }
            }
        },
    }
}

/// The scale value widget only (slider for Factor, W/H drag fields for Exact, nothing for Off) —
/// the mode itself is chosen by the SCALE segmented control in `app.rs`, not here. Returns `true`
/// if the value changed this frame.
pub(crate) fn scale_value_panel(ui: &mut egui::Ui, scale_input: &mut ScaleInput) -> bool {
    let mut value_edited = false;
    match scale_input {
        ScaleInput::Off => {}
        ScaleInput::Factor(factor) => {
            value_edited |= ui.add(egui::Slider::new(factor, 0.1..=2.0)).changed();
        }
        ScaleInput::Exact { width, height } => {
            ui.horizontal(|ui| {
                value_edited |=
                    ui.add(egui::DragValue::new(width).range(1..=7680).prefix("w:")).changed();
                value_edited |=
                    ui.add(egui::DragValue::new(height).range(1..=4320).prefix("h:")).changed();
            });
        }
    }
    value_edited
}

/// Server-name text field, full width. Returns `true` only when the field loses focus (not on
/// every keystroke) — restarting the capture thread per keystroke would tear down and recreate
/// the Syphon server dozens of times while the user is still typing.
pub(crate) fn server_name_panel(ui: &mut egui::Ui, server_name: &mut String) -> bool {
    ui.add(egui::TextEdit::singleline(server_name).desired_width(f32::INFINITY)).lost_focus()
}

/// What the crop numeric row did this frame. Exhaustively matched by `app.rs` — no `_` arm, so a
/// new action here forces the call site to decide what it means instead of silently doing
/// nothing. Creating/clearing the crop rect itself is decided by `app.rs` from the CROP
/// segmented control directly (see `controls_ui`), not by this function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CropAction {
    None,
    Edited(CropRect),
}

/// Crop numeric row: a W/H/X/Y `DragValue` grid for `rect`. Only rendered by `app.rs` while the
/// CROP segmented control is on "edit…" — the rect always exists by the time this is called. The
/// numeric fields and the on-screen drag rect (`preview_ui`'s crop overlay) are kept in sync
/// purely by both reading `self.crop` fresh every frame in `app.rs` — there is no separate
/// "pending edit" state to desync.
pub(crate) fn crop_panel(ui: &mut egui::Ui, mut rect: CropRect) -> CropAction {
    let mut edited = false;
    ui.horizontal(|ui| {
        edited |= ui.add(egui::DragValue::new(&mut rect.width).prefix("w:")).changed();
        edited |= ui.add(egui::DragValue::new(&mut rect.height).prefix("h:")).changed();
    });
    ui.horizontal(|ui| {
        edited |= ui.add(egui::DragValue::new(&mut rect.x).prefix("x:")).changed();
        edited |= ui.add(egui::DragValue::new(&mut rect.y).prefix("y:")).changed();
    });

    if edited { CropAction::Edited(rect) } else { CropAction::None }
}

#[cfg(test)]
mod tests {
    use gemelli_core::transform::ScaleSpec;

    use super::{ScaleInput, scale_from_input, scale_input_for_mode_index, scale_mode_index};

    #[test]
    fn scale_from_input_off_is_none() {
        assert_eq!(scale_from_input(ScaleInput::Off), None);
    }

    #[test]
    fn scale_from_input_factor_within_range_passes_through() {
        assert_eq!(scale_from_input(ScaleInput::Factor(0.5)), Some(ScaleSpec::Factor(0.5)));
    }

    #[test]
    fn scale_from_input_factor_clamps_below_minimum() {
        assert_eq!(scale_from_input(ScaleInput::Factor(0.0)), Some(ScaleSpec::Factor(0.1)));
    }

    #[test]
    fn scale_from_input_factor_clamps_above_maximum() {
        assert_eq!(scale_from_input(ScaleInput::Factor(5.0)), Some(ScaleSpec::Factor(2.0)));
    }

    #[test]
    fn scale_from_input_exact_zero_dims_clamp_to_one() {
        assert_eq!(
            scale_from_input(ScaleInput::Exact { width: 0, height: 0 }),
            Some(ScaleSpec::Exact { width: 1, height: 1 })
        );
    }

    #[test]
    fn scale_from_input_exact_normal_dims_pass_through() {
        assert_eq!(
            scale_from_input(ScaleInput::Exact { width: 960, height: 540 }),
            Some(ScaleSpec::Exact { width: 960, height: 540 })
        );
    }

    #[test]
    fn scale_mode_index_covers_all_three_states_in_off_factor_exact_order() {
        assert_eq!(scale_mode_index(ScaleInput::Off), 0);
        assert_eq!(scale_mode_index(ScaleInput::Factor(0.5)), 1);
        assert_eq!(scale_mode_index(ScaleInput::Exact { width: 10, height: 20 }), 2);
    }

    #[test]
    fn scale_input_for_mode_index_switching_to_off_discards_the_value() {
        assert_eq!(scale_input_for_mode_index(0, ScaleInput::Factor(0.5)), ScaleInput::Off);
    }

    #[test]
    fn scale_input_for_mode_index_reselecting_factor_preserves_its_value() {
        assert_eq!(
            scale_input_for_mode_index(1, ScaleInput::Factor(0.75)),
            ScaleInput::Factor(0.75)
        );
    }

    #[test]
    fn scale_input_for_mode_index_switching_to_factor_from_elsewhere_defaults_to_one() {
        assert_eq!(scale_input_for_mode_index(1, ScaleInput::Off), ScaleInput::Factor(1.0));
    }

    #[test]
    fn scale_input_for_mode_index_reselecting_exact_preserves_its_dims() {
        assert_eq!(
            scale_input_for_mode_index(2, ScaleInput::Exact { width: 640, height: 480 }),
            ScaleInput::Exact { width: 640, height: 480 }
        );
    }

    #[test]
    fn scale_input_for_mode_index_switching_to_exact_from_elsewhere_defaults_to_960x540() {
        assert_eq!(
            scale_input_for_mode_index(2, ScaleInput::Off),
            ScaleInput::Exact { width: 960, height: 540 }
        );
    }
}
