//! Shared custom-painted widgets for the compact label-left controls grid: a brow label, a
//! full-width segmented control (text or painter-drawn icon cells), a full-width action button,
//! and the vector icons all three lean on. Everything paints directly with `ui.painter()` (rather
//! than composing `egui::Button`/`egui::SelectableLabel`/font glyphs/SVG assets) so fill, text
//! color, and icon geometry all follow `theme::tokens` and this module's own shapes exactly —
//! icons are painter primitives (lines, filled triangles, arcs) specifically because font glyphs
//! for FLIP's mirror icons don't exist in any font this app loads (no candidate codepoint like
//! U+21CB renders in LINE Seed JP or any of egui's built-in fonts — real glyph coverage was
//! checked directly against those font files), and SVG assets were rejected — `resvg`/`usvg` are
//! MPL-2.0, which this repo's `deny.toml` permissive-only license policy forbids.

use crate::theme;

/// Row height for the compact controls grid (segmented cells, combo boxes, buttons): Cannelloni's
/// `targetMin` (24px), reserved for interactive rows generally — `action_button` alone uses the
/// taller `ACTION_BUTTON_HEIGHT` below, since a full-width call-to-action stays legible at a
/// slightly larger size even in the compact layout.
pub(crate) const ROW_HEIGHT: f32 = 24.0;

/// Height of the full-width action button ("START PUBLISHING" / "STOP PUBLISHING"). Distinct from
/// `ROW_HEIGHT` on purpose — the CTA stays visually the most prominent row in the grid even though
/// every input row above it shrank to 24px.
pub(crate) const ACTION_BUTTON_HEIGHT: f32 = 28.0;

/// Fixed width of `labeled_row`'s left label column. Measured as the widest group-label caption
/// actually used — "ROTATE" — rendered through the exact production font stack (LINE Seed JP
/// installed ahead of egui's built-ins, per `fonts::install_fonts`) at this module's 11px
/// uppercase label size: 39.22px. Frozen a few px above that measurement, not exactly at it, so a
/// future label rename/addition of similar length doesn't force this constant to be revisited
/// every time.
pub(crate) const LABEL_COLUMN_WIDTH: f32 = 44.0;

/// Converts a small non-negative count into `f32` without an `as` cast: `u16` is the largest
/// integer type that converts to `f32` losslessly (`f32`'s 24-bit mantissa can't represent every
/// `u32`), and nothing counted here (segmented cells, arc polyline segments) ever comes close to
/// `u16::MAX`, so the `unwrap_or` clamp never actually triggers.
fn count_to_f32(count: usize) -> f32 {
    f32::from(u16::try_from(count).unwrap_or(u16::MAX))
}

/// Splits `total_width` into `count` equal-ish cells, left to right. Every cell gets
/// `floor(total_width / count)` except the last, which absorbs whatever remains — so the sum of
/// cell widths always equals `total_width` exactly, with no gap or overhang at the right edge.
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
pub(crate) fn flip_from_segment_index(index: usize) -> (bool, bool) {
    match index {
        0 => (false, false),
        1 => (true, false),
        2 => (false, true),
        _ => (true, true),
    }
}

/// One `segmented` cell's visual content: a short text label (ROTATE/CROP/SCALE) or a
/// painter-drawn icon (FLIP's four cells — see the module doc for why icons are vector shapes,
/// not glyphs). FLIP's own cells never mix the two variants; a segmented control this app builds
/// is either all-`Text` or all-`Icon`, for one consistent visual language per control, but
/// `segmented` itself doesn't enforce that — it just renders whatever `Copy` enum each cell
/// carries.
#[derive(Debug, Clone, Copy)]
pub(crate) enum CellContent<'a> {
    Text(&'a str),
    Icon(IconKind),
}

/// Which vector icon `paint_icon` draws. One variant per icon this app needs: FLIP's four states,
/// the DEVICE row's refresh button, and the action button's play/stop prefix — every painter-drawn
/// icon in the app shares this one dispatch point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IconKind {
    FlipNone,
    FlipHorizontal,
    FlipVertical,
    FlipBoth,
    Refresh,
    Play,
    Stop,
}

/// Which way `triangle_points` points its triangle's tip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TriangleDirection {
    Left,
    Right,
    Up,
    Down,
}

/// Solid triangle inscribed in `rect`, tip pointing `direction`, base flush with the opposite
/// edge. Pure geometry (no painting) so symmetry and containment are unit-testable without a
/// `Painter`. Used both for `mirror_triangle_pair`'s FLIP icons and standalone for the action
/// button's `Play` icon (`Right`).
pub(crate) fn triangle_points(rect: egui::Rect, direction: TriangleDirection) -> [egui::Pos2; 3] {
    match direction {
        TriangleDirection::Right => {
            [rect.left_top(), rect.left_bottom(), egui::pos2(rect.right(), rect.center().y)]
        }
        TriangleDirection::Left => {
            [rect.right_top(), rect.right_bottom(), egui::pos2(rect.left(), rect.center().y)]
        }
        TriangleDirection::Down => {
            [rect.left_top(), rect.right_top(), egui::pos2(rect.center().x, rect.bottom())]
        }
        TriangleDirection::Up => {
            [rect.left_bottom(), rect.right_bottom(), egui::pos2(rect.center().x, rect.top())]
        }
    }
}

/// Which pair of `rect`'s halves `mirror_triangle_pair` splits: `Horizontal` divides left/right
/// (for the FLIP-H icon — divider is a vertical line, triangles point left and right, away from
/// each other); `Vertical` divides top/bottom (for FLIP-V — divider is horizontal, triangles point
/// up and down).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MirrorAxis {
    Horizontal,
    Vertical,
}

/// Point geometry for one FLIP icon's mirrored-triangle-pair: two triangles pointing away from
/// each other across a center divider (the standard "mirror" glyph convention — each half
/// reflects into the other), plus the divider itself as a line segment. Pure geometry (no
/// painting) so the pair's symmetry and the divider's placement are unit-testable.
pub(crate) fn mirror_triangle_pair(
    rect: egui::Rect,
    axis: MirrorAxis,
) -> ([egui::Pos2; 3], [egui::Pos2; 3], [egui::Pos2; 2]) {
    match axis {
        MirrorAxis::Horizontal => {
            let left_half = egui::Rect::from_min_max(
                rect.left_top(),
                egui::pos2(rect.center().x, rect.bottom()),
            );
            let right_half = egui::Rect::from_min_max(
                egui::pos2(rect.center().x, rect.top()),
                rect.right_bottom(),
            );
            let left_triangle = triangle_points(left_half, TriangleDirection::Left);
            let right_triangle = triangle_points(right_half, TriangleDirection::Right);
            let divider = [
                egui::pos2(rect.center().x, rect.top()),
                egui::pos2(rect.center().x, rect.bottom()),
            ];
            (left_triangle, right_triangle, divider)
        }
        MirrorAxis::Vertical => {
            let top_half = egui::Rect::from_min_max(
                rect.left_top(),
                egui::pos2(rect.right(), rect.center().y),
            );
            let bottom_half = egui::Rect::from_min_max(
                egui::pos2(rect.left(), rect.center().y),
                rect.right_bottom(),
            );
            let top_triangle = triangle_points(top_half, TriangleDirection::Up);
            let bottom_triangle = triangle_points(bottom_half, TriangleDirection::Down);
            let divider = [
                egui::pos2(rect.left(), rect.center().y),
                egui::pos2(rect.right(), rect.center().y),
            ];
            (top_triangle, bottom_triangle, divider)
        }
    }
}

/// Centers a `size x size` square icon bounding box inside `cell_rect`. Every painter-drawn icon
/// here (triangles, mirror pairs, the refresh arc) is computed against a square rect regardless of
/// the cell's own (usually wider-than-tall) shape, so this is the one place that reconciles the
/// two — pure geometry, unit-tested for centering.
pub(crate) fn icon_rect(cell_rect: egui::Rect, size: f32) -> egui::Rect {
    egui::Rect::from_center_size(cell_rect.center(), egui::vec2(size, size))
}

/// Thin horizontal bar centered in `rect`, used for the FLIP-none icon (the "no transform" state
/// reads as a plain dash — deliberately the least visually busy of the four FLIP icons). Pure
/// geometry so its centering and thickness ratio are unit-tested.
pub(crate) fn dash_rect(rect: egui::Rect) -> egui::Rect {
    let thickness = (rect.height() * 0.18).max(2.0);
    egui::Rect::from_center_size(rect.center(), egui::vec2(rect.width(), thickness))
}

/// Number of straight segments approximating the refresh icon's circular arc as a polyline —
/// `Painter` has no native arc primitive (see the module doc's API survey), so the arc is drawn as
/// a many-sided polyline instead.
const REFRESH_ARC_SEGMENTS: usize = 16;

/// The refresh arc's angular span: leaves a deliberate gap (360° - this) at the bottom so the icon
/// reads as an open circular arrow — like a partially-drawn circle with an arrowhead at the open
/// end — rather than a closed ring, which would look like a plain "o".
const REFRESH_ARC_SPAN_TURNS: f32 = 290.0 / 360.0;

/// Where the refresh arc starts, in radians, measured from 3 o'clock going clockwise (egui's
/// screen-space convention: angle 0 is +x, positive angle rotates toward +y i.e. downward on
/// screen). `-FRAC_PI_2` is straight up (12 o'clock) — an arbitrary but fixed start so the arc and
/// its arrowhead have one stable orientation.
const REFRESH_ARC_START: f32 = -std::f32::consts::FRAC_PI_2;

fn refresh_arc_radius(rect: egui::Rect) -> f32 {
    rect.width().min(rect.height()) * 0.4
}

/// Points tracing the refresh icon's open circular arc, centered in `rect`. Pure geometry (no
/// painting): every point sits at `refresh_arc_radius(rect)` from `rect.center()`, and the arc
/// deliberately does not close into a full circle (see `REFRESH_ARC_SPAN_TURNS`).
pub(crate) fn refresh_arc_points(rect: egui::Rect) -> Vec<egui::Pos2> {
    let center = rect.center();
    let radius = refresh_arc_radius(rect);
    let sweep = std::f32::consts::TAU * REFRESH_ARC_SPAN_TURNS;
    (0..=REFRESH_ARC_SEGMENTS)
        .map(|index| {
            let t = count_to_f32(index) / count_to_f32(REFRESH_ARC_SEGMENTS);
            let angle = REFRESH_ARC_START + sweep * t;
            egui::pos2(center.x + radius * angle.cos(), center.y + radius * angle.sin())
        })
        .collect()
}

/// Small filled arrowhead at the open end of `refresh_arc_points`, tangent to the arc's direction
/// of travel there (clockwise) rather than a triangle in some unrelated orientation, so it reads
/// as "the arrow this arc is spinning toward". Pure geometry: the tip coincides exactly with
/// `refresh_arc_points(rect)`'s last point (unit-tested).
pub(crate) fn refresh_arrowhead_points(rect: egui::Rect) -> [egui::Pos2; 3] {
    let center = rect.center();
    let radius = refresh_arc_radius(rect);
    let end_angle = REFRESH_ARC_START + std::f32::consts::TAU * REFRESH_ARC_SPAN_TURNS;
    let tip = egui::pos2(center.x + radius * end_angle.cos(), center.y + radius * end_angle.sin());
    // Unit tangent in the clockwise direction of travel, and unit outward-radial normal, at the
    // arc's end angle — standard derivatives of `(radius*cos, radius*sin)` with respect to angle.
    let tangent = egui::vec2(-end_angle.sin(), end_angle.cos());
    let normal = egui::vec2(end_angle.cos(), end_angle.sin());
    let size = radius * 0.7;
    let base_center = tip - tangent * size;
    [tip, base_center + normal * (size * 0.6), base_center - normal * (size * 0.6)]
}

fn paint_flip_none(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    painter.rect_filled(dash_rect(rect), 0.0, color);
}

fn paint_mirror_pair(
    painter: &egui::Painter,
    rect: egui::Rect,
    color: egui::Color32,
    axis: MirrorAxis,
) {
    let (first, second, divider) = mirror_triangle_pair(rect, axis);
    painter.add(egui::epaint::PathShape::convex_polygon(first.to_vec(), color, egui::Stroke::NONE));
    painter.add(egui::epaint::PathShape::convex_polygon(
        second.to_vec(),
        color,
        egui::Stroke::NONE,
    ));
    painter.line_segment(divider, egui::Stroke::new(2.0, color));
}

fn paint_flip_both(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    // Both mirror pairs overlaid at once would visually collide at full size, so each half draws
    // into a shrunk-and-offset sub-rect instead — same convention a compound "H+V" icon needs
    // regardless of which two base icons it's combining.
    let shrunk = egui::Rect::from_center_size(rect.center(), rect.size() * 0.68);
    paint_mirror_pair(painter, shrunk, color, MirrorAxis::Horizontal);
    paint_mirror_pair(painter, shrunk, color, MirrorAxis::Vertical);
}

fn paint_refresh(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    painter.add(egui::epaint::PathShape::line(
        refresh_arc_points(rect),
        egui::Stroke::new(2.0, color),
    ));
    let arrowhead = refresh_arrowhead_points(rect);
    painter.add(egui::epaint::PathShape::convex_polygon(
        arrowhead.to_vec(),
        color,
        egui::Stroke::NONE,
    ));
}

fn paint_play(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    let points = triangle_points(rect, TriangleDirection::Right);
    painter.add(egui::epaint::PathShape::convex_polygon(
        points.to_vec(),
        color,
        egui::Stroke::NONE,
    ));
}

fn paint_stop(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32) {
    painter.rect_filled(rect, 0.0, color);
}

/// Dispatches to the one painter-drawn icon `kind` names. Exhaustive match — no `_` arm — so a new
/// `IconKind` variant forces this to be revisited instead of silently drawing nothing.
pub(crate) fn paint_icon(
    painter: &egui::Painter,
    rect: egui::Rect,
    color: egui::Color32,
    kind: IconKind,
) {
    match kind {
        IconKind::FlipNone => paint_flip_none(painter, rect, color),
        IconKind::FlipHorizontal => paint_mirror_pair(painter, rect, color, MirrorAxis::Horizontal),
        IconKind::FlipVertical => paint_mirror_pair(painter, rect, color, MirrorAxis::Vertical),
        IconKind::FlipBoth => paint_flip_both(painter, rect, color),
        IconKind::Refresh => paint_refresh(painter, rect, color),
        IconKind::Play => paint_play(painter, rect, color),
        IconKind::Stop => paint_stop(painter, rect, color),
    }
}

/// Square icon-only button (the DEVICE row's 24px refresh control): same `BG_PANEL`/`BG_MUTED`
/// fill and `BORDER` outline convention as `segmented`'s unselected cells, so it reads as part of
/// the same control family instead of a plain default-themed `egui::Button`.
pub(crate) fn icon_button(ui: &mut egui::Ui, icon: IconKind, size: f32) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::click());
    let fill = if response.hovered() { theme::tokens::BG_MUTED } else { theme::tokens::BG_PANEL };
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, fill);
    painter.rect_stroke(
        rect,
        0.0,
        egui::Stroke::new(2.0, theme::tokens::BORDER),
        egui::StrokeKind::Inside,
    );
    paint_icon(painter, icon_rect(rect, size * 0.55), theme::tokens::TEXT_MUTED, icon);
    response
}

/// Draws one control-group row: a fixed-width uppercase `TEXT_SUBTLE` "brow label" (uppercase /
/// 11px / `TEXT_SUBTLE`) in `LABEL_COLUMN_WIDTH` (right-aligned, so it sits close to its control
/// per Gestalt proximity, rather than flush to the panel's left edge regardless of the label's
/// own length), then `add_control` in the remaining width. An empty `label` still reserves the
/// column (paints nothing) — `controls_ui` uses that to indent CROP/SCALE's numeric detail rows
/// so they align under their control instead of under the label.
///
/// The control area is a nested `ui.vertical`, not the same horizontal line as the label: a
/// control that itself needs multiple internal rows (CROP's W/H/X/Y `DragValue` grid spans two)
/// would otherwise be laid out as two side-by-side items in the outer horizontal flow instead of
/// stacked — nesting a vertical here is what lets `add_control` build its own multi-row layout.
pub(crate) fn labeled_row<R>(
    ui: &mut egui::Ui,
    label: &str,
    add_control: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    ui.horizontal(|ui| {
        let (_, label_rect) = ui.allocate_space(egui::vec2(LABEL_COLUMN_WIDTH, ROW_HEIGHT));
        if !label.is_empty() {
            ui.painter().text(
                egui::pos2(label_rect.right(), label_rect.center().y),
                egui::Align2::RIGHT_CENTER,
                label.to_uppercase(),
                egui::FontId::proportional(11.0),
                theme::tokens::TEXT_SUBTLE,
            );
        }
        ui.vertical(|ui| add_control(ui)).inner
    })
    .inner
}

/// Full-width segmented control: `count = cells.len()` equal-ish cells (see `cell_bounds`), one
/// shared 2px `BORDER` outline around the whole control instead of a border per cell, and 2px
/// vertical separators between cells. Selected cell: `ACCENT` fill + `BG_BASE` content (inverted,
/// to match `theme::apply_theme`'s selection scheme). Unselected: `BG_PANEL` fill + `TEXT_MUTED`
/// content, or `BG_MUTED` fill when hovered — `ACCENT_HOVER` is reserved for `action_button` alone,
/// so segmented-cell hover uses a neutral fill instead of the accent hover token. Each cell's
/// content is either a text label or a painter-drawn icon (`CellContent`) — same fill/hover/select
/// logic either way, since the color already carries the selected/unselected distinction and the
/// icon geometry carries its own shape distinction (WCAG 1.4.1: not color alone).
///
/// `id_salt` is threaded into an explicit `egui::Id` (rather than relying on the auto-id egui
/// would otherwise assign this call site) so this control's identity survives if the surrounding
/// UI's widget order shifts frame-to-frame — e.g. the CROP numeric row below appearing/
/// disappearing changes every later auto-id in `controls_ui`, but not an explicitly salted one.
pub(crate) fn segmented(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    selected: &mut usize,
    cells: &[CellContent<'_>],
) -> egui::Response {
    let count = cells.len();
    let width = ui.available_width();
    let height = ROW_HEIGHT;
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
    for (index, ((start, end), cell)) in
        bounds.iter().copied().zip(cells.iter().copied()).enumerate()
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
        let content_color =
            if is_selected { theme::tokens::BG_BASE } else { theme::tokens::TEXT_MUTED };

        painter.rect_filled(cell_rect, 0.0, fill);
        match cell {
            CellContent::Text(label) => {
                painter.text(
                    cell_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    label,
                    egui::FontId::proportional(13.0),
                    content_color,
                );
            }
            CellContent::Icon(kind) => {
                let icon_size = height.min(cell_rect.width()) * 0.55;
                paint_icon(painter, icon_rect(cell_rect, icon_size), content_color, kind);
            }
        }
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

/// Full-width x `ACTION_BUTTON_HEIGHT` call-to-action button ("START PUBLISHING" /
/// "STOP PUBLISHING"). Solid `ACCENT` fill, swapping to `ACCENT_HOVER` while the pointer is over
/// it, with `BG_BASE` icon + text — the same inverted-selection color pairing `segmented`'s
/// selected cell uses. `icon` (`Play`/`Stop`) paints just left of the label; the pair is centered
/// as one block in the button rather than the icon pinned to a fixed lane, since the label's own
/// width (`START PUBLISHING` vs `STOP PUBLISHING`) differs and a fixed icon lane would leave the
/// combined icon+text group off-center for one of the two states.
pub(crate) fn action_button(ui: &mut egui::Ui, icon: IconKind, label: &str) -> egui::Response {
    let width = ui.available_width();
    let height = ACTION_BUTTON_HEIGHT;
    let (rect, response) = ui.allocate_exact_size(egui::vec2(width, height), egui::Sense::click());

    let fill = if response.hovered() { theme::tokens::ACCENT_HOVER } else { theme::tokens::ACCENT };
    let painter = ui.painter();
    painter.rect_filled(rect, 0.0, fill);

    let font_id = egui::FontId::proportional(13.0);
    let galley = painter.layout_no_wrap(label.to_owned(), font_id, theme::tokens::BG_BASE);
    let icon_size = height * 0.45;
    let gap = 8.0;
    let content_width = icon_size + gap + galley.rect.width();
    let content_left = rect.center().x - content_width / 2.0;

    let icon_square = egui::Rect::from_min_size(
        egui::pos2(content_left, rect.center().y - icon_size / 2.0),
        egui::vec2(icon_size, icon_size),
    );
    paint_icon(painter, icon_square, theme::tokens::BG_BASE, icon);

    painter.galley(
        egui::pos2(icon_square.right() + gap, rect.center().y - galley.rect.height() / 2.0),
        galley,
        theme::tokens::BG_BASE,
    );

    response
}

#[cfg(test)]
mod tests {
    use super::{
        CellContent, IconKind, MirrorAxis, TriangleDirection, cell_at, cell_bounds, dash_rect,
        flip_from_segment_index, flip_segment_index, icon_rect, mirror_triangle_pair,
        refresh_arc_points, refresh_arrowhead_points, triangle_points,
    };

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

    // `CellContent`/`IconKind` are exercised through `segmented`'s rendering path, which the repo's
    // "rendering isn't unit-tested" policy excludes — these two asserts just confirm the enums
    // stay usable as plain data (Copy, matchable) without pulling in a `Painter`.
    #[test]
    fn cell_content_variants_are_copy_and_matchable() {
        let text = CellContent::Text("off");
        let icon = CellContent::Icon(IconKind::FlipNone);
        let text_copy = text;
        let icon_copy = icon;
        assert!(matches!(text_copy, CellContent::Text("off")));
        assert!(matches!(icon_copy, CellContent::Icon(IconKind::FlipNone)));
    }

    fn rect(x0: f32, y0: f32, x1: f32, y1: f32) -> egui::Rect {
        egui::Rect::from_min_max(egui::pos2(x0, y0), egui::pos2(x1, y1))
    }

    fn assert_within(point: egui::Pos2, bounds: egui::Rect) {
        assert!(
            bounds.left() <= point.x && point.x <= bounds.right(),
            "x out of bounds: {point:?}"
        );
        assert!(
            bounds.top() <= point.y && point.y <= bounds.bottom(),
            "y out of bounds: {point:?}"
        );
    }

    #[test]
    fn triangle_points_right_tip_touches_the_right_edge_and_base_spans_the_left_edge() {
        let r = rect(0.0, 0.0, 10.0, 20.0);
        let points = triangle_points(r, TriangleDirection::Right);
        assert_eq!(points[2], egui::pos2(10.0, 10.0));
        assert_eq!(points[0].x, 0.0);
        assert_eq!(points[1].x, 0.0);
        assert_eq!(points[0].y, 0.0);
        assert_eq!(points[1].y, 20.0);
    }

    #[test]
    fn triangle_points_left_is_the_horizontal_mirror_of_right() {
        let r = rect(0.0, 0.0, 10.0, 20.0);
        let right = triangle_points(r, TriangleDirection::Right);
        let left = triangle_points(r, TriangleDirection::Left);
        // Mirroring x across the rect's vertical centerline (cx=5) turns Right into Left.
        let cx = r.center().x;
        for point in right {
            let mirrored = egui::pos2(2.0 * cx - point.x, point.y);
            assert!(left.contains(&mirrored), "{point:?} has no mirror match in {left:?}");
        }
    }

    #[test]
    fn triangle_points_all_variants_stay_within_the_source_rect() {
        let r = rect(2.0, 3.0, 12.0, 23.0);
        for direction in [
            TriangleDirection::Left,
            TriangleDirection::Right,
            TriangleDirection::Up,
            TriangleDirection::Down,
        ] {
            for point in triangle_points(r, direction) {
                assert_within(point, r);
            }
        }
    }

    #[test]
    fn mirror_triangle_pair_horizontal_triangles_point_away_from_the_vertical_divider() {
        let r = rect(0.0, 0.0, 20.0, 10.0);
        let (left, right, divider) = mirror_triangle_pair(r, MirrorAxis::Horizontal);
        assert_eq!(divider, [egui::pos2(10.0, 0.0), egui::pos2(10.0, 10.0)]);
        // Left triangle's tip is its Left-pointing apex, which lands on the half-rect's own left
        // edge — i.e. the source rect's left edge, the far side from the divider.
        assert_eq!(left[2].x, 0.0);
        assert_eq!(right[2].x, 20.0);
    }

    #[test]
    fn mirror_triangle_pair_vertical_triangles_point_away_from_the_horizontal_divider() {
        let r = rect(0.0, 0.0, 10.0, 20.0);
        let (top, bottom, divider) = mirror_triangle_pair(r, MirrorAxis::Vertical);
        assert_eq!(divider, [egui::pos2(0.0, 10.0), egui::pos2(10.0, 10.0)]);
        assert_eq!(top[2].y, 0.0);
        assert_eq!(bottom[2].y, 20.0);
    }

    #[test]
    fn icon_rect_is_centered_on_the_source_rect_regardless_of_its_own_shape() {
        let r = rect(0.0, 0.0, 30.0, 10.0);
        let square = icon_rect(r, 8.0);
        assert_eq!(square.center(), r.center());
        assert_eq!(square.width(), 8.0);
        assert_eq!(square.height(), 8.0);
    }

    #[test]
    fn dash_rect_is_centered_and_thinner_than_the_source_rect() {
        let r = rect(0.0, 0.0, 20.0, 20.0);
        let dash = dash_rect(r);
        assert_eq!(dash.center(), r.center());
        assert_eq!(dash.width(), r.width());
        assert!(dash.height() < r.height());
    }

    fn distance(a: egui::Pos2, b: egui::Pos2) -> f32 {
        ((a.x - b.x).powi(2) + (a.y - b.y).powi(2)).sqrt()
    }

    #[test]
    fn refresh_arc_points_all_sit_on_the_same_circle() {
        let r = rect(0.0, 0.0, 20.0, 20.0);
        let center = r.center();
        let points = refresh_arc_points(r);
        let radius = distance(points[0], center);
        for point in &points {
            assert!((distance(*point, center) - radius).abs() < 0.01, "{point:?} off-circle");
        }
    }

    #[test]
    fn refresh_arc_points_leaves_a_gap_it_does_not_close_into_a_full_circle() {
        let r = rect(0.0, 0.0, 20.0, 20.0);
        let points = refresh_arc_points(r);
        let first = *points.first().expect("at least one point");
        let last = *points.last().expect("at least one point");
        // A closed circle would have first == last; a ~70° gap leaves them clearly apart.
        assert!(distance(first, last) > 1.0, "arc endpoints too close: {first:?} vs {last:?}");
    }

    #[test]
    fn refresh_arrowhead_tip_coincides_with_the_arcs_open_end() {
        let r = rect(0.0, 0.0, 20.0, 20.0);
        let arc_end = *refresh_arc_points(r).last().expect("at least one point");
        let arrowhead = refresh_arrowhead_points(r);
        assert!(
            distance(arrowhead[0], arc_end) < 0.01,
            "tip {:?} != arc end {arc_end:?}",
            arrowhead[0]
        );
    }

    #[test]
    fn refresh_arrowhead_is_a_non_degenerate_triangle() {
        let r = rect(0.0, 0.0, 20.0, 20.0);
        let [a, b, c] = refresh_arrowhead_points(r);
        // Twice the signed area via the 2D cross product; zero would mean the three points are
        // collinear (a degenerate, invisible "triangle").
        let cross = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
        assert!(cross.abs() > 0.01, "arrowhead points are collinear: {a:?} {b:?} {c:?}");
    }
}
