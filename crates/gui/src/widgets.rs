//! Shared custom-painted widgets for the portrait controls layout: a brow label, a full-width
//! segmented control, and a full-width action button. All three paint directly with
//! `ui.painter()` (rather than composing `egui::Button`/`egui::SelectableLabel`) so the fill,
//! text color, and hover state can follow the `theme::tokens` palette exactly instead of egui's
//! built-in widget visuals.

use crate::theme;

/// Converts a small non-negative count into `f32` without an `as` cast: `u16` is the largest
/// integer type that converts to `f32` losslessly (`f32`'s 24-bit mantissa can't represent every
/// `u32`), and no segmented control here ever has anywhere near `u16::MAX` cells, so the
/// `unwrap_or` clamp never actually triggers.
#[cfg_attr(not(test), allow(dead_code))]
fn count_to_f32(count: usize) -> f32 {
    f32::from(u16::try_from(count).unwrap_or(u16::MAX))
}

/// Splits `total_width` into `count` equal-ish cells, left to right. Every cell gets
/// `floor(total_width / count)` except the last, which absorbs whatever remains — so the sum of
/// cell widths always equals `total_width` exactly, with no gap or overhang at the right edge.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn cell_bounds(total_width: f32, count: usize) -> Vec<(f32, f32)> {
    if count == 0 {
        return Vec::new();
    }

    let base_width = (total_width / count_to_f32(count)).floor();
    let mut bounds = Vec::with_capacity(count);
    let mut used = 0.0_f32;
    for index in 0..count {
        let is_last = index + 1 == count;
        let width = if is_last { total_width - used } else { base_width };
        bounds.push((used, used + width));
        used += width;
    }
    bounds
}

/// Maps a click's local x-offset (relative to the segmented control's left edge) to a cell
/// index. Offsets at or past a cell boundary belong to the *next* cell (so a boundary exactly on
/// a click never picks the wrong side of it — see the boundary test below); offsets before the
/// first cell or past the last cell clamp to the nearest end instead of panicking or wrapping.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn cell_at(x_offset: f32, total_width: f32, count: usize) -> usize {
    if count == 0 {
        return 0;
    }
    if x_offset <= 0.0 {
        return 0;
    }

    for (index, (_, end)) in cell_bounds(total_width, count).into_iter().enumerate() {
        if x_offset < end {
            return index;
        }
    }
    count - 1
}

/// (h, v) toggle pair -> segmented-control index, in `none / H / V / H+V` order (matches the
/// design doc's cell order). Paired with `flip_from_segment_index` below for the round trip the
/// FLIP control needs every frame: read the index the user clicked, turn it back into the (h, v)
/// pair `build_transform` already expects.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn flip_segment_index(h: bool, v: bool) -> usize {
    match (h, v) {
        (false, false) => 0,
        (true, false) => 1,
        (false, true) => 2,
        (true, true) => 3,
    }
}

/// Inverse of `flip_segment_index`. Any index of 3 or greater (there is no such cell, but
/// `segmented`'s `selected` is a plain `usize` with no compile-time bound) clamps to H+V rather
/// than panicking.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn flip_from_segment_index(index: usize) -> (bool, bool) {
    match index {
        0 => (false, false),
        1 => (true, false),
        2 => (false, true),
        _ => (true, true),
    }
}

/// Small uppercase caption above a control group ("DEVICE", "ROTATE", …). egui has no
/// letter-spacing control, so the "brow label" look from the mockup is approximated with
/// uppercasing + a small size + `TEXT_SUBTLE` instead. Uppercasing happens inside this function —
/// callers pass normal-case text ("Device") and don't need to know the visual convention.
#[allow(dead_code)]
pub(crate) fn group_label(ui: &mut egui::Ui, text: &str) {
    ui.label(egui::RichText::new(text.to_uppercase()).size(11.0).color(theme::tokens::TEXT_SUBTLE));
}

/// Full-width segmented control: `count = labels.len()` equal-ish cells (see `cell_bounds`), one
/// shared 2px `BORDER` outline around the whole control instead of a border per cell, and 2px
/// vertical separators between cells. Selected cell: `ACCENT` fill + `BG_BASE` text (inverted, to
/// match `theme::apply_theme`'s selection scheme). Unselected: `BG_PANEL` fill + `TEXT_MUTED`
/// text, or `BG_MUTED` fill when hovered — `ACCENT_HOVER` is reserved for `action_button` alone,
/// so segmented-cell hover uses a neutral fill instead of the accent hover token.
///
/// `id_salt` is threaded into an explicit `egui::Id` (rather than relying on the auto-id egui
/// would otherwise assign this call site) so this control's identity survives if the surrounding
/// UI's widget order shifts frame-to-frame — e.g. the CROP numeric row below appearing/
/// disappearing changes every later auto-id in `controls_ui`, but not an explicitly salted one.
#[allow(dead_code)]
pub(crate) fn segmented(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    selected: &mut usize,
    labels: &[&str],
) -> egui::Response {
    let count = labels.len();
    let width = ui.available_width();
    let height = 32.0;
    let (_, rect) = ui.allocate_space(egui::vec2(width, height));
    let id = egui::Id::new(id_salt);
    let response = ui.interact(rect, id, egui::Sense::click());

    if response.clicked()
        && let Some(pointer) = response.interact_pointer_pos()
    {
        *selected = cell_at(pointer.x - rect.left(), width, count);
    }

    let hovered_cell =
        response.hover_pos().map(|pointer| cell_at(pointer.x - rect.left(), width, count));

    let painter = ui.painter();
    let bounds = cell_bounds(width, count);
    for (index, ((start, end), label)) in
        bounds.iter().copied().zip(labels.iter().copied()).enumerate()
    {
        let cell_rect = egui::Rect::from_min_max(
            rect.left_top() + egui::vec2(start, 0.0),
            egui::pos2(rect.left() + end, rect.bottom()),
        );
        let is_selected = index == *selected;
        let is_hovered = !is_selected && hovered_cell == Some(index);
        let fill = if is_selected {
            theme::tokens::ACCENT
        } else if is_hovered {
            theme::tokens::BG_MUTED
        } else {
            theme::tokens::BG_PANEL
        };
        let text_color =
            if is_selected { theme::tokens::BG_BASE } else { theme::tokens::TEXT_MUTED };

        painter.rect_filled(cell_rect, 0.0, fill);
        painter.text(
            cell_rect.center(),
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(13.0),
            text_color,
        );
    }

    for (start, _) in bounds.iter().skip(1) {
        let x = rect.left() + start;
        painter.line_segment(
            [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
            egui::Stroke::new(2.0, theme::tokens::BORDER),
        );
    }

    painter.rect_stroke(
        rect,
        0.0,
        egui::Stroke::new(2.0, theme::tokens::BORDER),
        egui::StrokeKind::Inside,
    );

    response
}

/// Full-width x 44px call-to-action button ("START PUBLISHING" / "STOP PUBLISHING"). Solid
/// `ACCENT` fill, swapping to `ACCENT_HOVER` while the pointer is over it, with `BG_BASE` text —
/// the same inverted-selection color pairing `segmented`'s selected cell uses. Painted directly
/// (not via `egui::Button`) so the hover fill can use `ACCENT_HOVER` specifically rather than
/// egui's ambient `visuals.widgets.hovered` styling.
#[allow(dead_code)]
pub(crate) fn action_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    let width = ui.available_width();
    let height = 44.0;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());

    let fill = if response.hovered() { theme::tokens::ACCENT_HOVER } else { theme::tokens::ACCENT };
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, fill);
    painter.text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        label,
        egui::FontId::proportional(15.0),
        theme::tokens::BG_BASE,
    );

    response
}

#[cfg(test)]
mod tests {
    use super::{cell_at, cell_bounds, flip_from_segment_index, flip_segment_index};

    #[test]
    fn cell_bounds_splits_evenly_when_width_divides_by_count() {
        assert_eq!(cell_bounds(90.0, 3), vec![(0.0, 30.0), (30.0, 60.0), (60.0, 90.0)]);
    }

    #[test]
    fn cell_bounds_gives_the_remainder_to_the_last_cell() {
        assert_eq!(cell_bounds(100.0, 3), vec![(0.0, 33.0), (33.0, 66.0), (66.0, 100.0)]);
    }

    #[test]
    fn cell_bounds_with_zero_cells_is_empty() {
        assert_eq!(cell_bounds(200.0, 0), Vec::new());
    }

    #[test]
    fn cell_bounds_with_one_cell_is_the_full_width() {
        assert_eq!(cell_bounds(50.0, 1), vec![(0.0, 50.0)]);
    }

    #[test]
    fn cell_at_clamps_negative_offset_to_the_first_cell() {
        assert_eq!(cell_at(-10.0, 90.0, 3), 0);
    }

    #[test]
    fn cell_at_finds_the_containing_cell() {
        assert_eq!(cell_at(45.0, 90.0, 3), 1);
    }

    #[test]
    fn cell_at_clamps_overflow_to_the_last_cell() {
        assert_eq!(cell_at(1000.0, 90.0, 3), 2);
    }

    #[test]
    fn cell_at_on_a_boundary_belongs_to_the_next_cell() {
        // 30.0 is simultaneously cell 0's end and cell 1's start; cell_at must pick one
        // consistently rather than double-counting or leaving a dead zone.
        assert_eq!(cell_at(30.0, 90.0, 3), 1);
    }

    #[test]
    fn cell_at_with_zero_cells_is_zero() {
        assert_eq!(cell_at(10.0, 100.0, 0), 0);
    }

    #[test]
    fn flip_segment_index_covers_all_four_states_in_none_h_v_hv_order() {
        assert_eq!(flip_segment_index(false, false), 0);
        assert_eq!(flip_segment_index(true, false), 1);
        assert_eq!(flip_segment_index(false, true), 2);
        assert_eq!(flip_segment_index(true, true), 3);
    }

    #[test]
    fn flip_from_segment_index_is_the_exact_inverse() {
        assert_eq!(flip_from_segment_index(0), (false, false));
        assert_eq!(flip_from_segment_index(1), (true, false));
        assert_eq!(flip_from_segment_index(2), (false, true));
        assert_eq!(flip_from_segment_index(3), (true, true));
    }

    #[test]
    fn flip_index_round_trips_for_every_state() {
        for (h, v) in [(false, false), (true, false), (false, true), (true, true)] {
            let index = flip_segment_index(h, v);
            assert_eq!(flip_from_segment_index(index), (h, v), "h={h} v={v} index={index}");
        }
    }
}
