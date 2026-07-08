//! Left-panel widgets. Each function borrows only the fields it needs — none of them touch
//! `SharedState` or `WorkerHandle` directly; `app.rs` owns all side effects.

use gemelli_core::capture::DeviceInfo;
use gemelli_core::transform::{Rotation, ScaleSpec};

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

/// Device combo box. Returns `true` if the selection changed this frame.
pub(crate) fn device_panel(
    ui: &mut egui::Ui,
    devices: &[DeviceInfo],
    selected: &mut usize,
) -> bool {
    let previous = *selected;
    egui::ComboBox::from_id_salt("device_select")
        .selected_text(devices.get(*selected).map_or("No devices", |d| d.name.as_str()))
        .show_ui(ui, |ui| {
            for (index, device) in devices.iter().enumerate() {
                ui.selectable_value(selected, index, device.name.as_str());
            }
        });
    *selected != previous
}

pub(crate) fn refresh_button(ui: &mut egui::Ui) -> bool {
    ui.button("Refresh").clicked()
}

/// 2x2 segmented rotation selector: `(0)(90)` / `(180)(270)`. Returns `true` if the selection
/// changed this frame.
pub(crate) fn rotate_panel(ui: &mut egui::Ui, rotation: &mut Rotation) -> bool {
    let previous = *rotation;
    let choices = [
        (Rotation::R0, "0"),
        (Rotation::R90, "90"),
        (Rotation::R180, "180"),
        (Rotation::R270, "270"),
    ];
    egui::Grid::new("rotate_grid").num_columns(2).show(ui, |ui| {
        for (index, (value, label)) in choices.into_iter().enumerate() {
            if ui.selectable_label(*rotation == value, label).clicked() {
                *rotation = value;
            }
            if index % 2 == 1 {
                ui.end_row();
            }
        }
    });
    *rotation != previous
}

/// Independent h/v toggle buttons. Returns `true` if either changed this frame.
pub(crate) fn flip_panel(ui: &mut egui::Ui, flip_h: &mut bool, flip_v: &mut bool) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        changed |= ui.toggle_value(flip_h, "h").changed();
        changed |= ui.toggle_value(flip_v, "v").changed();
    });
    changed
}

/// Mode radio row (Off / Factor / WxH) + the matching value widget. Returns `true` if the mode
/// or the value changed this frame.
pub(crate) fn scale_panel(ui: &mut egui::Ui, scale_input: &mut ScaleInput) -> bool {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum Mode {
        Off,
        Factor,
        Exact,
    }

    fn mode_of(input: ScaleInput) -> Mode {
        match input {
            ScaleInput::Off => Mode::Off,
            ScaleInput::Factor(_) => Mode::Factor,
            ScaleInput::Exact { .. } => Mode::Exact,
        }
    }

    let previous_mode = mode_of(*scale_input);
    let mut mode = previous_mode;
    ui.horizontal(|ui| {
        ui.radio_value(&mut mode, Mode::Off, "Off");
        ui.radio_value(&mut mode, Mode::Factor, "Factor");
        ui.radio_value(&mut mode, Mode::Exact, "WxH");
    });

    *scale_input = match mode {
        Mode::Off => ScaleInput::Off,
        Mode::Factor => match *scale_input {
            ScaleInput::Factor(factor) => ScaleInput::Factor(factor),
            ScaleInput::Off | ScaleInput::Exact { .. } => ScaleInput::Factor(1.0),
        },
        Mode::Exact => match *scale_input {
            ScaleInput::Exact { width, height } => ScaleInput::Exact { width, height },
            ScaleInput::Off | ScaleInput::Factor(_) => {
                ScaleInput::Exact { width: 960, height: 540 }
            }
        },
    };

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

    mode != previous_mode || value_edited
}

/// Server-name text field. Returns `true` only when the field loses focus (not on every
/// keystroke) — restarting the capture thread per keystroke would tear down and recreate the
/// Syphon server dozens of times while the user is still typing.
pub(crate) fn server_name_panel(ui: &mut egui::Ui, server_name: &mut String) -> bool {
    ui.text_edit_singleline(server_name).lost_focus()
}

/// Start/Stop button. `running` is computed by the caller (`WorkerHandle::is_running`), since
/// this module never holds a `WorkerHandle`. Returns `true` if clicked.
pub(crate) fn transport_button(ui: &mut egui::Ui, running: bool) -> bool {
    let label = if running { "Stop" } else { "Start" };
    ui.button(label).clicked()
}

#[cfg(test)]
mod tests {
    use gemelli_core::transform::ScaleSpec;

    use super::{ScaleInput, scale_from_input};

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
}
