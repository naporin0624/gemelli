# 配布準備(licenses / Cannelloni retheme / appmenu About)Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** gemelli GUI を配布可能にする — サードパーティライセンスの生成・アプリ内表示・同梱、UI 全体の Cannelloni palette への retheme、macOS ネイティブ appmenu の About(name / version / build id / author)。

**Architecture:** ライセンスは `cargo bundle-licenses`(JSON)+ 手書き appendix を `cargo xtask gen-licenses` が merge/sort し、GUI へ embed する JSON と `THIRD-PARTY-NOTICES` の 2 出力を単一ソースから生成、cargo-deny + 鮮度検査を CI でゲートする。メニューは muda のネイティブメニューバー(About はネイティブパネル)、Licenses は egui の immediate viewport。retheme は `theme.rs` の token 置換 + WCAG AA contrast proof の全面書き直し。

**Tech Stack:** Rust (edition 2024) / egui + eframe 0.35 / muda 0.19.3 / vergen-gix 10.0.1 / cargo-bundle-licenses 4.2.0 / cargo-deny / serde / clap / GitHub Actions

**Spec:** `docs/superpowers/specs/2026-07-08-distribution-prep-design.md`(承認済み。ただし spec の「vergen 9系」「muda 0.17系」は執筆時点の記述で、本計画の検証済みバージョン 10.0.1 / 0.19.3 が正)

## Global Constraints

- 作業ブランチ: 最初に `git switch -c feature/distribution-prep main` を実行してから Task 1 に入ること(main への直接コミット禁止)
- workspace lints: `clippy::unwrap_used` / `expect_used` / `as_conversions` はすべて **deny**(`clippy.toml` の `allow-unwrap-in-tests = true` / `allow-expect-in-tests = true` により `#[cfg(test)]` 内は例外)
- 各コミット前に必ず: `cargo fmt --all` → `cargo clippy --workspace --all-targets -- -D warnings` → `cargo test --workspace`(husky pre-commit も同等を強制する)
- すべてのコミットメッセージ末尾に空行 + `Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>` trailer を付けること
- コメントはコードから読み取れない制約のみ。過去の文脈(「〜から変更」)への言及は禁止
- 各タスク完了後: difit を起動してレビュー依頼(プロジェクト規約)
- タスクは番号順に実施(Task 6 の `include_str!` は Task 5 の生成物に、Task 3 は Task 2 の env var に依存)
- token 具体値・コントラスト実測値は承認済みモックアップ <https://claude.ai/code/artifact/8ef9d12a-3cd3-4c80-9b69-9fdd4d2dcbc1> と一致していること

## Task Overview

| # | 内容 | 主な成果物 |
| --- | --- | --- |
| 1 | Cannelloni retheme | `theme.rs` 全面書き換え + `app.rs` 2 行 |
| 2 | build id 埋め込み | `build.rs` + vergen-gix |
| 3 | ネイティブメニュー + About | `menu.rs` + muda + `app.rs` 配線 |
| 4 | xtask 純粋関数層 | `crates/xtask`(normalize / merge / sort / render) |
| 5 | xtask シェル層 + 生成物 | `gen-licenses` / `--check` + appendix + 生成物コミット |
| 6 | licenses データ層 | `licenses.rs`(parse / filter) |
| 7 | licenses ウィンドウ | `LicensesWindow`(immediate viewport)+ メニュー配線 |
| 8 | ポリシー + CI | `deny.toml` + `.github/workflows/license-check.yml` |

---
### Task 1: Cannelloni retheme (`crates/gui/src/theme.rs`)

**Files:**
- Modify: `crates/gui/src/theme.rs` (full rewrite — tokens module + `apply_theme` + tests)
- Modify: `crates/gui/src/app.rs` (2 call-site edits, lines 388 and 390 only)
- Test: tests live inline in `crates/gui/src/theme.rs` under `#[cfg(test)] mod tests` (this crate is a binary crate — `gemelli-gui`, no `lib.rs` — so there is nowhere else to put them)

**Interfaces:**
- Consumes: nothing new. Depends only on `egui` 0.35 (`egui::Color32`, `egui::Stroke`, `egui::CornerRadius`, `egui::Visuals`, `egui::Context`), already a workspace dependency.
- Produces (for later tasks — Task 3 `menu.rs`, Tasks 6–7 `licenses.rs` — to import):
  - `crate::theme::tokens::{BG_BASE, BG_PANEL, BG_MUTED, TEXT_PRIMARY, TEXT_MUTED, TEXT_SUBTLE, ACCENT, ACCENT_HOVER, ACCENT_ALT, DANGER, BORDER, BORDER_SUBTLE, CROP_OVERLAY}` — all `pub const _: egui::Color32`.
  - `crate::theme::apply_theme(ctx: &egui::Context)` — unchanged signature, called once from `GemelliApp::new`.
  - `crate::theme::contrast_ratio(a: Color32, b: Color32) -> f64` — unchanged, `#[cfg_attr(not(test), allow(dead_code))]`, available for Task 6's `licenses.rs` tests if it needs to prove `BG_MUTED`'s contrast once it has a real text pairing.
  - **Removed** (do not use in later tasks): `ACCENT_PUBLISH`, `ACCENT_IDLE`, `SELECTION_BG`. Replaced by `ACCENT`, `TEXT_SUBTLE`, and direct use of `ACCENT`/`BG_BASE` in `apply_theme`'s selection styling, respectively.

---

#### Context: why this crate can't do isolated red/green on tokens alone

`gemelli-gui` is a binary crate (`main.rs`, no `lib.rs`) — `theme.rs` and `app.rs` are both modules of the *same* compilation unit. This means removing a token constant that `app.rs` still references is a compile error in `app.rs`, not a normal test failure in `theme.rs`. The steps below embrace that reality: Step 1 gets a clean "red" by adding tests that reference tokens which don't exist *yet*, Step 2 lands the new `theme.rs` (still red, but now for a different, expected reason — `app.rs`'s old references), and Step 3 fixes the call sites to reach green. This has been dry-run against the real repo; the exact compiler output is reproduced below.

---

#### API verification (egui 0.35.0, confirmed from local registry source)

Source checked: `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/egui-0.35.0/src/style.rs` and `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/epaint-0.35.0/src/corner_radius.rs`.

- `egui::Visuals` (struct, `style.rs:985`) relevant fields: `window_fill: Color32`, `panel_fill: Color32`, `override_text_color: Option<Color32>`, `weak_text_color: Option<Color32>`, `hyperlink_color: Color32`, `selection: Selection`, `widgets: Widgets`, `window_corner_radius: CornerRadius`, `menu_corner_radius: CornerRadius`.
- `egui::Selection` (`style.rs:1188`): `pub bg_fill: Color32`, `pub stroke: Stroke`.
- `egui::Widgets` (`style.rs:1244`): `pub noninteractive: WidgetVisuals`, `pub inactive: WidgetVisuals`, `pub hovered: WidgetVisuals`, `pub active: WidgetVisuals`, `pub open: WidgetVisuals`.
- `egui::style::WidgetVisuals` (`style.rs:1284`): `pub bg_fill: Color32`, `pub weak_bg_fill: Color32`, `pub bg_stroke: Stroke`, `pub corner_radius: CornerRadius`, `pub fg_stroke: Stroke`, `pub expansion: f32`.
- `epaint::CornerRadius` (`corner_radius.rs:13`): `{ nw: u8, ne: u8, sw: u8, se: u8 }`, `#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]`. Has `pub const ZERO: Self = Self { nw: 0, ne: 0, sw: 0, se: 0 }` (`corner_radius.rs:50`) — re-exported as `egui::CornerRadius`, confirmed via `egui::CornerRadius::ZERO` used elsewhere in egui's own source (`containers/frame.rs:165`, `widgets/image.rs:240`).
- `epaint::Stroke` (`stroke.rs:12`): `{ width: f32, color: Color32 }`, `#[derive(Clone, Copy, Debug, Default, PartialEq)]`, constructor `Stroke::new(width: f32, color: impl Into<Color32>) -> Self`.
- `egui::Visuals::dark()` (`style.rs:1489`) defaults relevant to this task (i.e. what we're overriding): `window_corner_radius: CornerRadius::same(6)`, `menu_corner_radius: CornerRadius::same(6)`, and per `Widgets::dark()` (`style.rs:1673`) every one of `noninteractive`/`inactive`/`hovered`/`active`/`open` starts at `corner_radius: CornerRadius::same(2)` or `same(3)` (never zero) — so all five must be explicitly zeroed; `CornerRadius::default()` being `ZERO` does **not** help here since `Visuals::dark()` never calls `default()` for these fields.
- Building an array of disjoint mutable field references — `[&mut visuals.widgets.noninteractive, &mut visuals.widgets.inactive, ...]` — compiles under NLL (disjoint field borrows of the same struct in one expression); confirmed by actually compiling it (see verification below).

---

#### Token contract (exact hex values — pre-verified contrast ratios recomputed independently below and match the design doc exactly)

| token | hex | rgb | oklch source | contrast |
| --- | --- | --- | --- | --- |
| `BG_BASE` | `#121212` | (18,18,18) | dark.canvas 0.180 0 0 | — |
| `BG_PANEL` | `#1c1c1c` | (28,28,28) | dark.subtle 0.225 0 0 | — |
| `BG_MUTED` (new) | `#262626` | (38,38,38) | dark.muted 0.270 0 0 | reserved, unused this task |
| `TEXT_PRIMARY` | `#eeeff2` | (238,239,242) | gray.1 | 16.29 on BG_BASE, 14.82 on BG_PANEL |
| `TEXT_MUTED` | `#c9ccd1` | (201,204,209) | gray.6 | 11.63 on BG_BASE, 10.58 on BG_PANEL |
| `TEXT_SUBTLE` (was `ACCENT_IDLE`) | `#9a9ea7` | (154,158,167) | gray.8 | 6.98 on BG_BASE, 6.35 on BG_PANEL |
| `ACCENT` (was `ACCENT_PUBLISH`) | `#3996ff` | (57,150,255) | neon.blue | 6.23 on BG_BASE, 5.66 on BG_PANEL |
| `ACCENT_HOVER` (new) | `#2785ff` | (39,133,255) | neon.blueHover | reserved, unused this task |
| `ACCENT_ALT` (new) | `#34dde5` | (52,221,229) | neon.cyan | 11.26 on BG_BASE |
| selection text (inverted) | `BG_BASE` on `ACCENT` fill | — | accent.solid / fg.onSolid | 6.23 (same ratio, reversed roles) |
| `DANGER` | `#ff2939` | (255,41,57) | red.text, L bumped 0.650→0.660 | 5.02 on BG_BASE, 4.57 on BG_PANEL |
| `BORDER` (new) | `#696969` | (105,105,105) | dark.border | 3.41 on BG_BASE, 3.10 on BG_PANEL |
| `BORDER_SUBTLE` (new) | `#424242` | (66,66,66) | dark.borderSubtle | reserved, unused this task |
| `CROP_OVERLAY` | `Color32::WHITE` | — | unchanged | — |

All numbers above were recomputed independently with the same `contrast_ratio`/`relative_luminance`/`linearize` formula as this file (WCAG 2.1, `(L1+0.05)/(L2+0.05)`) and match the design doc's hand-computed values exactly — no drift.

**Note on `SELECTION_BG`:** the old token is removed entirely, not renamed. Its value (`#3996ff`) is identical to `ACCENT`'s new value, so `apply_theme` uses `tokens::ACCENT` directly for `selection.bg_fill` — there is no separate constant.

**Dead-code bookkeeping (why 3 of the 5 new tokens carry `#[allow(dead_code)]`):** `gemelli-gui` has no `lib.rs`, so `pub` items in a private `mod theme` are only as visible as the crate itself — an unreferenced `pub const` is flagged as dead code, and CI runs `cargo clippy --workspace --all-targets -- -D warnings`, which turns that warning into a hard failure. This task's `apply_theme` only reaches into `BG_BASE`, `BG_PANEL`, `TEXT_PRIMARY`, `TEXT_MUTED`, `ACCENT`, and `BORDER`; `TEXT_SUBTLE`, `DANGER`, and `CROP_OVERLAY` are reached from `app.rs`'s call sites. That leaves `BG_MUTED`, `ACCENT_HOVER`, and `BORDER_SUBTLE` genuinely unused until Task 7 (licenses window) and future widget-hover/slider work consume them — each gets a forward-looking doc comment plus `#[allow(dead_code)]` (or `#[cfg_attr(not(test), allow(dead_code))]` for `ACCENT_ALT`, which *is* exercised by a contrast-proof test but not yet by production code). This was verified empirically: `cargo clippy --workspace --all-targets -- -D warnings` was run against the exact new file below and produced zero warnings.

---

- [ ] **Step 1: Add failing tests for the new/changed token contract**

  Replace the `#[cfg(test)] mod tests { ... }` block at the bottom of `crates/gui/src/theme.rs` with the block shown in Step 2's full file below (the tests are written against token names — `ACCENT`, `TEXT_SUBTLE`, `BORDER`, `ACCENT_ALT`, the inverted-selection assertion, the corner-radius assertions, the border-stroke assertion — that do not exist in the current `tokens` module yet). Do not touch anything else in the file at this sub-step.

  Command:
  ```
  cargo test -p gemelli-gui theme
  ```

  Expected output (RED — compile error, confirmed by actually running this against the current `tokens` module):
  ```
  error[E0425]: cannot find value `TEXT_SUBTLE` in module `tokens`
     --> crates/gui/src/theme.rs:137:40
      |
  137 |         assert!(contrast_ratio(tokens::TEXT_SUBTLE, tokens::BG_BASE) >= 4.5);
      |                                        ^^^^^^^^^^^ not found in `tokens`

  error[E0425]: cannot find value `ACCENT` in module `tokens`
     --> crates/gui/src/theme.rs:149:40
      |
  149 |         assert!(contrast_ratio(tokens::ACCENT, tokens::BG_BASE) >= 4.5);
      |                                        ^^^^^^ not found in `tokens`

  error[E0425]: cannot find value `BORDER` in module `tokens`
     --> crates/gui/src/theme.rs:183:40
      |
  183 |         assert!(contrast_ratio(tokens::BORDER, tokens::BG_BASE) >= 3.0);
      |                                        ^^^^^^ not found in `tokens`

  error[E0425]: cannot find value `ACCENT_ALT` in module `tokens`
     --> crates/gui/src/theme.rs:193:40
      |
  193 |         assert!(contrast_ratio(tokens::ACCENT_ALT, tokens::BG_BASE) >= 3.0);
      |                                        ^^^^^^^^^^
  ```
  (plus more of the same shape for every other new/renamed reference — `cannot find value ... in module tokens`). This is the expected RED: the tests describe the target API before it exists.

- [ ] **Step 2: Replace `crates/gui/src/theme.rs` with the full new implementation**

  Overwrite the entire file with:

  ```rust
  //! WCAG 2.1 AA color tokens for the gemelli GUI — Cannelloni palette — plus
  //! the contrast-ratio calculation used to prove them (see `tokens` below).

  use egui::Color32;

  /// WCAG 2.1 relative-luminance contrast ratio between two colors.
  /// Formula: <https://www.w3.org/TR/WCAG21/#dfn-contrast-ratio>.
  ///
  /// Only exercised by this module's tests (there is no lib target to export
  /// it from), hence `allow(dead_code)` outside `cfg(test)`.
  #[cfg_attr(not(test), allow(dead_code))]
  pub fn contrast_ratio(a: Color32, b: Color32) -> f64 {
      let luminance_a = relative_luminance(a);
      let luminance_b = relative_luminance(b);
      let (lighter, darker) = if luminance_a >= luminance_b {
          (luminance_a, luminance_b)
      } else {
          (luminance_b, luminance_a)
      };
      (lighter + 0.05) / (darker + 0.05)
  }

  #[cfg_attr(not(test), allow(dead_code))]
  fn relative_luminance(color: Color32) -> f64 {
      let red = linearize(color.r());
      let green = linearize(color.g());
      let blue = linearize(color.b());
      0.2126 * red + 0.7152 * green + 0.0722 * blue
  }

  #[cfg_attr(not(test), allow(dead_code))]
  fn linearize(channel: u8) -> f64 {
      let normalized = f64::from(channel) / 255.0;
      if normalized <= 0.03928 {
          normalized / 12.92
      } else {
          ((normalized + 0.055) / 1.055).powf(2.4)
      }
  }

  /// Dark-theme color tokens converted from Cannelloni's `panda.config.ts` oklch
  /// primitives to sRGB `Color32`. Every token's contrast ratio against the
  /// background(s) it is meant to sit on is proved by the tests in this module —
  /// see the design doc (`docs/superpowers/specs/2026-07-08-distribution-prep-design.md`,
  /// section 1) for the oklch source and the hand-computed numbers behind each choice.
  pub mod tokens {
      use egui::Color32;

      /// Window background — Cannelloni `dark.canvas` (oklch 0.180 0 0).
      pub const BG_BASE: Color32 = Color32::from_rgb(18, 18, 18);
      /// Panel/sidebar/status-bar background — Cannelloni `dark.subtle` (oklch 0.225 0 0).
      pub const BG_PANEL: Color32 = Color32::from_rgb(28, 28, 28);
      /// Expanded-row background — Cannelloni `dark.muted` (oklch 0.270 0 0). Not yet
      /// consumed: reserved for the licenses window's expanded-entry background
      /// (design doc section 3, a later task) — `allow(dead_code)` until that call
      /// site exists.
      #[allow(dead_code)]
      pub const BG_MUTED: Color32 = Color32::from_rgb(38, 38, 38);

      /// Primary text — Cannelloni `gray.1` (oklch 0.952 0.004 265).
      pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(238, 239, 242);
      /// Secondary text — Cannelloni `gray.6` (oklch 0.845 0.008 265).
      pub const TEXT_MUTED: Color32 = Color32::from_rgb(201, 204, 209);
      /// Idle-state text — Cannelloni `gray.8` (oklch 0.700 0.013 265). Paired with
      /// the "○ stopped" text label at the call site — never color alone (WCAG 1.4.1).
      /// Replaces the old `ACCENT_IDLE`.
      pub const TEXT_SUBTLE: Color32 = Color32::from_rgb(154, 158, 167);

      /// Publishing state / links / selection fill — Cannelloni `neon.blue`
      /// (oklch 0.700 0.235 260). Paired with the "● publishing" text label at the
      /// call site — never color alone (WCAG 1.4.1). Replaces the old `ACCENT_PUBLISH`.
      /// Used as both a UI-component color (selection fill, 3.0:1 threshold) and as
      /// text (hyperlinks, the publishing label), so it is proved against the
      /// stricter 4.5:1 normal-text threshold.
      pub const ACCENT: Color32 = Color32::from_rgb(57, 150, 255);
      /// Hover-fill only — Cannelloni `neon.blueHover` (oklch 0.650 0.235 260). Not
      /// yet consumed: applying this to `Visuals::widgets.hovered` fills is future
      /// widget-hover styling work, out of this task's scope — `allow(dead_code)`
      /// until that call site exists.
      #[allow(dead_code)]
      pub const ACCENT_HOVER: Color32 = Color32::from_rgb(39, 133, 255);
      /// Slider-fill only — Cannelloni `neon.cyan` (oklch 0.820 0.130 200). Not yet
      /// consumed: no slider exists in the GUI yet — `allow(dead_code)` until one
      /// does. Proved at the 3.0:1 non-text threshold (WCAG 1.4.11) since it will
      /// only ever fill a widget, never render as text.
      #[cfg_attr(not(test), allow(dead_code))]
      pub const ACCENT_ALT: Color32 = Color32::from_rgb(52, 221, 229);

      /// Danger/error text. Deliberate deviation from the Cannelloni primitive
      /// `oklch(0.650 0.250 25)`: at that lightness this color only reaches
      /// 4.497:1 on `BG_PANEL` — just under the 4.5:1 AA threshold (Cannelloni
      /// itself only ever draws error text on `canvas`, not `subtle`, so the
      /// primitive never had to clear this bar). `gemelli`'s error banner renders
      /// on `BG_PANEL` (see `app.rs`), so L is bumped to 0.660, landing at 4.57:1.
      pub const DANGER: Color32 = Color32::from_rgb(255, 41, 57);

      /// 2px interactive-widget outline (`apply_theme`) — Cannelloni `dark.border`
      /// (oklch 0.520 0 0). Proved at the 3.0:1 non-text/UI-component threshold
      /// (WCAG 1.4.11), since it is a stroke, never text.
      pub const BORDER: Color32 = Color32::from_rgb(105, 105, 105);
      /// Non-informational divider lines only — Cannelloni `dark.borderSubtle`
      /// (oklch 0.380 0 0). Not yet consumed: reserved for the licenses window's
      /// hairline dividers (design doc section 3, a later task) — `allow(dead_code)`
      /// until that call site exists. No contrast proof needed: WCAG 1.4.11 exempts
      /// purely decorative, non-informational separators.
      #[allow(dead_code)]
      pub const BORDER_SUBTLE: Color32 = Color32::from_rgb(66, 66, 66);

      /// Crop-rect stroke. Drawn as a dual stroke (black outline + white core) at
      /// the `preview_ui` call site, since no single color has a provable contrast
      /// ratio against arbitrary live video content. Unchanged by the retheme.
      pub const CROP_OVERLAY: Color32 = Color32::WHITE;
  }

  /// Applies the `tokens` palette to `ctx`'s `Visuals`. Called once at startup
  /// from `GemelliApp::new`.
  pub fn apply_theme(ctx: &egui::Context) {
      let mut visuals = egui::Visuals::dark();
      visuals.window_fill = tokens::BG_BASE;
      visuals.panel_fill = tokens::BG_PANEL;
      visuals.override_text_color = Some(tokens::TEXT_PRIMARY);
      visuals.weak_text_color = Some(tokens::TEXT_MUTED);
      visuals.hyperlink_color = tokens::ACCENT;

      // Inverted selection: Cannelloni draws the selected/active state as a solid
      // ACCENT fill with dark (`fg.onSolid`-equivalent) text on top, not the usual
      // light-text-on-dark-fill pairing. egui 0.35 renders selected-widget text
      // using `selection.stroke` as its fg_stroke color — `override_text_color`
      // does not reach selected widgets (verified in egui 0.35 source) — so
      // setting that to `BG_BASE` is what makes the text read as "dark on blue".
      visuals.selection.bg_fill = tokens::ACCENT;
      visuals.selection.stroke = egui::Stroke::new(1.0, tokens::BG_BASE);

      // Neo-brutalist "terminal-print" identity: no rounded corners anywhere,
      // on any widget interaction state, window, or menu/popup.
      visuals.window_corner_radius = egui::CornerRadius::ZERO;
      visuals.menu_corner_radius = egui::CornerRadius::ZERO;
      for widget in [
          &mut visuals.widgets.noninteractive,
          &mut visuals.widgets.inactive,
          &mut visuals.widgets.hovered,
          &mut visuals.widgets.active,
          &mut visuals.widgets.open,
      ] {
          widget.corner_radius = egui::CornerRadius::ZERO;
      }

      // 2px ink border on every *interactive* widget state. `noninteractive` is
      // deliberately left alone here — it is egui's passive-chrome style (window
      // outlines, separators), not an interactive widget, and is out of this
      // task's scope.
      let border_stroke = egui::Stroke::new(2.0, tokens::BORDER);
      visuals.widgets.inactive.bg_stroke = border_stroke;
      visuals.widgets.hovered.bg_stroke = border_stroke;
      visuals.widgets.active.bg_stroke = border_stroke;
      visuals.widgets.open.bg_stroke = border_stroke;

      ctx.set_visuals(visuals);
  }

  #[cfg(test)]
  mod tests {
      use super::*;
      use egui::Color32;

      #[test]
      fn black_and_white_ratio_is_21() {
          let ratio = contrast_ratio(Color32::WHITE, Color32::BLACK);
          assert!((ratio - 21.0).abs() < 0.01, "expected ~21.0, got {ratio}");
      }

      #[test]
      fn same_color_ratio_is_1() {
          let gray = Color32::from_rgb(100, 100, 100);
          let ratio = contrast_ratio(gray, gray);
          assert!((ratio - 1.0).abs() < 0.0001, "expected 1.0, got {ratio}");
      }

      #[test]
      fn ratio_is_symmetric_in_argument_order() {
          let a = Color32::from_rgb(230, 230, 230);
          let b = Color32::from_rgb(26, 27, 30);
          assert!((contrast_ratio(a, b) - contrast_ratio(b, a)).abs() < 1e-9);
      }

      #[test]
      fn text_primary_meets_normal_text_contrast_on_bg_base() {
          assert!(contrast_ratio(tokens::TEXT_PRIMARY, tokens::BG_BASE) >= 4.5);
      }

      #[test]
      fn text_primary_meets_normal_text_contrast_on_bg_panel() {
          assert!(contrast_ratio(tokens::TEXT_PRIMARY, tokens::BG_PANEL) >= 4.5);
      }

      #[test]
      fn text_muted_meets_normal_text_contrast_on_bg_base() {
          assert!(contrast_ratio(tokens::TEXT_MUTED, tokens::BG_BASE) >= 4.5);
      }

      #[test]
      fn text_muted_meets_normal_text_contrast_on_bg_panel() {
          assert!(contrast_ratio(tokens::TEXT_MUTED, tokens::BG_PANEL) >= 4.5);
      }

      #[test]
      fn text_subtle_meets_normal_text_contrast_on_bg_base() {
          // TEXT_SUBTLE renders as the "○ stopped" *text* label (statusbar_ui), so
          // it must clear the 4.5:1 normal-text threshold, not a 3.0:1 UI-component
          // threshold.
          assert!(contrast_ratio(tokens::TEXT_SUBTLE, tokens::BG_BASE) >= 4.5);
      }

      #[test]
      fn text_subtle_meets_normal_text_contrast_on_bg_panel() {
          assert!(contrast_ratio(tokens::TEXT_SUBTLE, tokens::BG_PANEL) >= 4.5);
      }

      #[test]
      fn accent_meets_normal_text_contrast_on_bg_base() {
          // ACCENT renders as the "● publishing" text label and as hyperlink text,
          // so — like TEXT_SUBTLE above — it is held to the 4.5:1 normal-text bar.
          assert!(contrast_ratio(tokens::ACCENT, tokens::BG_BASE) >= 4.5);
      }

      #[test]
      fn accent_meets_normal_text_contrast_on_bg_panel() {
          assert!(contrast_ratio(tokens::ACCENT, tokens::BG_PANEL) >= 4.5);
      }

      #[test]
      fn inverted_selection_text_meets_normal_text_contrast() {
          // `apply_theme` paints selected text as BG_BASE on an ACCENT fill (the
          // inverted-selection scheme) — prove that pairing directly, in the
          // order it is actually rendered.
          assert!(contrast_ratio(tokens::BG_BASE, tokens::ACCENT) >= 4.5);
      }

      #[test]
      fn danger_meets_normal_text_contrast_on_bg_base() {
          assert!(contrast_ratio(tokens::DANGER, tokens::BG_BASE) >= 4.5);
      }

      /// The banner (`app.rs`'s `DANGER`-colored error label) renders on `BG_PANEL`, not
      /// `BG_BASE` — `egui::Panel::top` inherits `visuals.panel_fill`. Retargets the contrast
      /// proof at the surface `DANGER` actually sits on; the `BG_BASE` assertion above is kept
      /// too since it still holds and other `DANGER` usages may sit on the window background.
      #[test]
      fn danger_meets_normal_text_contrast_on_bg_panel() {
          assert!(contrast_ratio(tokens::DANGER, tokens::BG_PANEL) >= 4.5);
      }

      #[test]
      fn border_meets_non_text_contrast_on_bg_base() {
          // WCAG 1.4.11 non-text/UI-component threshold — BORDER is only ever a
          // stroke, never text.
          assert!(contrast_ratio(tokens::BORDER, tokens::BG_BASE) >= 3.0);
      }

      #[test]
      fn border_meets_non_text_contrast_on_bg_panel() {
          assert!(contrast_ratio(tokens::BORDER, tokens::BG_PANEL) >= 3.0);
      }

      #[test]
      fn accent_alt_meets_non_text_contrast_on_bg_base() {
          assert!(contrast_ratio(tokens::ACCENT_ALT, tokens::BG_BASE) >= 3.0);
      }

      #[test]
      fn apply_theme_sets_dark_mode_and_token_fills() {
          let ctx = egui::Context::default();
          apply_theme(&ctx);
          let visuals = ctx.global_style().visuals.clone();
          assert!(visuals.dark_mode);
          assert_eq!(visuals.window_fill, tokens::BG_BASE);
          assert_eq!(visuals.panel_fill, tokens::BG_PANEL);
          assert_eq!(visuals.override_text_color, Some(tokens::TEXT_PRIMARY));
          assert_eq!(visuals.weak_text_color, Some(tokens::TEXT_MUTED));
          assert_eq!(visuals.hyperlink_color, tokens::ACCENT);
      }

      #[test]
      fn apply_theme_inverts_selection_colors() {
          let ctx = egui::Context::default();
          apply_theme(&ctx);
          let visuals = ctx.global_style().visuals.clone();
          assert_eq!(visuals.selection.bg_fill, tokens::ACCENT);
          assert_eq!(visuals.selection.stroke.color, tokens::BG_BASE);
      }

      #[test]
      fn apply_theme_zeroes_all_corner_radii() {
          let ctx = egui::Context::default();
          apply_theme(&ctx);
          let visuals = ctx.global_style().visuals.clone();
          assert_eq!(visuals.window_corner_radius, egui::CornerRadius::ZERO);
          assert_eq!(visuals.menu_corner_radius, egui::CornerRadius::ZERO);
          assert_eq!(visuals.widgets.noninteractive.corner_radius, egui::CornerRadius::ZERO);
          assert_eq!(visuals.widgets.inactive.corner_radius, egui::CornerRadius::ZERO);
          assert_eq!(visuals.widgets.hovered.corner_radius, egui::CornerRadius::ZERO);
          assert_eq!(visuals.widgets.active.corner_radius, egui::CornerRadius::ZERO);
          assert_eq!(visuals.widgets.open.corner_radius, egui::CornerRadius::ZERO);
      }

      #[test]
      fn apply_theme_sets_border_stroke_on_interactive_widgets() {
          let ctx = egui::Context::default();
          apply_theme(&ctx);
          let visuals = ctx.global_style().visuals.clone();
          let expected = egui::Stroke::new(2.0, tokens::BORDER);
          assert_eq!(visuals.widgets.inactive.bg_stroke, expected);
          assert_eq!(visuals.widgets.hovered.bg_stroke, expected);
          assert_eq!(visuals.widgets.active.bg_stroke, expected);
          assert_eq!(visuals.widgets.open.bg_stroke, expected);
      }
  }
  ```

  Command:
  ```
  cargo test -p gemelli-gui theme
  ```

  Expected output at this sub-step (still RED, but now a *different* error — confirmed by actually running this): `app.rs` still references the two removed constants, so the crate fails to compile in `app.rs`, not in `theme.rs`:
  ```
  error[E0425]: cannot find value `ACCENT_PUBLISH` in module `theme::tokens`
     --> crates/gui/src/app.rs:388:49
      |
  388 |                 ui.colored_label(theme::tokens::ACCENT_PUBLISH, "\u{25cf} publishing");
      |                                                 ^^^^^^^^^^^^^^ not found in `theme::tokens`

  error[E0425]: cannot find value `ACCENT_IDLE` in module `theme::tokens`
     --> crates/gui/src/app.rs:390:49
      |
  390 |                 ui.colored_label(theme::tokens::ACCENT_IDLE, "\u{25cb} stopped");
      |                                                 ^^^^^^^^^^^ not found in `theme::tokens`
  ```
  This confirms the new `tokens` module and `apply_theme` compile cleanly in isolation — the only remaining failure is the two call sites, fixed next.

- [ ] **Step 3: Update the two call sites in `crates/gui/src/app.rs`**

  Grep confirms these are the *only* two lines in the whole `crates/gui/src` tree that reference the renamed/removed tokens (`ACCENT_PUBLISH`, `ACCENT_IDLE`); `SELECTION_BG` had no call sites outside `theme.rs` itself. `DANGER` (line 478) and `CROP_OVERLAY` (line 435) keep their names — only their underlying `Color32` values changed via the `tokens` module edit in Step 2, so those two lines need no edit. `sidebar.rs` and `preview.rs` (also under `crates/gui/src`) contain no `theme::tokens::` references at all — `sidebar_ui`, `statusbar_ui`, and `preview_ui` are methods defined inside `app.rs`, not separate files.

  In `crates/gui/src/app.rs`, `statusbar_ui` (around line 386-391):

  Before:
  ```rust
              let running = self.worker.as_ref().is_some_and(WorkerHandle::is_running);
              if running {
                  ui.colored_label(theme::tokens::ACCENT_PUBLISH, "\u{25cf} publishing");
              } else {
                  ui.colored_label(theme::tokens::ACCENT_IDLE, "\u{25cb} stopped");
              }
  ```

  After:
  ```rust
              let running = self.worker.as_ref().is_some_and(WorkerHandle::is_running);
              if running {
                  ui.colored_label(theme::tokens::ACCENT, "\u{25cf} publishing");
              } else {
                  ui.colored_label(theme::tokens::TEXT_SUBTLE, "\u{25cb} stopped");
              }
  ```

  Only two tokens change (`ACCENT_PUBLISH` → `ACCENT`, `ACCENT_IDLE` → `TEXT_SUBTLE`); the "● publishing" / "○ stopped" text labels are kept verbatim (WCAG 1.4.1 — color must never be the only signal).

  Command:
  ```
  cargo test -p gemelli-gui theme
  ```

  Expected output (GREEN — confirmed by actually running this exact sequence against the real repo):
  ```
  running 21 tests
  test theme::tests::accent_alt_meets_non_text_contrast_on_bg_base ... ok
  test theme::tests::accent_meets_normal_text_contrast_on_bg_panel ... ok
  test theme::tests::accent_meets_normal_text_contrast_on_bg_base ... ok
  test theme::tests::black_and_white_ratio_is_21 ... ok
  test theme::tests::border_meets_non_text_contrast_on_bg_base ... ok
  test theme::tests::danger_meets_normal_text_contrast_on_bg_base ... ok
  test theme::tests::border_meets_non_text_contrast_on_bg_panel ... ok
  test theme::tests::danger_meets_normal_text_contrast_on_bg_panel ... ok
  test theme::tests::inverted_selection_text_meets_normal_text_contrast ... ok
  test theme::tests::ratio_is_symmetric_in_argument_order ... ok
  test theme::tests::same_color_ratio_is_1 ... ok
  test theme::tests::text_muted_meets_normal_text_contrast_on_bg_base ... ok
  test theme::tests::text_primary_meets_normal_text_contrast_on_bg_base ... ok
  test theme::tests::text_muted_meets_normal_text_contrast_on_bg_panel ... ok
  test theme::tests::text_primary_meets_normal_text_contrast_on_bg_panel ... ok
  test theme::tests::text_subtle_meets_normal_text_contrast_on_bg_base ... ok
  test theme::tests::text_subtle_meets_normal_text_contrast_on_bg_panel ... ok
  test theme::tests::apply_theme_sets_border_stroke_on_interactive_widgets ... ok
  test theme::tests::apply_theme_inverts_selection_colors ... ok
  test theme::tests::apply_theme_sets_dark_mode_and_token_fills ... ok
  test theme::tests::apply_theme_zeroes_all_corner_radii ... ok

  test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 60 filtered out; finished in 0.00s
  ```

- [ ] **Step 4: Full lint/test gate before committing**

  Commands (all confirmed clean against the real repo with the exact file contents above):
  ```
  cargo fmt --all
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  ```
  Expected: `cargo fmt --all -- --check` exits 0 with no diff (the file above is already fmt-clean); `cargo clippy` produces no warnings (in particular, no `dead_code` warnings — every new token either has a real call site in `apply_theme`/`app.rs` or carries an explicit `allow(dead_code)`/`cfg_attr(not(test), allow(dead_code))`); `cargo test --workspace` shows `test result: ok. 79 passed; 0 failed; 2 ignored` (the 2 ignored are the pre-existing hardware-dependent tests, unrelated to this task).

- [ ] **Step 5: Commit**

  ```
  git add crates/gui/src/theme.rs crates/gui/src/app.rs
  git commit -m "$(cat <<'EOF'
  feat(gui): retheme to Cannelloni palette

  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  EOF
  )"
  ```

  Then run `difit` for review, per project convention, before moving to Task 2.

---

#### Verification performed while writing this plan

To avoid handing off unverified code, the full `theme.rs` above and the 2-line `app.rs` edit were applied directly to a clean working tree (`git status` confirmed clean before and after; changes were reverted with the saved originals once verification completed — no trace left in the working tree) and:
- `cargo test -p gemelli-gui theme` → 21/21 passed.
- `cargo clippy --workspace --all-targets -- -D warnings` → zero warnings.
- `cargo fmt --all -- --check` → exit 0.
- `cargo test --workspace` → 79 passed, 0 failed, 2 ignored (pre-existing hardware-gated tests).
- The two intermediate RED states (Step 1's and Step 2's expected compiler errors, quoted above) were also reproduced by actually running the corresponding partial edits, not guessed.

---

## API verification log (read this before the tasks)

All signatures below were confirmed by **downloading the actual crate source from
crates.io and compiling/running throwaway PoCs** (not just reading docs.rs prose,
which was often too vague to trust for exact signatures). Versions are pinned to
what is actually published today; the design doc's "vergen 9系" / "muda 0.17系" are
stale — the current stable releases are newer and both were verified to work.

- **`vergen-gix` 10.0.1** (crates.io `max_stable_version`, published 2026-06-25).
  Requires `rust-version = "1.95.0"`; this workspace's `rustc 1.96.1` satisfies it.
  - `vergen_gix::Gix::builder().sha(true).build()` — the `with` closure backing the
    `sha(bool)` builder method is `|short: bool| Some(Sha::builder().short(short).build())`
    (confirmed in `vergen-gix-10.0.1/src/gix/mod.rs`), so `sha(true)` means **short**
    SHA. This is also directly exercised by vergen's own test `git_sha` (`Gix::builder().sha(false).build()`
    for the full-length case). No other `Gix` field is enabled unless `.all()` is
    called first, so `Gix::builder().sha(true).build()` emits *only* `VERGEN_GIT_SHA`.
  - `vergen_gix::Build` (re-exported from `vergen` crate, gated behind the
    `vergen-gix` `"build"` feature) — `Build::builder().build_date(true).build()`
    emits `VERGEN_BUILD_DATE`. This one never depends on git and is always emitted.
  - `Emitter::default().add_instructions(&gix)?.add_instructions(&build)?.emit()?` —
    `add_instructions(&mut self, &dyn AddEntries) -> Result<&mut Self, anyhow::Error>`,
    `emit(&self) -> Result<(), anyhow::Error>`. Chaining through a temporary
    `Emitter::default()` is the pattern used in vergen-gix's own doc examples and
    compiles (temporary lifetime extension over the whole statement).
  - **Fallback behavior (empirically verified, see PoC below): the *default*
    `Emitter` — without calling `.fail_on_error()`, `.idempotent()`, or
    `.default_on_error()` — never fails the build when git info is unavailable.**
    It just leaves the affected var **unset** and prints one `cargo:warning`. This
    is exactly the semantics the design and this task's contract need: consumer
    code reads `option_env!("VERGEN_GIT_SHA")` and only *that* absence-not-a-panic
    behavior makes the `.unwrap_or("unknown")` fallback meaningful. (Using
    `.idempotent()`/`.default_on_error()` instead would make the var always
    `Some("VERGEN_IDEMPOTENT_OUTPUT")`, which would make the `"unknown"` fallback in
    consumer code dead code — so those flags are deliberately **not** used.)
  - PoC (run twice — outside then inside a git repo — see full transcript in this
    session): outside a repo, `cargo run` printed `sha=unknown date=2026-07-08` with
    a `cargo:warning=... Unable to set VERGEN_GIT_SHA` and **exit code 0**; inside a
    freshly-initialized git repo it printed `sha=54ee65c date=2026-07-08` with no
    warnings. `cargo clippy --all-targets -- -D warnings` against the same build.rs
    (with the workspace's exact `unwrap_used`/`expect_used`/`as_conversions` deny
    list copied in) additionally proved that **`.expect()` on the `Emitter` chain is
    rejected by clippy** — the build.rs below therefore uses `.map_err(|e|
    e.to_string())?` throughout, never `.expect()`/`.unwrap()`.

- **`muda` 0.19.3** (crates.io `max_stable_version`). Confirmed by `cargo check`
  and `cargo test` on a standalone crate reproducing the exact menu.rs shape below
  (all green, zero clippy warnings under `-D warnings`):
  - `Menu::new() -> Self`, `Menu::with_items(&[&dyn IsMenuItem]) -> muda::Result<Self>`.
  - `Submenu::with_items(text: impl AsRef<str>, enabled: bool, items: &[&dyn IsMenuItem]) -> muda::Result<Self>`.
  - `MenuItem::with_id(id: impl Into<MenuId>, text: impl AsRef<str>, enabled: bool, accelerator: Option<Accelerator>) -> Self`.
  - `PredefinedMenuItem::about(text: Option<&str>, metadata: Option<AboutMetadata>) -> PredefinedMenuItem`,
    `PredefinedMenuItem::separator() -> PredefinedMenuItem`, `PredefinedMenuItem::quit(text: Option<&str>) -> PredefinedMenuItem`.
  - `Menu::init_for_nsapp(&self)` is `#[cfg(target_os = "macos")]`, **not** `unsafe`,
    takes no extra arguments (source: `muda-0.19.3/src/menu.rs:414`).
  - `MenuEvent { pub id: MenuId }`, `MenuEvent::receiver() -> &'static MenuEventReceiver`
    (a `crossbeam_channel::Receiver<MenuEvent>`); draining pattern is
    `while let Ok(event) = MenuEvent::receiver().try_recv() { .. }` — non-blocking.
  - `MenuId(pub String)` derives `PartialEq, Eq`, so `&MenuId == &MenuId` works directly
    (no need for the `PartialEq<&str>`/`PartialEq<String>` convenience impls muda also provides).
  - `muda::Error` / `muda::Result<T>` are re-exported at the crate root
    (`pub use error::*;` in `lib.rs`) — `Submenu`/`Menu` builder methods already
    return `muda::Result<Self>`, so `build_app_menu() -> Result<AppMenu, muda::Error>`
    can `?`-propagate them with no `.map_err`.
  - `AboutMetadata` (source: `muda-0.19.3/src/about_metadata.rs`) is a plain public
    struct (not just a builder-only opaque type): `name`, `version`, `short_version`,
    `authors: Option<Vec<String>>`, `comments`, `copyright`, `license`, `website`,
    `website_label`, `credits`, `icon` — all `Option<...>`, derives `Default`. **On
    macOS specifically** (verified against `muda-0.19.3/src/platform_impl/macos/mod.rs:1060-1103`):
    `name` → `NSAboutPanelOptionApplicationName`, `version` → `NSAboutPanelOptionApplicationVersion`,
    `short_version` → `NSAboutPanelOptionVersion` (i.e. it renders as its own "Version"
    line distinct from `version` — this is exactly the "build id" slot the design
    wants), `copyright` → the `"Copyright"` key, `icon` → `NSAboutPanelOptionApplicationIcon`,
    `credits` → `NSAboutPanelOptionCredits`. `authors`/`comments`/`license`/`website`/`website_label`
    are silently no-ops on macOS (not passed to the native panel at all — this is
    documented on each field and confirmed in the platform impl). This does **not**
    change what to build: the task's contract is to construct the full cross-platform
    `AboutMetadata` and unit-test its Rust-level field values, independent of which
    subset macOS's native panel happens to render.
  - Default features (`gtk`, `libxdo`) are wired through `[target.'cfg(...linux...)'.dependencies]`
    in muda's own `Cargo.toml`, so depending on plain `muda = "0.19.3"` (no feature
    tweaks) compiles cleanly on macOS without pulling in any GTK toolchain — confirmed
    by the `cargo check`/`cargo test` PoC run on this machine.
  - **`dead_code` note (verified with a minimal repro under this workspace's exact
    `-D warnings` clippy invocation):** a struct field that is only ever *written*,
    never read (e.g. `AppMenu.menu` after `init_for_nsapp` has installed it — see
    below — and `GemelliApp.licenses_open` in this task, since its reader is a later
    task), trips `error: field \`x\` is never read` under `cargo clippy --all-targets
    -- -D warnings`, and a plain `#[allow(dead_code)]` on the field silences it. This
    differs from the `#[cfg_attr(not(test), allow(dead_code))]` pattern already used
    in `crates/gui/src/theme.rs` — that pattern is for functions that *are* exercised
    by `#[cfg(test)]` tests but not by production code; `AppMenu.menu` and
    `GemelliApp.licenses_open` are read by **neither**, so they need the unconditional
    form.

---

### Task 2: embed git build id via vergen-gix (`crates/gui/build.rs`)

**Files:**
- Modify: `/Users/napochaan/ghq/github.com/naporin0624/web-cam-sharedtexture/Cargo.toml` (add `[workspace.dependencies]` entries)
- Modify: `/Users/napochaan/ghq/github.com/naporin0624/web-cam-sharedtexture/crates/gui/Cargo.toml` (add `[build-dependencies]`)
- Modify: `/Users/napochaan/ghq/github.com/naporin0624/web-cam-sharedtexture/crates/gui/build.rs` (extend `run()`; existing rpath logic preserved verbatim, just moved into its own function)

**Interfaces:**
- Consumes: `DEP_SYPHON_BRIDGE_RPATH` env var (unchanged, existing).
- Produces: compile-time env vars `VERGEN_GIT_SHA` (short SHA, e.g. `54ee65c` — **may be absent** when git info can't be read) and `VERGEN_BUILD_DATE` (e.g. `2026-07-08` — always present). Any module in `gemelli-gui` reads them via `option_env!("VERGEN_GIT_SHA")` / `option_env!("VERGEN_BUILD_DATE")`. Task 3's `menu::build_id()` is the first consumer.

- [ ] **Step 1: (RED) confirm the build currently emits neither var**

  Before touching anything, prove the starting state — no `VERGEN_*` instructions exist yet:

  ```
  cd /Users/napochaan/ghq/github.com/naporin0624/web-cam-sharedtexture
  cargo build -p gemelli-gui -vv 2>&1 | grep -c "cargo:rustc-env=VERGEN"
  ```

  Expected output: `0` (grep finds nothing; with `grep -c` and no match this prints `0` and exits 1 — that non-zero exit is expected here and is the RED signal, not a tooling failure).

- [ ] **Step 2: add `vergen-gix` to workspace + build-dependencies**

  Edit `Cargo.toml` (workspace root) — add these two lines inside the existing `[workspace.dependencies]` table (after `arc-swap`):

  ```toml
  [workspace.dependencies]
  thiserror = "2"
  clap = { version = "4.6", features = ["derive"] }
  ctrlc = "3.5"
  eframe = "0.35"
  egui = "0.35"
  arc-swap = "1.9"
  vergen-gix = { version = "10.0.1", features = ["build"] }
  muda = "0.19.3"
  ```

  (`muda`'s workspace-dependency entry is added here too since Task 3 needs it —
  adding both lines in one edit to this shared file avoids touching it twice. No
  crate consumes `muda` yet; that starts in Task 3 Step 0, which adds the actual
  `muda = { workspace = true }` line to `crates/gui/Cargo.toml`'s `[dependencies]`.)

  Edit `crates/gui/Cargo.toml` — add a `[build-dependencies]` table (new, after `[dependencies]`; `[dependencies]` itself is unchanged in this task):

  ```toml
  [package]
  name = "gemelli-gui"
  version = "0.1.0"
  edition.workspace = true
  license.workspace = true
  repository.workspace = true

  [lints]
  workspace = true

  [dependencies]
  gemelli-core = { path = "../core" }
  eframe = { workspace = true }
  egui = { workspace = true }
  thiserror = { workspace = true }
  arc-swap = { workspace = true }
  # Platform gating lives inside gemelli-syphon itself (crate-wide
  # `#![cfg(target_os = "macos")]`), not in a `[target.'cfg(...)'.dependencies]`
  # table here: release-please's Rust manifest updater cannot parse cfg()
  # target tables and fails to bump this crate's version.
  gemelli-syphon = { path = "../syphon" }

  [build-dependencies]
  vergen-gix = { workspace = true }
  ```

  Run:

  ```
  cargo build -p gemelli-gui -vv 2>&1 | grep -c "cargo:rustc-env=VERGEN"
  ```

  Expected output: still `0` — `build.rs` hasn't been touched yet, so the new
  dependency is fetched/compiled but not yet used. This step is still RED; it just
  isolates "does the dependency resolve/compile" from "does build.rs use it".

- [ ] **Step 3: (GREEN) extend `build.rs` to emit the build id**

  Replace the full contents of `crates/gui/build.rs` with:

  ```rust
  use std::process::ExitCode;

  use vergen_gix::{Build, Emitter, Gix};

  fn main() -> ExitCode {
      match run() {
          Ok(()) => ExitCode::SUCCESS,
          Err(reason) => {
              eprintln!("crates/gui build.rs failed: {reason}");
              ExitCode::FAILURE
          }
      }
  }

  fn run() -> Result<(), String> {
      emit_build_id()?;
      emit_syphon_rpath()
  }

  /// Embeds a short git SHA (`VERGEN_GIT_SHA`) and the build date
  /// (`VERGEN_BUILD_DATE`) as compile-time env vars, for `menu::about_metadata`
  /// (Task 3) to read back via `option_env!` and show in the native About panel.
  ///
  /// Deliberately uses vergen-gix's *default* `Emitter` — no `.fail_on_error()`,
  /// `.idempotent()`, or `.default_on_error()`. Verified empirically: when git info
  /// is unavailable (e.g. building from a source tarball with no `.git` directory),
  /// `add_instructions`/`emit` still return `Ok` — they leave `VERGEN_GIT_SHA` unset
  /// and print a `cargo:warning`, they do not fail the build.
  /// `option_env!("VERGEN_GIT_SHA").unwrap_or("unknown")` on the consumer side is
  /// what turns "unset" into a displayable fallback; `VERGEN_BUILD_DATE` has no such
  /// gap since it comes from the local clock, not git, so it is always emitted.
  fn emit_build_id() -> Result<(), String> {
      let gix = Gix::builder().sha(true).build();
      let build = Build::builder().build_date(true).build();

      Emitter::default()
          .add_instructions(&gix)
          .map_err(|reason| reason.to_string())?
          .add_instructions(&build)
          .map_err(|reason| reason.to_string())?
          .emit()
          .map_err(|reason| reason.to_string())?;

      Ok(())
  }

  // `crates/syphon/build.rs` emits the `-rpath` linker args needed to find the
  // vendored Syphon.framework at runtime, but Cargo's `rustc-link-arg`
  // instruction only applies to the emitting package's own targets — it does
  // not propagate to downstream binaries that merely depend on that crate
  // (unlike `rustc-link-lib`/`rustc-link-search`, which do propagate). Since
  // this crate's binary links `gemelli-syphon` on macOS, it needs
  // the same rpath entries itself or `@rpath/Syphon.framework/...` cannot be
  // resolved at process launch.
  //
  // Rather than duplicating the rpath list here, read it back from syphon's
  // `links = "syphon_bridge"` build-script metadata (published as
  // `cargo::metadata=rpath=...` in crates/syphon/build.rs) via the
  // `DEP_SYPHON_BRIDGE_RPATH` env var Cargo derives from it. This var is only
  // set when the syphon crate is an active dependency (macOS targets), so its
  // absence on other platforms is expected and not an error.
  fn emit_syphon_rpath() -> Result<(), String> {
      let Ok(rpaths) = std::env::var("DEP_SYPHON_BRIDGE_RPATH") else {
          return Ok(());
      };

      for rel in rpaths.split(';').filter(|rel| !rel.is_empty()) {
          println!("cargo:rustc-link-arg=-Wl,-rpath,{rel}");
      }

      Ok(())
  }
  ```

  Run:

  ```
  touch crates/gui/build.rs   # force a rerun even if cargo thinks it's unchanged
  cargo build -p gemelli-gui -vv 2>&1 | grep "cargo:rustc-env=VERGEN"
  ```

  Expected output (two lines, prefixed by cargo with the package name/version;
  exact SHA/date will differ per commit/day):

  ```
  [gemelli-gui 0.1.0] cargo:rustc-env=VERGEN_BUILD_DATE=2026-07-08
  [gemelli-gui 0.1.0] cargo:rustc-env=VERGEN_GIT_SHA=<7-char-short-sha>
  ```

- [ ] **Step 4: regression-check the existing rpath logic is untouched**

  ```
  cargo build -p gemelli-gui -vv 2>&1 | grep "rustc-link-arg=-Wl,-rpath"
  ```

  Expected output: the same `-rpath` lines this build already produced before this
  task (one per entry in `DEP_SYPHON_BRIDGE_RPATH`) — unchanged in content, only
  possibly reordered relative to the new `VERGEN_*` lines since `emit_build_id()`
  now runs first in `run()`.

- [ ] **Step 5: lint, full test, commit**

  ```
  cargo fmt --all
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  ```

  Expected: all three exit 0, `cargo test --workspace` still reports the same
  passing-test count as immediately before this task (no test files touched yet —
  the absolute number depends on Task 1 having landed).

  ```
  git add Cargo.toml crates/gui/Cargo.toml crates/gui/build.rs
  git commit -m "feat(gui): embed git build id via vergen-gix"
  ```

---

### Task 3: native app menu with About via muda (`crates/gui/src/menu.rs` + `app.rs` wiring)

**Files:**
- Create: `/Users/napochaan/ghq/github.com/naporin0624/web-cam-sharedtexture/crates/gui/src/menu.rs`
- Modify: `/Users/napochaan/ghq/github.com/naporin0624/web-cam-sharedtexture/crates/gui/src/main.rs` (add `mod menu;`, alphabetically between `mod fps_meter;` and `mod preview;`)
- Modify: `/Users/napochaan/ghq/github.com/naporin0624/web-cam-sharedtexture/crates/gui/src/app.rs` (new fields on `GemelliApp` struct ~line 76-111, construction ~line 125-149, new `poll_menu_actions` method, call site in `ui()` ~line 471-472)
- Modify: `/Users/napochaan/ghq/github.com/naporin0624/web-cam-sharedtexture/crates/gui/Cargo.toml` (add `muda = { workspace = true }` to `[dependencies]` — Step 0 below)

**Interfaces:**
- Consumes: `option_env!("VERGEN_GIT_SHA")` (Task 2's build.rs); `muda::{AboutMetadata, Menu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu}`.
- **Produces** (exact names, relied on by later tasks — Task 7's licenses viewport consumes `GemelliApp.licenses_open`):
  - `pub enum MenuAction { OpenLicenses }`
  - `pub struct AppMenu` (opaque; holds the muda `Menu` + the licenses `MenuId`)
  - `pub fn build_app_menu() -> Result<AppMenu, muda::Error>`
  - `impl AppMenu { pub fn poll_actions(&self) -> Vec<MenuAction> }`
  - On `GemelliApp`: private field `menu: Option<crate::menu::AppMenu>`, private field `licenses_open: bool` (write-only in this task; Task 7 reads it to open the licenses viewport).

- [ ] **Step 0: add `muda` to `crates/gui/Cargo.toml`**

  Edit `crates/gui/Cargo.toml`'s `[dependencies]` table (add the `muda` line after
  `gemelli-syphon`; leave `[build-dependencies]` from Task 2 untouched):

  ```toml
  [dependencies]
  gemelli-core = { path = "../core" }
  eframe = { workspace = true }
  egui = { workspace = true }
  thiserror = { workspace = true }
  arc-swap = { workspace = true }
  # Platform gating lives inside gemelli-syphon itself (crate-wide
  # `#![cfg(target_os = "macos")]`), not in a `[target.'cfg(...)'.dependencies]`
  # table here: release-please's Rust manifest updater cannot parse cfg()
  # target tables and fails to bump this crate's version.
  gemelli-syphon = { path = "../syphon" }
  muda = { workspace = true }

  [build-dependencies]
  vergen-gix = { workspace = true }
  ```

  Run:

  ```
  cargo build -p gemelli-gui
  ```

  Expected: succeeds (fetches/compiles `muda` and its macOS `objc2`/`objc2-app-kit`
  dependencies; nothing references `muda` from Rust code yet, so no new warnings).

- [ ] **Step 1: (RED) write the `about_metadata` test against a function that doesn't exist yet**

  Create `crates/gui/src/menu.rs` with only this much content:

  ```rust
  //! Native application menu (macOS: `gemelli ▸ About / Quit`, `Help ▸ Open Source
  //! Licenses…`) built with `muda`.
  //!
  //! `muda` is a normal (not platform-gated) dependency, so this module compiles on
  //! every target; only the `Menu::init_for_nsapp` call inside `build_app_menu` —
  //! which installs the menu as the app's NSApp main menu — is macOS-only.

  #[cfg(test)]
  mod tests {
      use super::about_metadata;

      #[test]
      fn about_metadata_has_the_expected_fields() {
          let metadata = about_metadata();

          assert_eq!(metadata.name, Some("gemelli".to_string()));
          assert_eq!(metadata.version, Some(env!("CARGO_PKG_VERSION").to_string()));
          assert_eq!(metadata.authors, Some(vec!["naporitan".to_string()]));
          assert_eq!(metadata.copyright, Some("\u{a9} 2026 naporitan".to_string()));
          assert_eq!(metadata.website, Some("https://napochaan.com".to_string()));
          assert!(metadata.short_version.is_some());
      }
  }
  ```

  Add `mod menu;` to `crates/gui/src/main.rs` (keeping the existing alphabetical
  order: `app`, `crop_editor`, `fonts`, `fps_meter`, `menu`, `preview`, `sidebar`,
  `theme`, `worker`).

  Run:

  ```
  cargo test -p gemelli-gui menu
  ```

  Expected output: a compile error (RED) — `about_metadata` doesn't exist yet:

  ```
  error[E0425]: cannot find function `about_metadata` in module `menu`
  ```

- [ ] **Step 2: (GREEN) implement `about_metadata`**

  Add to the top of `crates/gui/src/menu.rs` (above the `#[cfg(test)]` block):

  ```rust
  use muda::AboutMetadata;

  /// Short git SHA embedded by `build.rs` via vergen-gix, or `"unknown"` when it
  /// could not be determined at build time (e.g. building from a source tarball
  /// with no `.git` directory — `build.rs`'s `emit_build_id` does not fail the
  /// build in that case, it just leaves the var unset).
  fn build_id() -> &'static str {
      option_env!("VERGEN_GIT_SHA").unwrap_or("unknown")
  }

  /// Assembles this app's `AboutMetadata`. Kept as a pure function (no globals, no
  /// I/O beyond reading compile-time env vars) so its contents are unit-testable
  /// without constructing a real menu.
  fn about_metadata() -> AboutMetadata {
      AboutMetadata {
          name: Some("gemelli".to_string()),
          version: Some(env!("CARGO_PKG_VERSION").to_string()),
          short_version: Some(build_id().to_string()),
          authors: Some(vec!["naporitan".to_string()]),
          copyright: Some("\u{a9} 2026 naporitan".to_string()),
          website: Some("https://napochaan.com".to_string()),
          ..Default::default()
      }
  }
  ```

  Run:

  ```
  cargo test -p gemelli-gui menu
  ```

  Expected output:

  ```
  running 1 test
  test menu::tests::about_metadata_has_the_expected_fields ... ok

  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
  ```

- [ ] **Step 3: (RED) write the `action_for` tests against a function/enum that don't exist yet**

  Extend the `#[cfg(test)] mod tests` block in `crates/gui/src/menu.rs`:

  ```rust
  #[cfg(test)]
  mod tests {
      use super::{about_metadata, action_for, MenuAction};
      use muda::MenuId;

      #[test]
      fn about_metadata_has_the_expected_fields() {
          let metadata = about_metadata();

          assert_eq!(metadata.name, Some("gemelli".to_string()));
          assert_eq!(metadata.version, Some(env!("CARGO_PKG_VERSION").to_string()));
          assert_eq!(metadata.authors, Some(vec!["naporitan".to_string()]));
          assert_eq!(metadata.copyright, Some("\u{a9} 2026 naporitan".to_string()));
          assert_eq!(metadata.website, Some("https://napochaan.com".to_string()));
          assert!(metadata.short_version.is_some());
      }

      #[test]
      fn action_for_maps_the_licenses_id_to_open_licenses() {
          let licenses_id = MenuId::new("gemelli-open-source-licenses");

          assert_eq!(action_for(&licenses_id, &licenses_id), Some(MenuAction::OpenLicenses));
      }

      #[test]
      fn action_for_ignores_ids_it_does_not_recognize() {
          let licenses_id = MenuId::new("gemelli-open-source-licenses");
          let other_id = MenuId::new("some-other-item");

          assert_eq!(action_for(&other_id, &licenses_id), None);
      }
  }
  ```

  Run:

  ```
  cargo test -p gemelli-gui menu
  ```

  Expected output: compile errors (RED) — `action_for` and `MenuAction` don't exist:

  ```
  error[E0425]: cannot find function `action_for` in module `menu`
  error[E0412]: cannot find type `MenuAction` in module `menu`
  ```

- [ ] **Step 4: (GREEN) implement `MenuAction` and `action_for`**

  Add to `crates/gui/src/menu.rs` (after the `AboutMetadata` imports, before `about_metadata`):

  ```rust
  use muda::MenuId;

  /// What the running app should do in response to a menu activation. `About` and
  /// `Quit` are native `PredefinedMenuItem`s handled entirely by the OS (or by muda
  /// itself for `Quit` on non-macOS platforms) — they never surface here.
  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum MenuAction {
      OpenLicenses,
  }
  ```

  And, after `about_metadata`:

  ```rust
  /// Maps a fired `MenuEvent`'s id to the `MenuAction` it represents. `None` covers
  /// ids muda already handled natively (About, Quit) or any id from a menu we
  /// didn't build.
  fn action_for(event_id: &MenuId, licenses_id: &MenuId) -> Option<MenuAction> {
      (event_id == licenses_id).then_some(MenuAction::OpenLicenses)
  }
  ```

  Run:

  ```
  cargo test -p gemelli-gui menu
  ```

  Expected output:

  ```
  running 3 tests
  test menu::tests::about_metadata_has_the_expected_fields ... ok
  test menu::tests::action_for_ignores_ids_it_does_not_recognize ... ok
  test menu::tests::action_for_maps_the_licenses_id_to_open_licenses ... ok

  test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
  ```

- [ ] **Step 5: implement `AppMenu` and `build_app_menu`**

  These are not covered by new unit tests — per the design's test table, only
  `AboutMetadata` assembly and `MenuId → MenuAction` mapping are asserted by
  automated tests; `build_app_menu`/`poll_actions` are exercised manually via
  `cargo run -p gemelli-gui` (they call into real OS menu APIs, which is what the
  PoC in the verification log above already smoke-tested in isolation). Add to
  `crates/gui/src/menu.rs` (after the `action_for` function):

  ```rust
  use muda::{Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};

  /// The app's native menu bar, plus the id needed to recognize its one custom item.
  pub struct AppMenu {
      // Never read again after `build_app_menu` installs it, but must stay alive
      // for the app's lifetime: dropping `Menu` frees its native (NSMenu on macOS)
      // backing storage, which would tear down the menu bar it was just installed
      // as. `#[allow(dead_code)]` is required here — this field is genuinely
      // write-only, in both the test and non-test build (see verification log).
      #[allow(dead_code)]
      menu: Menu,
      licenses_id: MenuId,
  }

  impl AppMenu {
      /// Drains every menu activation queued since the last poll and maps it to a
      /// `MenuAction`. Never blocks (`try_recv`) — safe to call once per frame.
      pub fn poll_actions(&self) -> Vec<MenuAction> {
          let mut actions = Vec::new();
          while let Ok(event) = MenuEvent::receiver().try_recv() {
              if let Some(action) = action_for(event.id(), &self.licenses_id) {
                  actions.push(action);
              }
          }
          actions
      }
  }

  /// Builds the `gemelli ▸ About / Quit` and `Help ▸ Open Source Licenses…` menu.
  ///
  /// On macOS this also installs it as the app's main menu (`init_for_nsapp`) —
  /// safe to call here because `build_app_menu` is only ever invoked from
  /// `GemelliApp::new`, which eframe calls after `NSApplication` already exists.
  pub fn build_app_menu() -> Result<AppMenu, muda::Error> {
      let licenses_id = MenuId::new("gemelli-open-source-licenses");
      let licenses_item =
          MenuItem::with_id(licenses_id.clone(), "Open Source Licenses\u{2026}", true, None);

      let app_submenu = Submenu::with_items(
          "gemelli",
          true,
          &[
              &PredefinedMenuItem::about(None, Some(about_metadata())),
              &PredefinedMenuItem::separator(),
              &PredefinedMenuItem::quit(None),
          ],
      )?;
      let help_submenu = Submenu::with_items("Help", true, &[&licenses_item])?;

      let menu = Menu::with_items(&[&app_submenu, &help_submenu])?;

      #[cfg(target_os = "macos")]
      menu.init_for_nsapp();

      Ok(AppMenu { menu, licenses_id })
  }
  ```

  Run:

  ```
  cargo test -p gemelli-gui menu
  cargo build -p gemelli-gui
  ```

  Expected: same 3 passing tests as Step 4 (unchanged), and the build succeeds
  with no errors.

- [ ] **Step 6: wire `AppMenu` into `GemelliApp`**

  In `crates/gui/src/app.rs`, add two fields to the `GemelliApp` struct (after
  `last_uploaded`, the last field before the closing `}` around line 110):

  ```rust
      /// `None` when `menu::build_app_menu()` failed at startup (see `GemelliApp::new`)
      /// — the app still runs, just without a menu bar.
      menu: Option<crate::menu::AppMenu>,
      /// Set by `poll_menu_actions` on `MenuAction::OpenLicenses`. Write-only in this
      /// task — the licenses viewport (a later task) reads it to decide whether to
      /// show/focus its window. `#[allow(dead_code)]` is required in the meantime
      /// (see verification log: this field is unread in both test and non-test
      /// builds until that task lands).
      #[allow(dead_code)]
      licenses_open: bool,
  ```

  In `GemelliApp::new` (currently lines 114-150), build the menu right after the
  `theme`/`fonts` setup and before `capture::list_devices()` (order doesn't matter
  functionally, but keeping menu setup near the top groups "one-time native setup"
  together):

  ```rust
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
  ```

  ...and add `menu` and `licenses_open: false` to the `Self { .. }` construction
  (alongside the other fields, e.g. right after `last_uploaded: None,`):

  ```rust
              last_uploaded: None,
              menu,
              licenses_open: false,
          }
      }
  ```

  Add a new private method (near `drain_errors`, since it has the same "single
  per-frame poll" shape):

  ```rust
      /// Drains this frame's menu activations and applies each one. Exhaustive
      /// match over `MenuAction` — a new variant added upstream forces this match
      /// to be revisited instead of silently no-op'ing.
      fn poll_menu_actions(&mut self) {
          let Some(menu) = &self.menu else { return };
          for action in menu.poll_actions() {
              match action {
                  crate::menu::MenuAction::OpenLicenses => self.licenses_open = true,
              }
          }
      }
  ```

  And call it from `eframe::App::ui`, alongside the existing `drain_errors()` call
  (currently line 472):

  ```rust
      fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
          self.drain_errors();
          self.poll_menu_actions();
          self.refresh_preview(ui.ctx());
  ```

  Run:

  ```
  cargo test -p gemelli-gui
  cargo clippy --workspace --all-targets -- -D warnings
  ```

  Expected: all existing `app.rs` tests (`flip_from_toggles_covers_all_four_combinations`,
  `build_transform_assembles_all_fields`, `build_transform_defaults_to_no_op`,
  `drain_stale_errors_empties_every_queued_message`, `drain_stale_errors_on_an_empty_channel_is_a_no_op`,
  `refit_crop_returns_none_when_the_rect_still_fits`, `refit_crop_returns_the_clamped_rect_when_the_new_frame_is_smaller`)
  plus the 3 new `menu::tests::*` still pass; clippy reports zero warnings (in
  particular: no `dead_code` on `menu`/`licenses_open`, since both `#[allow(dead_code)]`
  attributes are in place, and `menu` itself is read by `poll_menu_actions`).

- [ ] **Step 7: manual smoke check (not an automated test — exercises real OS menu APIs)**

  ```
  cargo run -p gemelli-gui
  ```

  On macOS, confirm in the running app: the menu bar shows a `gemelli` menu (About
  gemelli / separator / Quit gemelli) and a `Help` menu (Open Source Licenses…).
  Click "About gemelli" and confirm the native panel shows the app name, the
  `CARGO_PKG_VERSION` value, and a distinct "Version" line with the short git SHA
  (this is `short_version`, mapped to `NSAboutPanelOptionVersion` — see
  verification log). Clicking "Open Source Licenses…" does nothing yet (Task 7
  wires that up) but must not crash or panic the app.

- [ ] **Step 8: full workspace lint + test, commit**

  ```
  cargo fmt --all
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  ```

  Expected: all exit 0; test count increases by exactly 3 (the new `menu::tests::*`).

  ```
  git add crates/gui/Cargo.toml crates/gui/src/menu.rs crates/gui/src/main.rs crates/gui/src/app.rs
  git commit -m "feat(gui): add native app menu with About via muda"
  ```

---

## API verification notes (read before implementing)

- **cargo-bundle-licenses**: pinned to **v4.2.0** (crates.io `max_stable_version` = `4.2.0`,
  matches GitHub tag `v4.2.0`, published 2025-10-21). JSON schema verified by reading the
  actual source at `github.com/sstadick/cargo-bundle-licenses@main`:
  - `src/bundle.rs` → `struct Bundle { root_name: String, third_party_libraries: Vec<FinalizedLicense> }`
    (both fields `pub(crate)`, no `#[serde(rename)]`, so JSON keys are exactly `root_name` /
    `third_party_libraries`).
  - `src/finalized_license.rs` → `struct FinalizedLicense { package_name: String, package_version: String,
    #[serde(default)] repository: String, license: String, licenses: Vec<LicenseAndText> }` and
    `struct LicenseAndText { license: String, text: String }`.
  - `src/format.rs` → `Format::Json` calls `serde_json::to_string_pretty(&bundle)`, i.e. `cargo
    bundle-licenses --format json` prints the `Bundle` struct verbatim as pretty JSON to stdout
    (or to `--output <path>`, we use stdout via subprocess capture).
  - `src/main.rs` confirms the cargo subcommand invocation is `cargo bundle-licenses --format json`
    (the binary is named `cargo-bundle-licenses`; cargo strips the leading `bundle-licenses` arg).
  - Real example of the exact JSON shape this pipeline consumes:
    ```json
    {
      "root_name": "gemelli-gui, gemelli-cli",
      "third_party_libraries": [
        {
          "package_name": "eframe",
          "package_version": "0.35.0",
          "repository": "https://github.com/emilk/egui",
          "license": "MIT OR Apache-2.0",
          "licenses": [
            { "license": "MIT", "text": "…MIT license full text…" },
            { "license": "Apache-2.0", "text": "…Apache-2.0 full text…" }
          ]
        }
      ]
    }
    ```
- **xtask alias pattern**: no `.cargo/config.toml` exists yet in this repo (verified: directory
  absent). Standard `cargo-xtask` book pattern confirmed: `.cargo/config.toml` with
  `[alias]\nxtask = "run --package xtask --"`.
- **mise cargo backend**: confirmed via `mise.jdx.dev/dev-tools/backends/cargo.html` — a cargo
  crate is installed and pinned with `"cargo:<crate-name>" = "<version>"` under `[tools]`. We add
  `"cargo:cargo-bundle-licenses" = "4.2.0"` to `mise.toml` so `mise install` puts the pinned binary
  on `PATH` for every contributor and CI runner.
- **Test-code lint exemption (root cause verified, not assumed)**: the repo root carries a
  `clippy.toml` with `allow-unwrap-in-tests = true` and `allow-expect-in-tests = true` — this is
  what lets `crates/core/src/transform/scale.rs`'s `#[cfg(test)] mod tests` block call `.unwrap()`
  repeatedly (e.g. `Frame::new(2, 3, data).unwrap()`) with **no** local `#[allow(...)]` and still
  pass `cargo clippy -p gemelli-core --all-targets -- -D warnings` clean (verified by running it).
  Because this is a workspace-wide `clippy.toml` setting, not a per-crate opt-in, it applies to
  `crates/xtask` automatically — this plan's test code uses `.unwrap()`/`.expect()` freely inside
  `#[cfg(test)]` blocks, matching the established convention, with no test-only `#[allow]` needed.
- **`xtask` crate is a `--bin`-only crate with no other consumer.** Between Task 4 (pure functions
  exist but nothing calls them yet) and Task 5 (shell layer wires them into `gen-licenses`), the
  pure functions would be flagged `dead_code` by the `-D warnings` clippy gate the husky
  pre-commit hook runs on every commit. Task 4's stub `main.rs` carries a scoped, commented
  `#![allow(dead_code)]` that Task 5 deletes once the shell layer calls every function for real.

---

### Task 4: `crates/xtask` pure function layer (normalize / merge / sort / render), TDD

**Files:**
- Create: `crates/xtask/Cargo.toml`
- Create: `crates/xtask/src/main.rs` (stub only — shell layer replaces this in Task 5)
- Create: `crates/xtask/src/license_entry.rs`
- Create: `crates/xtask/src/normalize.rs`
- Create: `crates/xtask/src/merge.rs`
- Create: `crates/xtask/src/sort.rs`
- Create: `crates/xtask/src/render.rs`
- Modify: `Cargo.toml` (root) — add `crates/xtask` to `[workspace] members`, add `serde` /
  `serde_json` to `[workspace.dependencies]`
- Test: `#[cfg(test)] mod tests` inline in each of `license_entry.rs`, `normalize.rs`, `merge.rs`,
  `sort.rs`, `render.rs`, run via `cargo test -p xtask`

**Interfaces:**

Consumes (verified real schema, see notes above) — `cargo bundle-licenses --format json` stdout:
```json
{
  "root_name": "gemelli-gui, gemelli-cli",
  "third_party_libraries": [
    {
      "package_name": "eframe",
      "package_version": "0.35.0",
      "repository": "https://github.com/emilk/egui",
      "license": "MIT OR Apache-2.0",
      "licenses": [
        { "license": "MIT", "text": "…full text…" },
        { "license": "Apache-2.0", "text": "…full text…" }
      ]
    }
  ]
}
```

Produces — **this is the shared interface Task 6 (GUI) parses with serde; the schema is frozen
here**:
```json
[{"name": "eframe", "version": "0.35.0", "license": "MIT OR Apache-2.0", "text": "…full text…", "homepage": "https://github.com/emilk/egui", "category": "library"}]
```
```rust
// crates/xtask/src/license_entry.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LicenseCategory {
    Library,
    Font,
    Native,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LicenseEntry {
    pub name: String,
    pub version: Option<String>,
    pub license: String,
    pub text: String,
    pub homepage: Option<String>,
    pub category: LicenseCategory,
}
```
`version: Option<String>` and `homepage: Option<String>` are `null` for appendix (`font`/`native`)
entries. `sort.rs` relies on `LicenseCategory`'s **declared variant order** (`Library < Font <
Native`) for `#[derive(Ord)]` — this order is deliberate: it puts the bulk of entries (crate
dependencies) first and the two appendix entries last, matching the GUI's category filter order
in `crates/gui/src/licenses.rs` (`enum LicenseCategory { Library, Font, Native }`, spec §3).

---

- [ ] **Step 1: scaffold `crates/xtask` and register it as a workspace member**

  `crates/xtask/Cargo.toml`:
  ```toml
  [package]
  name = "xtask"
  version = "0.1.0"
  edition.workspace = true
  license.workspace = true
  repository.workspace = true
  publish = false

  [lints]
  workspace = true

  [dependencies]
  serde = { workspace = true }
  serde_json = { workspace = true }
  ```

  Modify root `Cargo.toml`:
  ```toml
  [workspace]
  resolver = "2"
  members = ["crates/core", "crates/cli", "crates/gui", "crates/syphon", "crates/xtask"]

  [workspace.package]
  edition = "2024"
  license = "MIT"
  repository = "https://github.com/naporin0624/web-cam-sharedtexture"

  [workspace.dependencies]
  thiserror = "2"
  clap = { version = "4.6", features = ["derive"] }
  ctrlc = "3.5"
  eframe = "0.35"
  egui = "0.35"
  arc-swap = "1.9"
  vergen-gix = { version = "10.0.1", features = ["build"] }
  muda = "0.19.3"
  serde = { version = "1", features = ["derive"] }
  serde_json = "1"

  [workspace.lints.clippy]
  unwrap_used = "deny"
  expect_used = "deny"
  as_conversions = "deny"
  ```

  `crates/xtask/src/main.rs` (stub — Task 5 replaces this body):
  ```rust
  // Shell layer (CLI parsing, `cargo bundle-licenses` subprocess, file IO) lands in
  // Task 5 and calls every pure function below from `gen-licenses`. Until then this
  // stub only exists so `cargo test -p xtask` can compile and run the unit tests in
  // each module; `dead_code` is allowed here and removed once Task 5 wires them in.
  #![allow(dead_code)]

  mod license_entry;
  mod merge;
  mod normalize;
  mod render;
  mod sort;

  fn main() {}
  ```

  Run:
  ```
  cargo test -p xtask
  ```
  Expected: compiles, `running 0 tests ... ok` (no tests exist yet).

- [ ] **Step 2: `license_entry.rs` — shared schema types (TDD the serde wire format)**

  Failing test first — create `crates/xtask/src/license_entry.rs` with only the test module (no
  types yet), run `cargo test -p xtask`, expect a compile error (`cannot find type
  LicenseCategory`). Then add the full file:

  ```rust
  use serde::{Deserialize, Serialize};

  /// Wire format shared with `crates/gui/src/licenses.rs::LicenseCategory` (spec §3). Variant
  /// declaration order is load-bearing: `sort.rs` derives `Ord` from it so Library entries sort
  /// before the Font/Native appendix entries.
  #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
  #[serde(rename_all = "lowercase")]
  pub enum LicenseCategory {
      Library,
      Font,
      Native,
  }

  /// One row in `crates/gui/assets/third-party-licenses.json`. `version`/`homepage` are `None`
  /// for the hand-written appendix entries (Syphon Framework, LINE Seed JP).
  #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
  pub struct LicenseEntry {
      pub name: String,
      pub version: Option<String>,
      pub license: String,
      pub text: String,
      pub homepage: Option<String>,
      pub category: LicenseCategory,
  }

  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn category_serializes_to_lowercase_spdx_style_tag() {
          assert_eq!(serde_json::to_string(&LicenseCategory::Library).unwrap(), "\"library\"");
          assert_eq!(serde_json::to_string(&LicenseCategory::Font).unwrap(), "\"font\"");
          assert_eq!(serde_json::to_string(&LicenseCategory::Native).unwrap(), "\"native\"");
      }

      #[test]
      fn category_declared_order_is_library_then_font_then_native() {
          assert!(LicenseCategory::Library < LicenseCategory::Font);
          assert!(LicenseCategory::Font < LicenseCategory::Native);
      }

      #[test]
      fn entry_round_trips_through_json_with_null_optionals() {
          let entry = LicenseEntry {
              name: "LINE Seed JP".to_string(),
              version: None,
              license: "OFL-1.1".to_string(),
              text: "full text".to_string(),
              homepage: Some("https://seed.line.me/".to_string()),
              category: LicenseCategory::Font,
          };
          let json = serde_json::to_string(&entry).unwrap();
          assert!(json.contains("\"version\":null"));
          let round_tripped: LicenseEntry = serde_json::from_str(&json).unwrap();
          assert_eq!(round_tripped, entry);
      }
  }
  ```

  Run:
  ```
  cargo test -p xtask
  ```
  Expected: `test license_entry::tests::category_serializes_to_lowercase_spdx_style_tag ... ok`,
  `test license_entry::tests::category_declared_order_is_library_then_font_then_native ... ok`,
  `test license_entry::tests::entry_round_trips_through_json_with_null_optionals ... ok`,
  `3 passed`.

- [ ] **Step 3: `normalize.rs` — cargo-bundle-licenses JSON → `Vec<LicenseEntry>` (TDD)**

  Design decision (Cannelloni precedent, per task brief): **one `LicenseEntry` per package**, not
  per SPDX component. When a package has multiple `licenses[]` entries (e.g. dual `MIT OR
  Apache-2.0`), concatenate each component's full text under a `### <spdx>` heading, joined by a
  `\n\n---\n\n` separator, into a single `text` field; `license` stays the combined SPDX string
  from `FinalizedLicense.license`. `repository` (empty string when absent, matching the upstream
  `#[serde(default)]`) becomes `homepage: None` when empty, `Some(..)` otherwise. `category` is
  always `Library` — appendix entries are added later by `merge.rs`.

  Write the test first (fails to compile: `normalize`/`CargoBundleOutput` don't exist yet), then
  add the full file:

  ```rust
  use serde::Deserialize;

  use crate::license_entry::{LicenseCategory, LicenseEntry};

  /// Mirrors the JSON emitted by `cargo bundle-licenses --format json` (verified against
  /// cargo-bundle-licenses v4.2.0's `src/bundle.rs::Bundle` and
  /// `src/finalized_license.rs::FinalizedLicense`/`LicenseAndText`). `root_name` is intentionally
  /// omitted from this struct: serde ignores unknown JSON fields by default, and we don't need it.
  #[derive(Debug, Deserialize)]
  pub struct CargoBundleOutput {
      pub third_party_libraries: Vec<CargoBundleLicense>,
  }

  #[derive(Debug, Deserialize)]
  pub struct CargoBundleLicense {
      pub package_name: String,
      pub package_version: String,
      #[serde(default)]
      pub repository: String,
      pub license: String,
      pub licenses: Vec<CargoBundleLicenseText>,
  }

  #[derive(Debug, Deserialize)]
  pub struct CargoBundleLicenseText {
      pub license: String,
      pub text: String,
  }

  pub fn normalize(bundle: CargoBundleOutput) -> Vec<LicenseEntry> {
      bundle.third_party_libraries.into_iter().map(normalize_one).collect()
  }

  fn normalize_one(lib: CargoBundleLicense) -> LicenseEntry {
      let text = lib
          .licenses
          .iter()
          .map(|component| format!("### {}\n\n{}", component.license, component.text))
          .collect::<Vec<_>>()
          .join("\n\n---\n\n");
      let homepage = if lib.repository.is_empty() { None } else { Some(lib.repository) };

      LicenseEntry {
          name: lib.package_name,
          version: Some(lib.package_version),
          license: lib.license,
          text,
          homepage,
          category: LicenseCategory::Library,
      }
  }

  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn normalizes_dual_license_package_and_joins_repository_as_homepage() {
          let raw = r#"{
              "root_name": "gemelli-gui",
              "third_party_libraries": [
                  {
                      "package_name": "eframe",
                      "package_version": "0.35.0",
                      "repository": "https://github.com/emilk/egui",
                      "license": "MIT OR Apache-2.0",
                      "licenses": [
                          {"license": "MIT", "text": "MIT license text"},
                          {"license": "Apache-2.0", "text": "Apache license text"}
                      ]
                  },
                  {
                      "package_name": "no-repo-crate",
                      "package_version": "1.0.0",
                      "repository": "",
                      "license": "MIT",
                      "licenses": [
                          {"license": "MIT", "text": "MIT license text"}
                      ]
                  }
              ]
          }"#;
          let bundle: CargoBundleOutput = serde_json::from_str(raw).unwrap();

          let entries = normalize(bundle);

          assert_eq!(entries.len(), 2);
          assert_eq!(entries[0].name, "eframe");
          assert_eq!(entries[0].version, Some("0.35.0".to_string()));
          assert_eq!(entries[0].license, "MIT OR Apache-2.0");
          assert_eq!(entries[0].homepage, Some("https://github.com/emilk/egui".to_string()));
          assert_eq!(entries[0].category, LicenseCategory::Library);
          assert_eq!(
              entries[0].text,
              "### MIT\n\nMIT license text\n\n---\n\n### Apache-2.0\n\nApache license text"
          );

          assert_eq!(entries[1].name, "no-repo-crate");
          assert_eq!(entries[1].homepage, None);
          assert_eq!(entries[1].text, "### MIT\n\nMIT license text");
      }
  }
  ```

  Run:
  ```
  cargo test -p xtask
  ```
  Expected: `test normalize::tests::normalizes_dual_license_package_and_joins_repository_as_homepage ... ok`.

- [ ] **Step 4: `merge.rs` — scanner entries + appendix entries, appendix wins on name collision (TDD)**

  ```rust
  use std::collections::HashSet;

  use crate::license_entry::LicenseEntry;

  /// Merges scanner (`cargo bundle-licenses`) output with the hand-written appendix. On a name
  /// collision the appendix entry wins and the scanner's is dropped — the appendix exists
  /// specifically to override/supply entries the crate scanner cannot know about (Syphon
  /// Framework, LINE Seed JP), so it must never be shadowed by a same-named crate dependency.
  pub fn merge(scanner: Vec<LicenseEntry>, appendix: Vec<LicenseEntry>) -> Vec<LicenseEntry> {
      let appendix_names: HashSet<&str> = appendix.iter().map(|entry| entry.name.as_str()).collect();
      let mut merged: Vec<LicenseEntry> =
          scanner.into_iter().filter(|entry| !appendix_names.contains(entry.name.as_str())).collect();
      merged.extend(appendix);
      merged
  }

  #[cfg(test)]
  mod tests {
      use super::*;
      use crate::license_entry::LicenseCategory;

      fn entry(name: &str, category: LicenseCategory) -> LicenseEntry {
          LicenseEntry {
              name: name.to_string(),
              version: None,
              license: "MIT".to_string(),
              text: "text".to_string(),
              homepage: None,
              category,
          }
      }

      #[test]
      fn appendix_entry_replaces_scanner_entry_with_same_name() {
          let scanner = vec![
              entry("eframe", LicenseCategory::Library),
              entry("Syphon Framework", LicenseCategory::Library),
          ];
          let appendix = vec![entry("Syphon Framework", LicenseCategory::Native)];

          let merged = merge(scanner, appendix);

          assert_eq!(merged.len(), 2);
          let syphon = merged.iter().find(|e| e.name == "Syphon Framework").unwrap();
          assert_eq!(syphon.category, LicenseCategory::Native);
          assert!(merged.iter().any(|e| e.name == "eframe"));
      }

      #[test]
      fn no_collision_keeps_every_entry() {
          let scanner = vec![entry("eframe", LicenseCategory::Library)];
          let appendix = vec![entry("Syphon Framework", LicenseCategory::Native)];

          let merged = merge(scanner, appendix);

          assert_eq!(merged.len(), 2);
      }
  }
  ```

  Run:
  ```
  cargo test -p xtask
  ```
  Expected: `test merge::tests::appendix_entry_replaces_scanner_entry_with_same_name ... ok`,
  `test merge::tests::no_collision_keeps_every_entry ... ok`.

- [ ] **Step 5: `sort.rs` — stable sort by `(category, name)` (TDD)**

  ```rust
  use crate::license_entry::LicenseEntry;

  /// Stable sort by `(category, name)`. `Vec::sort_by` is documented-stable in std, which is what
  /// gives this function its "安定ソート" guarantee — no extra bookkeeping needed.
  pub fn sort_entries(mut entries: Vec<LicenseEntry>) -> Vec<LicenseEntry> {
      entries.sort_by(|a, b| (a.category, &a.name).cmp(&(b.category, &b.name)));
      entries
  }

  #[cfg(test)]
  mod tests {
      use super::*;
      use crate::license_entry::LicenseCategory;

      fn entry(name: &str, category: LicenseCategory) -> LicenseEntry {
          LicenseEntry {
              name: name.to_string(),
              version: None,
              license: "MIT".to_string(),
              text: "text".to_string(),
              homepage: None,
              category,
          }
      }

      #[test]
      fn sorts_by_category_then_name_with_declared_category_order() {
          let entries = vec![
              entry("zeta", LicenseCategory::Native),
              entry("alpha", LicenseCategory::Library),
              entry("beta", LicenseCategory::Font),
              entry("omega", LicenseCategory::Library),
          ];

          let sorted = sort_entries(entries);

          let order: Vec<(&str, LicenseCategory)> =
              sorted.iter().map(|e| (e.name.as_str(), e.category)).collect();
          assert_eq!(
              order,
              vec![
                  ("alpha", LicenseCategory::Library),
                  ("omega", LicenseCategory::Library),
                  ("beta", LicenseCategory::Font),
                  ("zeta", LicenseCategory::Native),
              ]
          );
      }

      #[test]
      fn stable_sort_preserves_relative_order_for_equal_keys() {
          let mut first = entry("dup", LicenseCategory::Library);
          first.version = Some("1.0.0".to_string());
          let mut second = entry("dup", LicenseCategory::Library);
          second.version = Some("2.0.0".to_string());

          let sorted = sort_entries(vec![first.clone(), second.clone()]);

          assert_eq!(sorted[0].version, first.version);
          assert_eq!(sorted[1].version, second.version);
      }
  }
  ```

  Run:
  ```
  cargo test -p xtask
  ```
  Expected: `test sort::tests::sorts_by_category_then_name_with_declared_category_order ... ok`,
  `test sort::tests::stable_sort_preserves_relative_order_for_equal_keys ... ok`.

- [ ] **Step 6: `render.rs` — `Vec<LicenseEntry>` → `THIRD-PARTY-NOTICES` text (TDD)**

  Format decision: header + `====` (80 `=`) separators mirroring the existing hand-written file
  (verified against the committed `THIRD-PARTY-NOTICES`). Unlike the hand-written file, every
  entry now also gets an explicit `<name> [<version>]` line and an explicit SPDX `license` line —
  the file will carry hundreds of crate entries, so a machine-identifiable license ident per entry
  matters more here than it did for the original two hand-written blocks.

  ```rust
  use crate::license_entry::LicenseEntry;

  const SEPARATOR: &str = "================================================================================";

  pub fn render_notices(entries: &[LicenseEntry]) -> String {
      let mut out = String::new();
      out.push_str("THIRD-PARTY SOFTWARE NOTICES AND INFORMATION\n\n");
      out.push_str("This package incorporates components from the projects listed below.\n");

      for entry in entries {
          out.push('\n');
          out.push_str(SEPARATOR);
          out.push_str("\n\n");
          match &entry.version {
              Some(version) => out.push_str(&format!("{} {}\n", entry.name, version)),
              None => out.push_str(&format!("{}\n", entry.name)),
          }
          out.push_str(&entry.license);
          out.push('\n');
          if let Some(homepage) = &entry.homepage {
              out.push_str(homepage);
              out.push('\n');
          }
          out.push('\n');
          out.push_str(&entry.text);
          out.push('\n');
      }

      out
  }

  #[cfg(test)]
  mod tests {
      use super::*;
      use crate::license_entry::LicenseCategory;

      #[test]
      fn renders_header_and_entries_with_separators() {
          let entries = vec![
              LicenseEntry {
                  name: "libfoo".to_string(),
                  version: Some("1.0.0".to_string()),
                  license: "MIT".to_string(),
                  text: "MIT TEXT".to_string(),
                  homepage: Some("https://example.com/libfoo".to_string()),
                  category: LicenseCategory::Library,
              },
              LicenseEntry {
                  name: "Syphon Framework".to_string(),
                  version: None,
                  license: "BSD-3-Clause".to_string(),
                  text: "BSD TEXT".to_string(),
                  homepage: Some("https://github.com/Syphon/Syphon-Framework".to_string()),
                  category: LicenseCategory::Native,
              },
          ];

          let rendered = render_notices(&entries);

          let expected = "THIRD-PARTY SOFTWARE NOTICES AND INFORMATION\n\nThis package incorporates components from the projects listed below.\n\n================================================================================\n\nlibfoo 1.0.0\nMIT\nhttps://example.com/libfoo\n\nMIT TEXT\n\n================================================================================\n\nSyphon Framework\nBSD-3-Clause\nhttps://github.com/Syphon/Syphon-Framework\n\nBSD TEXT\n";

          assert_eq!(rendered, expected);
      }

      #[test]
      fn entry_without_homepage_omits_the_homepage_line() {
          let entries = vec![LicenseEntry {
              name: "no-homepage-crate".to_string(),
              version: Some("2.0.0".to_string()),
              license: "MIT".to_string(),
              text: "TEXT".to_string(),
              homepage: None,
              category: LicenseCategory::Library,
          }];

          let rendered = render_notices(&entries);

          assert!(rendered.contains("no-homepage-crate 2.0.0\nMIT\n\nTEXT\n"));
      }
  }
  ```

  Run:
  ```
  cargo test -p xtask
  ```
  Expected: `test render::tests::renders_header_and_entries_with_separators ... ok`,
  `test render::tests::entry_without_homepage_omits_the_homepage_line ... ok`, all Step 2–6 tests
  green (10 total: 3 in `license_entry.rs` + 1 in `normalize.rs` + 2 in `merge.rs` + 2 in
  `sort.rs` + 2 in `render.rs`).

- [ ] **Step 7: verify and commit the pure function layer**

  ```
  cargo fmt --all
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test -p xtask
  git add crates/xtask Cargo.toml Cargo.lock
  git commit -m "feat(xtask): add license pipeline pure functions"
  ```
  Expected: clippy clean, `cargo test -p xtask` shows `10 passed; 0 failed`, husky pre-commit hook
  (`cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo
  test --workspace`) passes.

---

### Task 5: shell layer, appendix JSON, `gen-licenses` command, and committed generated artifacts

**Files:**
- Create: `licenses/appendix.json`
- Create: `.cargo/config.toml`
- Modify: `crates/xtask/Cargo.toml` (add `clap`, `thiserror`)
- Modify: `crates/xtask/src/main.rs` (replace Task 4 stub with the full CLI/shell layer)
- Modify: `mise.toml` (pin `cargo:cargo-bundle-licenses`)
- Create (generated, committed): `crates/gui/assets/third-party-licenses.json`
- Modify (generated, committed, replaces hand-written content): `THIRD-PARTY-NOTICES`

**Interfaces:**

Consumes: the Task 4 pure functions (`normalize`, `merge`, `sort_entries`, `render_notices`),
`licenses/appendix.json` (same `Vec<LicenseEntry>` wire schema as the pure layer's `Produces`, see
Task 4), and `cargo bundle-licenses --format json` subprocess stdout (see Task 4's verified
schema).

Produces: `crates/gui/assets/third-party-licenses.json` — the exact schema Task 6 embeds via
`include_str!` and parses with serde:
```json
[{"name": "eframe", "version": "0.35.0", "license": "MIT OR Apache-2.0", "text": "…full text…", "homepage": "https://github.com/emilk/egui", "category": "library"}]
```
and `THIRD-PARTY-NOTICES` (repo root, plain text, format from Task 4 Step 6's `render_notices`).

CLI surface:
```
cargo xtask gen-licenses          # writes both files, always
cargo xtask gen-licenses --check  # regenerates in memory, byte-compares against the committed
                                   # files, exits nonzero naming the first stale file
```

Error handling (fail-fast per spec §"エラーハンドリング方針" — xtask is a developer-invoked tool,
so subprocess/JSON failures abort immediately with a message, no silent fallback):
```rust
#[derive(Debug, thiserror::Error)]
enum XtaskError {
    #[error("failed to spawn `cargo bundle-licenses`: {0}")]
    Spawn(std::io::Error),
    #[error("`cargo bundle-licenses` exited with an error:\n{0}")]
    Subprocess(String),
    #[error("`cargo bundle-licenses` output was not valid UTF-8: {0}")]
    Utf8(std::string::FromUtf8Error),
    #[error("failed to parse JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("I/O error: {0}")]
    Io(std::io::Error),
    #[error("license artifact is stale: {0} does not match freshly generated output; run `cargo xtask gen-licenses`")]
    Stale(String),
}
```

---

- [ ] **Step 1: write `licenses/appendix.json` (repo root, exactly 2 entries)**

  Content copied verbatim from the current hand-written `THIRD-PARTY-NOTICES` (read at
  `/Users/napochaan/ghq/github.com/naporin0624/web-cam-sharedtexture/THIRD-PARTY-NOTICES`).
  Design decision on `homepage` vs. `text` split: for Syphon Framework the URL line that used to
  sit directly under the project name (`https://github.com/Syphon/Syphon-Framework`) becomes the
  structured `homepage` field verbatim, and `text` is the remaining BSD-3-Clause body. For LINE
  Seed JP the task brief specifies a *different* canonical `homepage`
  (`https://seed.line.me/`) than the URL already embedded in the file's body
  (`https://github.com/line/seed (release v20251119)`) — since that URL doesn't duplicate the new
  `homepage` field, it stays inside `text` exactly where it was, so the release version reference
  from the original file is not lost.

  ```json
  [
    {
      "name": "Syphon Framework",
      "version": null,
      "license": "BSD-3-Clause",
      "text": "Copyright 2010 bangnoise (Tom Butterworth) & vade (Anton Marini).\nAll rights reserved.\n\nRedistribution and use in source and binary forms, with or without\nmodification, are permitted provided that the following conditions are met:\n\n* Redistributions of source code must retain the above copyright\nnotice, this list of conditions and the following disclaimer.\n\n* Redistributions in binary form must reproduce the above copyright\nnotice, this list of conditions and the following disclaimer in the\ndocumentation and/or other materials provided with the distribution.\n\n* Neither the name of the Syphon Project nor the names of its contributors\nmay be used to endorse or promote products derived from this software\nwithout specific prior written permission.\n\nTHIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS \"AS IS\" AND\nANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED\nWARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE\nDISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDERS BE LIABLE FOR ANY\nDIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES\n(INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;\nLOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND\nON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT\n(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS\nSOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.",
      "homepage": "https://github.com/Syphon/Syphon-Framework",
      "category": "native"
    },
    {
      "name": "LINE Seed JP",
      "version": null,
      "license": "OFL-1.1",
      "text": "https://github.com/line/seed (release v20251119)\n\nFetched at build time by scripts/fetch-fonts.sh and embedded into the\ngemelli-gui binary via include_bytes! (crates/gui/src/fonts.rs).\n\nLicensed under the SIL Open Font License, Version 1.1. The full license text\nis written to vendor/fonts/LICENSE by scripts/fetch-fonts.sh at fetch time;\nit is not committed to this repository (vendor/fonts/ is gitignored).\n\n\"LINE\" is a trademark of LY Corporation. This notice does not grant any\nrights to that trademark.",
      "homepage": "https://seed.line.me/",
      "category": "font"
    }
  ]
  ```

- [ ] **Step 2: `.cargo/config.toml` — `cargo xtask` alias**

  ```toml
  [alias]
  xtask = "run --package xtask --"
  ```

- [ ] **Step 3: pin the scanner tool in `mise.toml`**

  ```toml
  [tools]
  rust = "1.96.1"
  node = "lts"
  pnpm = "10.33.2"
  "cargo:cargo-bundle-licenses" = "4.2.0"
  ```

  Run `mise install` and confirm `cargo bundle-licenses --version` resolves (expected output
  contains `cargo-bundle-licenses 4.2.0`).

- [ ] **Step 4: add `clap` + `thiserror` to `crates/xtask/Cargo.toml`**

  ```toml
  [package]
  name = "xtask"
  version = "0.1.0"
  edition.workspace = true
  license.workspace = true
  repository.workspace = true
  publish = false

  [lints]
  workspace = true

  [dependencies]
  clap = { workspace = true }
  serde = { workspace = true }
  serde_json = { workspace = true }
  thiserror = { workspace = true }
  ```

- [ ] **Step 5: replace the Task 4 stub with the full shell layer**

  `crates/xtask/src/main.rs`:
  ```rust
  use std::{
      path::{Path, PathBuf},
      process::{Command, ExitCode},
  };

  use clap::{Parser, Subcommand};

  mod license_entry;
  mod merge;
  mod normalize;
  mod render;
  mod sort;

  use license_entry::LicenseEntry;

  #[derive(Debug, thiserror::Error)]
  enum XtaskError {
      #[error("failed to spawn `cargo bundle-licenses`: {0}")]
      Spawn(std::io::Error),
      #[error("`cargo bundle-licenses` exited with an error:\n{0}")]
      Subprocess(String),
      #[error("`cargo bundle-licenses` output was not valid UTF-8: {0}")]
      Utf8(std::string::FromUtf8Error),
      #[error("failed to parse JSON: {0}")]
      Json(#[from] serde_json::Error),
      #[error("I/O error: {0}")]
      Io(std::io::Error),
      #[error("license artifact is stale: {0} does not match freshly generated output; run `cargo xtask gen-licenses`")]
      Stale(String),
  }

  #[derive(Parser)]
  #[command(name = "xtask")]
  struct Cli {
      #[command(subcommand)]
      command: Commands,
  }

  // Named `Commands`, not `Command` — `Command` collides with `std::process::Command` imported
  // above for the subprocess call, which fails to compile (E0255 name defined multiple times /
  // E0117 orphan rule on the derive). Caught by actually compiling this file during plan review.
  #[derive(Subcommand)]
  enum Commands {
      /// Regenerate crates/gui/assets/third-party-licenses.json and THIRD-PARTY-NOTICES.
      GenLicenses {
          /// Regenerate into memory and byte-compare against the committed files instead of
          /// writing them; exits nonzero if either file is stale.
          #[arg(long)]
          check: bool,
      },
  }

  struct Artifacts {
      json: String,
      notices: String,
  }

  fn project_root() -> PathBuf {
      // crates/xtask -> crates -> repo root. Standard cargo-xtask pattern: CARGO_MANIFEST_DIR is
      // resolved at compile time so this works regardless of the caller's current directory.
      let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
      root.pop();
      root.pop();
      root
  }

  fn run_cargo_bundle_licenses() -> Result<normalize::CargoBundleOutput, XtaskError> {
      let output = Command::new("cargo")
          .args(["bundle-licenses", "--format", "json"])
          .output()
          .map_err(XtaskError::Spawn)?;

      if !output.status.success() {
          let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
          return Err(XtaskError::Subprocess(stderr));
      }

      let stdout = String::from_utf8(output.stdout).map_err(XtaskError::Utf8)?;
      serde_json::from_str(&stdout).map_err(XtaskError::Json)
  }

  fn read_appendix(root: &Path) -> Result<Vec<LicenseEntry>, XtaskError> {
      let path = root.join("licenses/appendix.json");
      let raw = std::fs::read_to_string(path).map_err(XtaskError::Io)?;
      serde_json::from_str(&raw).map_err(XtaskError::Json)
  }

  fn build_artifacts(root: &Path) -> Result<Artifacts, XtaskError> {
      let scanned = run_cargo_bundle_licenses()?;
      let scanner_entries = normalize::normalize(scanned);
      let appendix_entries = read_appendix(root)?;
      let merged = merge::merge(scanner_entries, appendix_entries);
      let sorted = sort::sort_entries(merged);

      let mut json = serde_json::to_string_pretty(&sorted).map_err(XtaskError::Json)?;
      json.push('\n');
      let notices = render::render_notices(&sorted);

      Ok(Artifacts { json, notices })
  }

  fn check_matches(path: &Path, expected: &str) -> Result<(), XtaskError> {
      let actual = std::fs::read_to_string(path).map_err(XtaskError::Io)?;
      if actual != expected {
          return Err(XtaskError::Stale(path.display().to_string()));
      }
      Ok(())
  }

  fn gen_licenses(check: bool) -> Result<(), XtaskError> {
      let root = project_root();
      let artifacts = build_artifacts(&root)?;
      let assets_path = root.join("crates/gui/assets/third-party-licenses.json");
      let notices_path = root.join("THIRD-PARTY-NOTICES");

      if check {
          check_matches(&assets_path, &artifacts.json)?;
          check_matches(&notices_path, &artifacts.notices)?;
          println!("license artifacts are up to date");
          return Ok(());
      }

      if let Some(parent) = assets_path.parent() {
          std::fs::create_dir_all(parent).map_err(XtaskError::Io)?;
      }
      std::fs::write(&assets_path, &artifacts.json).map_err(XtaskError::Io)?;
      std::fs::write(&notices_path, &artifacts.notices).map_err(XtaskError::Io)?;
      println!("wrote {}", assets_path.display());
      println!("wrote {}", notices_path.display());
      Ok(())
  }

  fn main() -> ExitCode {
      let cli = Cli::parse();
      let result = match cli.command {
          Commands::GenLicenses { check } => gen_licenses(check),
      };

      match result {
          Ok(()) => ExitCode::SUCCESS,
          Err(err) => {
              eprintln!("error: {err}");
              ExitCode::FAILURE
          }
      }
  }
  ```

  Note: `normalize.rs`'s `CargoBundleOutput`/`CargoBundleLicense`/`CargoBundleLicenseText` structs
  (Task 4 Step 3) must be `pub` for `main.rs` to name `normalize::CargoBundleOutput` — they already
  are (see Task 4 Step 3's code).

- [ ] **Step 6: run the generator against the real dependency graph and verify `--check`**

  ```
  cargo build -p xtask
  cargo xtask gen-licenses
  ```
  Expected: two lines, `wrote .../crates/gui/assets/third-party-licenses.json` and `wrote
  .../THIRD-PARTY-NOTICES`, exit code 0. If `cargo bundle-licenses` errors on a missing local
  registry checkout, run `cargo fetch` first and retry — the scanner reads LICENSE files out of
  Cargo's local source cache and needs it warm.

  ```
  git diff --stat
  ```
  Expected: `crates/gui/assets/third-party-licenses.json` (new file) and `THIRD-PARTY-NOTICES`
  (modified, hand-written content replaced) show real diffs sized to the actual dependency count.

  ```
  cargo xtask gen-licenses --check
  ```
  Expected: `license artifacts are up to date`, exit code 0 (confirms the write path and the
  compare path agree byte-for-byte — the core correctness property of `--check`).

  Spot-check the committed schema:
  ```
  python3 -c "import json; data = json.load(open('crates/gui/assets/third-party-licenses.json')); \
    names = [e['name'] for e in data]; \
    assert 'Syphon Framework' in names and 'LINE Seed JP' in names; \
    assert all(e['category'] in ('library', 'font', 'native') for e in data); \
    print(len(data), 'entries; appendix present')"
  ```
  Expected: `<N> entries; appendix present` with no assertion error.

- [ ] **Step 7: lint, test, and commit the shell layer + generated artifacts**

  ```
  cargo fmt --all
  cargo clippy --workspace --all-targets -- -D warnings
  cargo test --workspace
  git add crates/xtask Cargo.toml Cargo.lock .cargo/config.toml mise.toml licenses/appendix.json \
    crates/gui/assets/third-party-licenses.json THIRD-PARTY-NOTICES
  git commit -m "feat(xtask): add gen-licenses command and generated artifacts"
  ```
  Expected: clippy clean, `cargo test --workspace` all green, husky pre-commit hook passes, commit
  created. After this commit, `cargo xtask gen-licenses --check` run from a clean checkout must
  exit 0 — this is the fixture Task 6's integration test ("committed
  `third-party-licenses.json` parses and contains both appendix entries") and the future
  `license-check.yml` CI workflow (spec §5, out of this task's scope) both depend on.

---

## API verification notes (read before implementing)

**egui 0.35.0 viewport API** (read from
`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/egui-0.35.0/src/context.rs` and
`.../src/viewport.rs` directly — not from memory):

```rust
// context.rs:3960
pub fn show_viewport_deferred(
    &self,
    new_viewport_id: ViewportId,
    viewport_builder: ViewportBuilder,
    viewport_ui_cb: impl Fn(&mut Ui, ViewportClass) + Send + Sync + 'static,
)

// context.rs:4014
pub fn show_viewport_immediate<T>(
    &self,
    new_viewport_id: ViewportId,
    builder: ViewportBuilder,
    mut viewport_ui_cb: impl FnMut(&mut Ui, ViewportClass) -> T,
) -> T
```

**Decision: `show_viewport_immediate`, not deferred.** The deferred callback bound is
`Fn + Send + Sync + 'static` — it cannot borrow `&mut self` from `LicensesWindow` (the closure
must own everything it touches and be shareable across the redraw thread), so a deferred
implementation would need `Arc<Mutex<LicensesWindowState>>` shared state with a second
synchronization layer just to let the search box and category filter mutate. The immediate
variant's callback is `FnMut` with no `'static`/`Send`/`Sync` bound and is called synchronously
inline (`viewport_ui_cb(ui, ViewportClass::Immediate)`, same stack frame, same thread) — so
`LicensesWindow::show(&mut self, ctx)` can pass a closure that directly captures `&mut self.query`,
`&mut self.category`, `&mut self.expanded` (Rust's per-field closure capture makes these disjoint
borrows valid alongside the immutable `&self.data` read). The performance cost the docs warn about
(both viewports repaint together) is irrelevant here: this window has no continuous rendering (no
video feed), it's a static list that redraws only on user interaction, exactly like eframe's own
`show_viewport_immediate` doc example use case.

Other verified signatures used below:
- `ViewportId::from_hash_of(source: impl AsId) -> Self` (viewport.rs:153) — stable id from a
  string, no need for a lazily-initialized static.
- `ViewportBuilder::with_title`, `::with_inner_size` (viewport.rs:356, 532) — builder pattern,
  `Default` impl provided.
- `ViewportCommand::Focus` (viewport.rs:1170) sent via `Context::send_viewport_cmd_to(id, cmd)`
  (context.rs:3921) — used for "focus the already-open window" on repeat menu clicks.
- `ViewportInfo::close_requested(&self) -> bool` (`src/data/input/viewport_info.rs:111`), read via
  `ui.ctx().input(|i| i.viewport().close_requested())` **inside** the immediate callback — `ui.ctx()`
  there is scoped to the child viewport's own `InputState`, not the parent's, so this reads the
  child window's own OS close-button click.
- `egui::Panel::top/bottom/left(id).show(ui: &mut Ui, ...)` and `egui::CentralPanel::default().show(ui: &mut Ui, ...)`
  both take `&mut Ui`, not `&Context` — confirmed in `containers/panel.rs:363` and `:1062`. This
  matches this repo's existing `app.rs` call sites (e.g. `egui::Panel::top("banner").show(ui, |ui| ...)`),
  so the licenses viewport's top bar/body panels are written the same way, passing the `ui` the
  immediate-viewport callback already received — no `ui.ctx()` needed for these calls.

**cargo-deny `[licenses]` schema** (verified via
<https://embarkstudios.github.io/cargo-deny/checks/licenses/cfg.html>): the `version` field is
obsolete/unused; `unlicensed`, `deny`, `copyleft`, `allow-osi-fsf-free`, `default` were all removed
and now error if present. The current schema is just `allow` (SPDX id list; for an `OR` expression
any one allowed license satisfies it, for an `AND` expression every listed license must be
individually allowed), `exceptions`, `private.ignore`, `confidence-threshold`, `clarify`. No
`version =` key is written below.

**cargo-deny-action** (verified via <https://github.com/EmbarkStudios/cargo-deny-action>): pin
`EmbarkStudios/cargo-deny-action@v2`, pass `command: check licenses` explicitly (this repo's
`deny.toml` intentionally has no `[advisories]`/`[bans]`/`[sources]` tables, so a bare `check`
would apply their unconfigured defaults instead of being scoped to licenses only).

**Full dependency-graph license enumeration** (verified by running, in this repo,
`cargo metadata --format-version 1 | python3 -c "..."` grouping every package by its `license`
field — see raw output transcribed below): the spec's baseline allowlist (`MIT`, `Apache-2.0`,
`BSD-2-Clause`, `BSD-3-Clause`, `ISC`, `Zlib`, `Unicode-3.0`) does **not** cover everything
currently in `Cargo.lock`. Five additions are required, none of them copyleft:

| License | Where it comes from | Why it must be listed explicitly |
| --- | --- | --- |
| `OFL-1.1` | `epaint_default_fonts` — expression `(MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0` | `AND` expression: every term must be individually allowed, not just the `OR` sub-clause |
| `Ubuntu-font-1.0` | same crate, same `AND` expression | same reason |
| `IJG` | `mozjpeg` (expression `IJG` alone) and `mozjpeg-sys` (`IJG AND Zlib AND BSD-3-Clause`) | standalone/`AND` expression, no `OR` fallback to an already-allowed license |
| `BSL-1.0` | `clipboard-win`, `error-code` (expression `BSL-1.0` alone) | standalone, Windows-clipboard deps pulled in transitively by egui/eframe even though this workspace doesn't ship on Windows yet |
| `CC0-1.0` | `hexf-parse` (expression `CC0-1.0` alone) | standalone, public-domain-equivalent dedication |

All five are permissive (font licenses, a BSD-style JPEG license, the Boost license, and a public
domain dedication) — no copyleft/weak-copyleft license (MPL-2.0, LGPL, GPL) appears anywhere in the
current graph, so there is no decision point requiring your sign-off beyond confirming these five.
Everything else in the graph (`Apache-2.0 WITH LLVM-exception OR ...`, `... OR GPL-2.0-only`,
`... OR LGPL-2.1-or-later`, legacy slash-separated forms like `MIT/Apache-2.0`, `Unlicense OR MIT`,
etc.) resolves through the `OR` clause down to an already-allowed license and needs no addition.

**ubuntu-latest verification for Task 8's CI**: `crates/syphon/src/lib.rs` is
`#![cfg(target_os = "macos")]` (whole-crate gate, not a `[target.'cfg(...)'.dependencies]` table —
see the comment in `crates/syphon/Cargo.toml` explaining this is deliberate so release-please's
manifest updater can still parse it) and `crates/syphon/build.rs` returns `Ok(())` immediately when
`CARGO_CFG_TARGET_OS != "macos"`, compiling nothing. `crates/gui/build.rs` similarly no-ops when
`DEP_SYPHON_BRIDGE_RPATH` isn't set. Neither of Task 8's CI jobs compiles the workspace at all —
`cargo deny check licenses` and `cargo xtask gen-licenses --check` both operate at the
`cargo metadata`/manifest level, which resolves the full dependency graph (including
platform-gated deps) without invoking `rustc` on any of it. `ubuntu-latest` is therefore
sufficient for both jobs; verified by running `cargo clippy --workspace --all-targets` and
`cargo metadata` locally on this macOS checkout without hitting any macOS-only compile step for
metadata-level commands.

---

### Task 6: `licenses.rs` data model — parse + filter (TDD)

**Files:**
- `crates/gui/src/licenses.rs` (new)
- `crates/gui/src/main.rs` (edit: add `mod licenses;`)
- `crates/gui/Cargo.toml` (edit: add `serde`, `serde_json` deps)
- `crates/gui/assets/third-party-licenses.json` (placeholder only if Task 5 hasn't landed yet — see Step 0)

**Interfaces:**
- Consumes: `crates/gui/assets/third-party-licenses.json` (Task 5's generated+committed asset;
  schema is the top-level JSON array of `LicenseEntry` below, sorted by `(category, name)`).
  Consumes `serde`/`serde_json` from `[workspace.dependencies]` (Task 5 adds these workspace-wide
  with the `derive` feature on `serde`; this task only wires them into `gemelli-gui`'s own
  `[dependencies]`).
- Produces: `pub enum LicenseCategory { Library, Font, Native }`, `pub struct LicenseEntry { name,
  version, license, text, homepage, category }`, `pub fn parse_licenses(json: &str) ->
  Result<Vec<LicenseEntry>, serde_json::Error>`, `pub fn filter_entries<'a>(entries: &'a
  [LicenseEntry], query: &str, category: Option<LicenseCategory>) -> Vec<&'a LicenseEntry>` — all
  consumed by Task 7 in the same file.

- [ ] **Step 0: coordinate the embedded asset with Task 5.**
  `include_str!("../assets/third-party-licenses.json")` is a *compile-time* dependency — if
  Task 5 hasn't landed yet when this task starts, `cargo test -p gemelli-gui` won't even build.
  Check first:
  ```bash
  test -f crates/gui/assets/third-party-licenses.json && echo present || echo missing
  ```
  If `missing`, create a minimal schema-valid placeholder so the crate compiles (this is
  overwritten byte-for-byte the next time Task 5's/Task 4's `cargo xtask gen-licenses` runs — the
  schema below matches exactly, so nothing here needs to change when that happens):
  ```bash
  mkdir -p crates/gui/assets
  ```
  ```json
  [
    {
      "name": "Syphon Framework",
      "version": null,
      "license": "BSD-3-Clause",
      "text": "Copyright 2010 bangnoise (Tom Butterworth) & vade (Anton Marini).\nAll rights reserved.\n\nRedistribution and use in source and binary forms, with or without\nmodification, are permitted provided that the following conditions are met:\n\n* Redistributions of source code must retain the above copyright\nnotice, this list of conditions and the following disclaimer.\n\n* Redistributions in binary form must reproduce the above copyright\nnotice, this list of conditions and the following disclaimer in the\ndocumentation and/or other materials provided with the distribution.\n\n* Neither the name of the Syphon Project nor the names of its contributors\nmay be used to endorse or promote products derived from this software\nwithout specific prior written permission.\n\nTHIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS \"AS IS\" AND\nANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED\nWARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE\nDISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDERS BE LIABLE FOR ANY\nDIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES\n(INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;\nLOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND\nON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT\n(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS\nSOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.",
      "homepage": "https://github.com/Syphon/Syphon-Framework",
      "category": "native"
    },
    {
      "name": "LINE Seed JP",
      "version": null,
      "license": "OFL-1.1",
      "text": "This Font Software is licensed under the SIL Open Font License, Version 1.1.\nSee https://scripts.sil.org/OFL for the full license text.",
      "homepage": "https://seed.line.me/index_jp.html",
      "category": "font"
    }
  ]
  ```
  If `present`, skip this step entirely and do not touch the file — it's Task 5's committed
  output and already contains these two appendix entries plus every crate entry.

- [ ] **Step 1: RED — write the failing tests first.**
  Create `crates/gui/src/licenses.rs` with only the type definitions and test module (no
  `parse_licenses`/`filter_entries` bodies yet — stub them to `unimplemented!()`... but this
  workspace denies `unwrap_used`/`expect_used`, not `unimplemented!`, and clippy's restriction
  lints don't fire inside `#[cfg(test)]` per this repo's existing pattern (verified: `cargo clippy
  --workspace --all-targets` is currently clean despite `app.rs`'s test module calling `.unwrap()`
  on channel sends — clippy's `unwrap_used`/`expect_used` skip test-cfg code). Write the full file
  in one pass (types + impls + tests together) rather than a separate stub pass, since the type
  definitions themselves have no failure mode to TDD — only `parse_licenses` and `filter_entries`
  do:

  ```rust
  //! Bundled third-party license data: parsing the generated manifest and filtering it for the
  //! licenses window (Task 7 adds the window's rendering to this same module).

  /// Which of the three sources a license entry came from. `Font`/`Native` entries are the
  /// hand-written appendix (Syphon Framework, LINE Seed JP) that `cargo xtask gen-licenses` merges
  /// in; `Library` is every Rust crate dependency `cargo-bundle-licenses` discovers.
  #[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
  #[serde(rename_all = "lowercase")]
  pub enum LicenseCategory {
      Library,
      Font,
      Native,
  }

  #[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
  pub struct LicenseEntry {
      pub name: String,
      pub version: Option<String>,
      pub license: String,
      pub text: String,
      pub homepage: Option<String>,
      pub category: LicenseCategory,
  }

  /// The generated+committed license manifest (Task 5's `cargo xtask gen-licenses` output).
  /// `include_str!` makes a missing/unreadable file a *compile* error — the only failure mode left
  /// at runtime is malformed *content*, which `parse_licenses`'s `Result` surfaces instead of
  /// panicking (the workspace denies `unwrap_used`/`expect_used`, so this is the only option
  /// anyway).
  const EMBEDDED_LICENSES_JSON: &str = include_str!("../assets/third-party-licenses.json");

  /// Parses the embedded manifest. Never panics.
  pub fn parse_licenses(json: &str) -> Result<Vec<LicenseEntry>, serde_json::Error> {
      serde_json::from_str(json)
  }

  /// Case-insensitive substring match on `name`, AND'd with an optional exact `category` match.
  /// `query = ""` matches every entry (the empty string is a substring of everything) — this is
  /// the window's initial "nothing typed yet" state.
  pub fn filter_entries<'a>(
      entries: &'a [LicenseEntry],
      query: &str,
      category: Option<LicenseCategory>,
  ) -> Vec<&'a LicenseEntry> {
      let query_lower = query.to_lowercase();
      entries
          .iter()
          .filter(|entry| entry.name.to_lowercase().contains(&query_lower))
          .filter(|entry| category.as_ref().is_none_or(|wanted| &entry.category == wanted))
          .collect()
  }

  #[cfg(test)]
  mod tests {
      use super::*;

      const SAMPLE_JSON: &str = r#"[
          {
              "name": "serde",
              "version": "1.0.210",
              "license": "MIT OR Apache-2.0",
              "text": "MIT License text goes here.",
              "homepage": "https://serde.rs",
              "category": "library"
          },
          {
              "name": "LINE Seed JP",
              "version": null,
              "license": "OFL-1.1",
              "text": "SIL Open Font License text goes here.",
              "homepage": null,
              "category": "font"
          }
      ]"#;

      #[test]
      fn parse_licenses_reads_a_well_formed_manifest() {
          let entries = parse_licenses(SAMPLE_JSON).unwrap();

          assert_eq!(entries.len(), 2);
          assert_eq!(entries[0].name, "serde");
          assert_eq!(entries[0].version, Some("1.0.210".to_string()));
          assert_eq!(entries[0].category, LicenseCategory::Library);
          assert_eq!(entries[1].name, "LINE Seed JP");
          assert_eq!(entries[1].version, None);
          assert_eq!(entries[1].category, LicenseCategory::Font);
      }

      #[test]
      fn parse_licenses_reports_malformed_json_as_an_error_not_a_panic() {
          let result = parse_licenses("{ not valid json");

          assert!(result.is_err());
      }

      #[test]
      fn filter_entries_matches_name_case_insensitively() {
          let entries = parse_licenses(SAMPLE_JSON).unwrap();

          let filtered = filter_entries(&entries, "SERDE", None);

          assert_eq!(filtered.len(), 1);
          assert_eq!(filtered[0].name, "serde");
      }

      #[test]
      fn filter_entries_by_category_only() {
          let entries = parse_licenses(SAMPLE_JSON).unwrap();

          let filtered = filter_entries(&entries, "", Some(LicenseCategory::Font));

          assert_eq!(filtered.len(), 1);
          assert_eq!(filtered[0].name, "LINE Seed JP");
      }

      #[test]
      fn filter_entries_combines_query_and_category_with_and() {
          let entries = parse_licenses(SAMPLE_JSON).unwrap();

          // Matches the query but not the category -> excluded.
          let filtered = filter_entries(&entries, "serde", Some(LicenseCategory::Font));

          assert!(filtered.is_empty());
      }

      #[test]
      fn filter_entries_with_empty_query_and_no_category_returns_everything() {
          let entries = parse_licenses(SAMPLE_JSON).unwrap();

          let filtered = filter_entries(&entries, "", None);

          assert_eq!(filtered.len(), entries.len());
      }

      #[test]
      fn committed_asset_parses_and_contains_both_appendix_entries() {
          let entries = parse_licenses(EMBEDDED_LICENSES_JSON).unwrap();

          assert!(
              entries.iter().any(|e| e.name == "Syphon Framework"),
              "committed manifest is missing the Syphon Framework appendix entry"
          );
          assert!(
              entries.iter().any(|e| e.name == "LINE Seed JP"),
              "committed manifest is missing the LINE Seed JP appendix entry"
          );
      }
  }
  ```

  Run it and confirm it fails to *compile* first (proving the test file didn't silently exist
  already), then delete nothing and let the real bodies above make it pass in the same edit — this
  task's "RED" step is the `include_str!`/dependency wiring below, not the pure-function logic
  (which has no meaningful intermediate broken state worth committing).

- [ ] **Step 2: wire dependencies before compiling.** Edit `crates/gui/Cargo.toml`:
  ```toml
  [dependencies]
  gemelli-core = { path = "../core" }
  eframe = { workspace = true }
  egui = { workspace = true }
  thiserror = { workspace = true }
  arc-swap = { workspace = true }
  serde = { workspace = true }
  serde_json = { workspace = true }
  # Platform gating lives inside gemelli-syphon itself (crate-wide
  # `#![cfg(target_os = "macos")]`), not in a `[target.'cfg(...)'.dependencies]` table here:
  # release-please's Rust manifest updater cannot parse cfg() target tables and fails to bump
  # this crate's version.
  gemelli-syphon = { path = "../syphon" }
  ```
  This assumes Task 5 has already added `serde = { version = "1", features = ["derive"] }` and
  `serde_json = { version = "1" }` to the workspace root `Cargo.toml`'s `[workspace.dependencies]`.
  If `cargo check -p gemelli-gui` fails with "no field `derive`"-shaped errors or "cannot find
  derive macro", check the root `Cargo.toml` first — the fix belongs there, not in this crate.

- [ ] **Step 3: register the module.** Edit `crates/gui/src/main.rs`, inserting alphabetically:
  ```rust
  mod app;
  mod crop_editor;
  mod fonts;
  mod fps_meter;
  mod licenses;
  mod preview;
  mod sidebar;
  mod theme;
  mod worker;
  ```

- [ ] **Step 4: GREEN.**
  ```bash
  cargo test -p gemelli-gui licenses
  ```
  Expected output (7 tests from Step 1):
  ```
  running 7 tests
  test licenses::tests::filter_entries_by_category_only ... ok
  test licenses::tests::filter_entries_combines_query_and_category_with_and ... ok
  test licenses::tests::filter_entries_matches_name_case_insensitively ... ok
  test licenses::tests::filter_entries_with_empty_query_and_no_category_returns_everything ... ok
  test licenses::tests::parse_licenses_reads_a_well_formed_manifest ... ok
  test licenses::tests::parse_licenses_reports_malformed_json_as_an_error_not_a_panic ... ok
  test licenses::tests::committed_asset_parses_and_contains_both_appendix_entries ... ok

  test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
  ```

- [ ] **Step 5: lint + full suite, then commit.**
  ```bash
  cargo fmt --all
  cargo clippy --workspace --all-targets
  cargo test --workspace
  ```
  Both must be clean/green before committing (`as_conversions`/`unwrap_used`/`expect_used` are
  denied workspace-wide for non-test code; this file's non-test code uses no `as` casts and no
  `.unwrap()`/`.expect()` — `parse_licenses` returns `Result`, `filter_entries` uses
  `Option::is_none_or`, not `.unwrap()`).
  ```bash
  git add crates/gui/src/licenses.rs crates/gui/src/main.rs crates/gui/Cargo.toml
  # + crates/gui/assets/third-party-licenses.json only if Step 0 created a placeholder
  git commit -m "feat(gui): add licenses data model"
  ```

---

### Task 7: licenses viewport UI + menu wiring

**Files:**
- `crates/gui/src/licenses.rs` (edit: append `LicensesWindow` + rendering + view-model helpers)
- `crates/gui/src/app.rs` (edit: `GemelliApp` gains a `licenses: LicensesWindow` field; wire
  `OpenLicenses` and the per-frame `show` call)

**Interfaces:**
- Consumes: `crate::theme::tokens::{BG_MUTED, TEXT_MUTED, TEXT_SUBTLE, ACCENT, BORDER_SUBTLE,
  DANGER}` (the first five from Task 1's retheme; `DANGER` already exists in today's
  `theme.rs` and is reused as-is for the parse-error message). Consumes, from Task 3's menu
  wiring in `app.rs`: a `MenuAction::OpenLicenses` variant reaching `GemelliApp` once per menu
  click, and the general shape "menu events are drained once near the top of `ui()`". This task
  removes whatever standalone `licenses_open: bool` field/write Task 3 introduced and replaces its
  single call site with `self.licenses.request_open(ui.ctx())` (see Step 4) — `LicensesWindow`
  becomes the sole owner of open/closed state, there is no longer a separate bool on `GemelliApp`.
- Produces: `pub struct LicensesWindow { .. }` with `pub fn request_open(&mut self, ctx:
  &egui::Context)` and `pub fn show(&mut self, ctx: &egui::Context)`, plus
  `pub(crate) fn version_display(version: &Option<String>) -> &str` and
  `pub(crate) fn license_badge_text(license: &str) -> String` (pure, unit-tested; the row-drawing
  code itself is not unit-tested, matching this repo's existing `sidebar.rs` split between tested
  pure helpers and untested rendering).

- [ ] **Step 1: RED — view-model helper tests.** Append to `crates/gui/src/licenses.rs`'s test
  module (the function bodies go in the same edit, same reasoning as Task 6 Step 1 — these are
  one-line pure functions with no meaningful broken intermediate state):
  ```rust
  #[test]
  fn version_display_shows_an_em_dash_for_a_missing_version() {
      assert_eq!(version_display(&None), "\u{2014}");
  }

  #[test]
  fn version_display_shows_the_version_when_present() {
      assert_eq!(version_display(&Some("1.2.3".to_string())), "1.2.3");
  }

  #[test]
  fn license_badge_text_leaves_a_single_license_unchanged() {
      assert_eq!(license_badge_text("MIT"), "MIT");
  }

  #[test]
  fn license_badge_text_normalizes_an_or_expression_to_a_slash() {
      assert_eq!(license_badge_text("MIT OR Apache-2.0"), "MIT / Apache-2.0");
  }
  ```

- [ ] **Step 2: GREEN — add the pure helpers and the `LicensesWindow` type.** Append (above the
  test module) to `crates/gui/src/licenses.rs`:
  ```rust
  use std::collections::HashSet;
  use std::sync::OnceLock;

  use crate::theme;

  /// Display text for a possibly-absent version — font/native appendix entries have no crate
  /// version to show.
  pub(crate) fn version_display(version: &Option<String>) -> &str {
      version.as_deref().unwrap_or("\u{2014}")
  }

  /// Right-aligned license badge text. Crates commonly express dual licenses in SPDX `OR` form
  /// (`"MIT OR Apache-2.0"`); normalized to the more compact `"MIT / Apache-2.0"` for the badge.
  pub(crate) fn license_badge_text(license: &str) -> String {
      license.replace(" OR ", " / ")
  }

  fn category_toggle(
      ui: &mut egui::Ui,
      current: &mut Option<LicenseCategory>,
      value: Option<LicenseCategory>,
      label: &str,
  ) {
      if ui.selectable_label(*current == value, label).clicked() {
          *current = value;
      }
  }

  /// Draws one row per filtered entry. `all_entries` (not `filtered`) is what `expanded`'s indices
  /// are keyed against: `filtered` is rebuilt fresh from `filter_entries` every frame, so a row's
  /// position within it shifts as the search box/category filter change — keying `expanded` off
  /// `filtered`'s own position would silently reattach the "expanded" flag to a different entry
  /// the moment the visible set changes shape. Keying off `all_entries`'s stable index avoids
  /// that; `filtered`'s items are `&LicenseEntry` borrowed from `all_entries`, so `std::ptr::eq`
  /// recovers the original index without re-running the filter's own matching logic.
  fn render_entry_list(
      ui: &mut egui::Ui,
      all_entries: &[LicenseEntry],
      filtered: &[&LicenseEntry],
      expanded: &mut HashSet<usize>,
  ) {
      egui::ScrollArea::vertical().show(ui, |ui| {
          for entry in filtered {
              let Some(index) =
                  all_entries.iter().position(|candidate| std::ptr::eq(candidate, *entry))
              else {
                  continue;
              };

              ui.horizontal(|ui| {
                  let name_clicked =
                      ui.add(egui::Label::new(&entry.name).sense(egui::Sense::click())).clicked();
                  ui.label(version_display(&entry.version));
                  ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                      ui.colored_label(theme::tokens::TEXT_SUBTLE, license_badge_text(&entry.license));
                  });

                  if name_clicked {
                      if expanded.contains(&index) {
                          expanded.remove(&index);
                      } else {
                          expanded.insert(index);
                      }
                  }
              });

              if expanded.contains(&index) {
                  egui::Frame::NONE.fill(theme::tokens::BG_MUTED).inner_margin(8.0).show(ui, |ui| {
                      ui.colored_label(theme::tokens::TEXT_MUTED, &entry.text);
                      if let Some(homepage) = &entry.homepage {
                          ui.hyperlink_to(homepage, homepage);
                      }
                  });
              }

              ui.add(egui::Separator::default().spacing(0.0));
          }
      });
  }

  /// Deferred vs. immediate viewport: see the plan doc's API-verification section for why this
  /// uses `show_viewport_immediate` — the short version is that the immediate callback can borrow
  /// `&mut self`'s fields directly, while the deferred one requires `Send + Sync + 'static` and
  /// would force an `Arc<Mutex<_>>` around this window's state for no benefit (no continuous
  /// rendering happens here that would need the deferred variant's independent repaint scheduling).
  #[derive(Default)]
  pub struct LicensesWindow {
      open: bool,
      query: String,
      category: Option<LicenseCategory>,
      expanded: HashSet<usize>,
      data: OnceLock<Result<Vec<LicenseEntry>, serde_json::Error>>,
  }

  impl LicensesWindow {
      fn viewport_id() -> egui::ViewportId {
          egui::ViewportId::from_hash_of("gemelli-licenses-window")
      }

      /// Called from `MenuAction::OpenLicenses` handling in `app.rs`. Opens the window on first
      /// request; if it's already open, focuses the existing native window instead of no-op'ing —
      /// re-clicking "Open Source Licenses…" while it's already open should bring it forward, not
      /// silently do nothing.
      pub fn request_open(&mut self, ctx: &egui::Context) {
          if self.open {
              ctx.send_viewport_cmd_to(Self::viewport_id(), egui::ViewportCommand::Focus);
          } else {
              self.open = true;
          }
      }

      /// Renders the licenses viewport if open; a no-op otherwise. Called unconditionally every
      /// frame from `GemelliApp::ui`, mirroring how every other panel in `app.rs` draws
      /// unconditionally and gates its own visibility internally.
      pub fn show(&mut self, ctx: &egui::Context) {
          if !self.open {
              return;
          }

          let entries_result = self.data.get_or_init(|| parse_licenses(EMBEDDED_LICENSES_JSON));
          let query = &mut self.query;
          let category = &mut self.category;
          let expanded = &mut self.expanded;
          let open = &mut self.open;

          ctx.show_viewport_immediate(
              Self::viewport_id(),
              egui::ViewportBuilder::default()
                  .with_title("Open Source Licenses — gemelli")
                  .with_inner_size([640.0, 520.0]),
              |ui, _class| {
                  if ui.ctx().input(|i| i.viewport().close_requested()) {
                      *open = false;
                  }

                  egui::Panel::top("licenses_top_bar").show(ui, |ui| {
                      ui.add_space(4.0);
                      ui.horizontal(|ui| {
                          ui.text_edit_singleline(query);
                          ui.add(egui::Separator::default().vertical());
                          category_toggle(ui, category, None, "All");
                          category_toggle(ui, category, Some(LicenseCategory::Library), "Library");
                          category_toggle(ui, category, Some(LicenseCategory::Font), "Font");
                          category_toggle(ui, category, Some(LicenseCategory::Native), "Native");
                      });
                      ui.add_space(4.0);
                  });

                  egui::CentralPanel::default().show(ui, |ui| match entries_result {
                      Ok(entries) => {
                          let filtered = filter_entries(entries, query, category.clone());
                          render_entry_list(ui, entries, &filtered, expanded);
                      }
                      Err(error) => {
                          ui.colored_label(
                              theme::tokens::DANGER,
                              format!("failed to load bundled licenses: {error}"),
                          );
                      }
                  });
              },
          );
      }
  }
  ```
  Note the `BORDER_SUBTLE`/`ACCENT` tokens named in the design contract are available for finer
  styling (hairline row separators, the search box's focus outline) but aren't force-fit into this
  minimal pass — `ui.add(egui::Separator::default()...)` already renders a hairline using the
  ambient `Visuals::widgets` stroke Task 1's retheme points at `BORDER_SUBTLE`; wiring a literal
  `theme::tokens::BORDER_SUBTLE` call here would fight that instead of using it. If Task 1 lands
  first and its contrast tests pin `Separator` to a different stroke, adjust this file then rather
  than guessing here.

- [ ] **Step 3: GREEN — run the new tests.**
  ```bash
  cargo test -p gemelli-gui licenses
  ```
  Expected: the 4 new tests plus Task 6's 7, all passing (11 total):
  ```
  test result: ok. 11 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
  ```

- [ ] **Step 4: wire into `GemelliApp` (`crates/gui/src/app.rs`).** Add the field next to the
  other UI-only state (near `banner`/`preview_mode`):
  ```rust
  pub struct GemelliApp {
      // ...existing fields...
      banner: Option<String>,
      licenses: crate::licenses::LicensesWindow,
      // ...
  }
  ```
  In `GemelliApp::new`, add `licenses: crate::licenses::LicensesWindow::default(),` to the
  struct literal (matches `#[derive(Default)]` on `LicensesWindow` — every field type
  (`bool`, `String`, `Option<T>`, `HashSet`, `OnceLock`) implements `Default`, so this needs no
  hand-written constructor).

  Find Task 3's menu-event handling (it will look roughly like a `try_recv` loop over
  `MenuEvent::receiver()` translating to `MenuAction`, matched near the top of
  `impl eframe::App for GemelliApp { fn ui(...) }`). Wherever it currently does something like
  `MenuAction::OpenLicenses => self.licenses_open = true,`, replace it with:
  ```rust
  MenuAction::OpenLicenses => self.licenses.request_open(ui.ctx()),
  ```
  and delete the now-unused `licenses_open: bool` field and its initialization entirely —
  `LicensesWindow.open` is the single source of truth from this task onward.

  Then, in `fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame)`, add the render call
  alongside the other panel calls (order doesn't matter functionally since it's an independent
  native viewport, not a panel inside the main window — placing it right after
  `self.drain_errors(); self.refresh_preview(ui.ctx());` keeps all the "things that happen once per
  frame regardless of layout" calls grouped together):
  ```rust
  fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
      self.drain_errors();
      self.refresh_preview(ui.ctx());
      self.licenses.show(ui.ctx());

      // ...rest of the existing method unchanged...
  }
  ```

- [ ] **Step 5: lint + full suite, then commit.**
  ```bash
  cargo fmt --all
  cargo clippy --workspace --all-targets
  cargo test --workspace
  ```
  ```bash
  git add crates/gui/src/licenses.rs crates/gui/src/app.rs
  git commit -m "feat(gui): add open-source licenses window"
  ```

- [ ] **Step 6: manual check + difit review.** Per this repo's workflow, run the GUI, open
  `Help ▸ Open Source Licenses…` (available once Task 3's menu lands), confirm search/category
  filtering and row expansion behave, then start `difit` and request review before moving on.

---

### Task 8: `deny.toml` + GitHub Actions CI

**Files:**
- `deny.toml` (new, repo root)
- `.github/workflows/license-check.yml` (new — first workflow in this repo)
- `README.md` (edit: add a short "license checks" section — see Step 3)

**Interfaces:**
- Consumes: the full dependency-graph license enumeration verified above. Consumes `cargo xtask
  gen-licenses --check` (Task 5's xtask deliverable, out of this task's scope to build — this task
  only calls it from CI and documents it) and `cargo-bundle-licenses` (pinned in Task 5's
  `mise.toml`; this task's CI step installs the **same** pinned version — keep both in sync
  manually, there's no shared source of truth between a GitHub Actions step and `mise.toml`).
- Produces: a hard CI gate on `push`/`pull_request` for both license policy and generated-manifest
  freshness. No changes to husky/pre-commit hooks (per the spec's explicit non-goal: this check is
  "too heavy for every commit").

- [ ] **Step 1: write `deny.toml` at the repo root.**
  ```toml
  # Third-party license policy for this workspace's full Cargo dependency graph. See
  # docs/superpowers/specs/2026-07-08-distribution-prep-design.md section 5.
  #
  # Only `cargo deny check licenses` is in scope — [advisories]/[bans]/[sources] are deliberately
  # left unconfigured. Always invoke with an explicit `check licenses`; a bare `cargo deny check`
  # would also run those unconfigured checks against their own default policies.
  [licenses]
  confidence-threshold = 0.95
  allow = [
      "MIT",
      "Apache-2.0",
      "BSD-2-Clause",
      "BSD-3-Clause",
      "ISC",
      "Zlib",
      "Unicode-3.0",
      # epaint_default_fonts' expression is `(MIT OR Apache-2.0) AND OFL-1.1 AND Ubuntu-font-1.0`
      # — an AND expression requires every term individually allowed, not just the OR sub-clause.
      # OFL-1.1 also matches the LINE Seed JP appendix entry's own license.
      "OFL-1.1",
      "Ubuntu-font-1.0",
      # mozjpeg ("IJG") / mozjpeg-sys ("IJG AND Zlib AND BSD-3-Clause"). IJG License is a
      # permissive BSD-style license from the Independent JPEG Group.
      "IJG",
      # clipboard-win / error-code, pulled in transitively by egui/eframe's Windows clipboard
      # support even though this workspace doesn't ship on Windows yet. Boost Software License,
      # permissive.
      "BSL-1.0",
      # hexf-parse. Public-domain-equivalent dedication, permissive.
      "CC0-1.0",
  ]
  # gemelli-{core,cli,gui,syphon} are all `license = "MIT"` (workspace.package), already covered
  # by the allow list above — no [licenses.private] override is needed.
  ```

- [ ] **Step 2: verify locally (requires `cargo-deny` installed — `cargo install cargo-deny` if
  not already present via mise/Task 5).**
  ```bash
  cargo deny check licenses
  ```
  Expected output on a clean pass:
  ```
  licenses ok
  ```
  If it instead reports a license not in the allow list, that means the dependency graph changed
  since this enumeration (a new transitive dependency was added) — do not silently add the
  license; identify which crate pulled it in (`cargo tree -i <crate>`), confirm it's permissive,
  and only then extend `allow` with a comment matching the style above.

- [ ] **Step 3: `.github/workflows/license-check.yml`.**
  ```yaml
  name: License Check

  on:
    push:
      branches: [main]
    pull_request:

  jobs:
    policy:
      name: cargo-deny (licenses)
      runs-on: ubuntu-latest
      steps:
        - uses: actions/checkout@v4
        - uses: EmbarkStudios/cargo-deny-action@v2
          with:
            command: check licenses

    freshness:
      name: license manifest freshness
      runs-on: ubuntu-latest
      steps:
        - uses: actions/checkout@v4
        - uses: dtolnay/rust-toolchain@1.96.1
          # Keep this pin in sync with `mise.toml`'s `rust` tool version.
        - name: Install cargo-bundle-licenses
          run: cargo install cargo-bundle-licenses --version 4.2.0 --locked
          # Keep this version pin in sync with Task 5's mise.toml pin for the same tool — if that
          # version changes, bump this line in the same PR.
        - name: Check generated license manifest is up to date
          run: cargo xtask gen-licenses --check
  ```
  Both jobs run on `ubuntu-latest` — see the API-verification section above for why this is safe
  (neither job compiles the macOS-gated `gemelli-syphon`/`gemelli-gui` crates; both operate at the
  `cargo metadata`/manifest level).

- [ ] **Step 4: document the local commands.** Add a short section to `README.md` (create the
  section if the file has no natural home for it yet — a "## License checks" heading near any
  existing "## Development"/"## Testing" section is fine; if `README.md` has no such structure,
  append it at the end):
  ```markdown
  ## License checks

  This repo enforces a permissive-only license policy on its Cargo dependency graph and keeps a
  generated third-party license manifest in sync with `Cargo.lock`. Both run in CI on every push
  and pull request; neither runs in the pre-commit hook (too heavy for every commit).

  ```bash
  # Verify every dependency's license is on the allow list (`deny.toml`).
  cargo deny check licenses

  # Regenerate crates/gui/assets/third-party-licenses.json and THIRD-PARTY-NOTICES after adding
  # or removing a dependency, then commit the result.
  cargo xtask gen-licenses
  ```
  ```

- [ ] **Step 5: commit.**
  ```bash
  git add deny.toml .github/workflows/license-check.yml README.md
  git commit -m "ci: add license policy and freshness checks"
  ```
  This task has no Rust unit tests of its own (it's policy config + CI wiring); its "test" is
  Step 2's local `cargo deny check licenses` run plus watching the new workflow go green on the
  PR this lands in.
