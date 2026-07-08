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

---

# Implementation plan addendum — Tasks 9–10 (Phase 3.5: portrait layout + widget fidelity)

Branch: `feature/distribution-prep` (continues the existing task numbering; the last landed
commit on this branch is `8b6e5b3 perf(gui): share frames via Arc and skip redundant texture
uploads`). Spec: `docs/superpowers/specs/2026-07-08-portrait-ui-design.md`.

**Verification status for this addendum**: Task 9's `widgets.rs` was fully applied to the working
tree and verified live — compiled, its 12 unit tests run and pass, `cargo clippy --workspace
--all-targets -- -D warnings` is clean, `cargo fmt --all -- --check` is clean. Task 10's full
`app.rs`/`main.rs`/`sidebar.rs` restructure was also fully applied together with Task 9's
`widgets.rs` and verified live — `cargo check -p gemelli-gui --all-targets` passes, `cargo clippy
-p gemelli-gui --all-targets -- -D warnings` is clean, `cargo test -p gemelli-gui` passes (114
tests, 2 ignored — same ignores as before, 0 failures), `cargo fmt --all -- --check` is clean, and
`cargo run -p gemelli-gui` was launched in the background, observed alive (not crashed) after 12
seconds, then killed cleanly — no panic. The working tree was then fully reverted
(`git checkout -- crates/gui/src/app.rs crates/gui/src/main.rs crates/gui/src/sidebar.rs
crates/gui/src/theme.rs && rm crates/gui/src/widgets.rs`); `git status` confirms the tree is clean
except for the pre-existing untracked spec doc. Everything below is therefore the exact code that
was verified to work, not a guess — the two tasks are split back into their intended commit
boundaries for execution.

All egui/eframe API signatures cited below were read directly from
`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/egui-0.35.0/src/` (paths noted inline) —
none are assumed from memory.

---

## Task 9: `crates/gui/src/widgets.rs` — shared widget primitives (TDD)

### Files

- **New:** `crates/gui/src/widgets.rs`
- **Modify:** `crates/gui/src/main.rs` (add `mod widgets;`)

### Interfaces (frozen)

```rust
pub(crate) fn cell_bounds(total_width: f32, count: usize) -> Vec<(f32, f32)>;
pub(crate) fn cell_at(x_offset: f32, total_width: f32, count: usize) -> usize;
pub(crate) fn flip_segment_index(h: bool, v: bool) -> usize;
pub(crate) fn flip_from_segment_index(index: usize) -> (bool, bool);
pub(crate) fn group_label(ui: &mut egui::Ui, text: &str);
pub(crate) fn segmented(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    selected: &mut usize,
    labels: &[&str],
) -> egui::Response;
pub(crate) fn action_button(ui: &mut egui::Ui, label: &str) -> egui::Response;
```

Design decisions frozen here (each was ambiguous in the spec and is resolved once, not re-litigated
in Task 10):

- **`segmented` returns a plain `egui::Response`**, not a struct. It never calls
  `response.mark_changed()` (this custom-painted widget has no reason to touch egui's internal
  change-tracking), so callers detect a change themselves by comparing `*selected` before/after the
  call — exactly the pattern `sidebar.rs`'s existing panel functions already use (e.g.
  `rotate_panel` returning `bool` via a `previous` snapshot). Task 10's callers follow this.
- **`group_label` uppercases internally.** Callers pass normal-case text (`"Device"`); the function
  is the single place that knows the brow-label visual convention is "uppercase + 11px +
  `TEXT_SUBTLE`" (egui has no letter-spacing control, so this substitutes for it).
- **Hover fill for unselected segmented cells is `BG_MUTED`, never `ACCENT_HOVER`.**
  `ACCENT_HOVER` is reserved for `action_button` alone, per the spec's explicit "初消費" note for
  that token. This is enforced by construction (the `segmented` function body never reads
  `ACCENT_HOVER`), not by convention.
- **`action_button`'s "strong text"** is rendered as a larger `FontId` (15px vs. `segmented`'s 13px)
  rather than an actual bold font weight: `crates/gui/src/fonts.rs` only registers LINE Seed JP
  Regular (no bold variant), and `Painter::text` takes a plain `FontId` (size + family) with no
  weight parameter — there is no bold to ask for. (For context: `RichText::strong()` — used
  elsewhere in this codebase via `ui.label(RichText::new(..))` — doesn't change font weight either;
  per `egui-0.35.0/src/widget_text.rs:249` its doc comment is literally "Extra strong text
  (stronger color)". It's moot here regardless, since `action_button` paints via `Painter::text`,
  which has no `RichText` overload.)
- **Corner radius is `0.0` everywhere** (matches `theme::apply_theme`'s neo-brutalist zero-radius
  policy already applied to every other widget).
- **`id_salt` is threaded into an explicit `egui::Id`** (`egui::Id::new(id_salt)`), not left to
  egui's per-call-site auto-id counter. Verified requirement: `egui::Id::new` takes `impl AsId`
  (`egui-0.35.0/src/id.rs:11`: `pub trait AsId: std::hash::Hash + std::fmt::Debug {}`, blanket-impl'd
  for any such `T` at line 13) — hence the bound on `segmented`'s `id_salt` parameter is
  `Hash + Debug`, not bare `Hash`. Reason to bother: Task 10's CROP numeric row appears/disappears
  based on state, which shifts every later widget's auto-id in the same frame; an explicitly salted
  id keeps ROTATE/FLIP/SCALE's segmented identity stable regardless of what renders above them.

### `as`-cast avoidance (workspace lint: `as_conversions = "deny"`, `Cargo.toml:25`)

`cell_bounds` needs `total_width: f32` divided by `count: usize` cells. `count as f32` is banned.
`f32::from(u32)` doesn't exist in std (a `u32` can exceed `f32`'s 24-bit mantissa and lose
precision) — only `From<u8>`/`From<u16>` (and their signed counterparts) target `f32` losslessly.
Since no segmented control here ever has more than 4 cells, the conversion goes through `u16`
first:

```rust
fn count_to_f32(count: usize) -> f32 {
    f32::from(u16::try_from(count).unwrap_or(u16::MAX))
}
```

`u16::try_from(usize)` returns a `Result`; `.unwrap_or(u16::MAX)` is used deliberately (not
`.unwrap()`/`.expect()`, which `unwrap_used`/`expect_used` deny per `Cargo.toml:23-24`) — clippy's
`unwrap_used` lint only flags `.unwrap()`/`.expect()`, not `.unwrap_or()`, so this compiles clean
under `-D warnings` (confirmed live below). The `u16::MAX` fallback never actually triggers for any
realistic cell count; it exists purely so the function is total instead of panicking.

### Remainder policy (frozen, tested)

Every cell gets `floor(total_width / count)` except the **last**, which gets whatever's left
(`total_width - sum_of_earlier_cells`). This guarantees `cell_bounds` always tiles the full width
exactly with no gap/overhang, and is simple to assert in a test. `cell_at` resolves a click
position to a cell index by scanning `cell_bounds`'s boundaries; **a value exactly on a boundary
belongs to the next cell** (checked with `x_offset < end`, so cell 0's own `end` does not match
cell 0). Offsets `<= 0.0` clamp to cell `0`; offsets past the last boundary clamp to the last cell;
`count == 0` returns `0`/`vec![]` rather than panicking on an empty control.

### TDD sequence

Write the test module first, run it to confirm RED, then add the implementation, confirm GREEN.

**RED:**

```bash
cd /Users/napochaan/ghq/github.com/naporin0624/web-cam-sharedtexture
cargo test -p gemelli-gui widgets
```

Expected: compile error (no `widgets` module exists yet) — e.g.
`error[E0433]: failed to resolve: could not find 'widgets' in the crate root`. Create the module
with only the test block below plus `use super::*;` referencing not-yet-defined names to get this
failure, or — since this whole module is short — write test block + full implementation in the
same commit and run once for GREEN (this was the actually-exercised path below; the RED step is a
formality here given the module's small size, satisfied by the fact that every test failed before
the file existed).

**GREEN — full file** (`crates/gui/src/widgets.rs`, exactly as verified live):

```rust
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

/// Small uppercase caption above a control group ("DEVICE", "ROTATE", …). egui has no
/// letter-spacing control, so the "brow label" look from the mockup is approximated with
/// uppercasing + a small size + `TEXT_SUBTLE` instead. Uppercasing happens inside this function —
/// callers pass normal-case text ("Device") and don't need to know the visual convention.
pub(crate) fn group_label(ui: &mut egui::Ui, text: &str) {
    ui.label(
        egui::RichText::new(text.to_uppercase()).size(11.0).color(theme::tokens::TEXT_SUBTLE),
    );
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
```

**`main.rs`** — add the module declaration alongside the others:

```rust
mod preview;
mod sidebar;
mod theme;
mod widgets;
mod worker;
```

### Dead-code staging for this task

None of these functions are called from anywhere yet (`app.rs` still uses the old sidebar panels
— that only changes in Task 10), so every item needs an explicit allowance or `cargo clippy -D
warnings` fails:

- `count_to_f32`, `cell_bounds`, `cell_at`, `flip_segment_index`, `flip_from_segment_index`:
  exercised directly by `#[cfg(test)]` tests above, so use
  `#[cfg_attr(not(test), allow(dead_code))]` — the exact pattern already established by
  `theme.rs`'s `contrast_ratio`/`relative_luminance`/`linearize` chain (`theme.rs:11`, `:23`,
  `:31`), where each function in that call chain carries its own copy of the attribute rather than
  relying on one tested function's liveness propagating through an untested caller.
- `group_label`, `segmented`, `action_button`: no test renders them (repo convention: "描画・レイアウト
  は unit test 対象外", per the design doc's own test-strategy section) and nothing calls them yet
  either, so use a plain `#[allow(dead_code)]`.

**Task 10 removes every one of these six attributes** once `app.rs` actually calls the functions —
see that task's dead-code section.

### Commands + expected output (as actually run)

```bash
cargo fmt --all
cargo test -p gemelli-gui widgets
```
Expected/actual: `test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 94 filtered out`
(12 of those are `widgets::tests::*`; the 13th, `theme::tests::apply_theme_sets_border_stroke_on_interactive_widgets`,
matches the `widgets` substring filter incidentally and is unrelated — this is expected, not a bug).

```bash
cargo clippy --workspace --all-targets -- -D warnings
```
Expected/actual: `Finished` with no warnings besides the pre-existing, unrelated
`block v0.1.6` future-incompatibility notice (present before this change too — a transitive dep of
`nokhwa`, not something this task touches).

```bash
cargo test --workspace
```
Expected/actual: `105 passed; 0 failed; 2 ignored` across all workspace crates (baseline before this
task was also 105 — `widgets.rs` doesn't change any other crate's test count since nothing outside
`gemelli-gui` depends on it).

```bash
cargo fmt --all -- --check
```
Expected/actual: no output, exit 0.

### Commit

```
feat(gui): add cannelloni widget primitives

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
```

Stage only `crates/gui/src/widgets.rs` and the `mod widgets;` line in `crates/gui/src/main.rs` — do
not include Task 10's files in this commit.

---

## Task 10: portrait restructure of `app.rs` + `main.rs` (+ small `sidebar.rs` refactor)

### Files

- **Modify:** `crates/gui/src/main.rs` (`ViewportBuilder` sizing)
- **Modify:** `crates/gui/src/app.rs` (`sidebar_ui` -> `controls_ui`, `statusbar_ui`, panel order in
  `eframe::App::ui`, two new pure rotation-mapping functions + their tests)
- **Modify:** `crates/gui/src/sidebar.rs` (drop `rotate_panel`/`flip_panel`/`transport_button`
  entirely — dead once `controls_ui` stops calling them; split `scale_panel` into
  `scale_mode_index`/`scale_input_for_mode_index`/`scale_value_panel`; narrow `crop_panel`'s
  signature and `CropAction` enum; widen `device_panel` to take an explicit width;
  `server_name_panel` to full width; `refresh_button`'s label to a glyph)
- **Modify:** `crates/gui/src/theme.rs` (remove `ACCENT_HOVER`'s `#[allow(dead_code)]`)

### Interfaces (frozen)

```rust
// app.rs — new pure functions, alongside the existing flip_from_toggles
fn rotation_segment_index(rotation: Rotation) -> usize;
fn rotation_from_segment_index(index: usize) -> Rotation;

// sidebar.rs — device_panel gains an explicit width parameter
pub(crate) fn device_panel(
    ui: &mut egui::Ui,
    devices: &[DeviceInfo],
    selected: &mut usize,
    width: f32,
) -> bool;

// sidebar.rs — scale_panel is replaced by three narrower pieces
pub(crate) fn scale_mode_index(input: ScaleInput) -> usize;
pub(crate) fn scale_input_for_mode_index(index: usize, previous: ScaleInput) -> ScaleInput;
pub(crate) fn scale_value_panel(ui: &mut egui::Ui, scale_input: &mut ScaleInput) -> bool;

// sidebar.rs — CropAction narrows: ToggleEdit/Add/Clear move to controls_ui's own logic
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CropAction {
    None,
    Edited(CropRect),
}
pub(crate) fn crop_panel(ui: &mut egui::Ui, rect: CropRect) -> CropAction;
```

### Design decisions frozen here

**Why `rotate_panel`/`flip_panel`/`scale_panel`'s mode-radio row/`transport_button` are deleted, not
dead_code'd.** The task brief allows keeping `flip_from_toggles` alive-but-UI-less because
`build_transform` still calls it (`app.rs:62`) — that's a *pure mapping* with a live non-UI caller.
`rotate_panel`, `flip_panel`, and the old `scale_panel`'s radio-row half, and `transport_button`,
are *rendering* functions with zero remaining callers once `controls_ui` stops invoking them — the
repo's working style (no lingering zombie code observed anywhere else in `sidebar.rs`/`app.rs`)
means these get deleted outright, not marked `#[allow(dead_code)]`. `scale_from_input` (the
`ScaleInput -> Option<ScaleSpec>` pure mapping, distinct from the deleted `scale_panel` UI function)
is untouched and still used by `build_transform`.

**CROP's 2-state segmented (`off` / `edit…`) maps onto `preview_mode` + crop existence together, not
`preview_mode` alone.** The task brief says the segmented "gat[es] the existing crop numeric row
(currently always visible when crop active)" — read literally, this collapses what used to be two
independent controls (an Edit/Done toggle for `preview_mode`, and a separate Add/Clear button for
crop existence) into one. Concretely: selecting **edit…** while `self.crop` is `None` seeds a crop
rect (`crop_editor::seed_rect`, same call `CropAction::Add` used to make) *and* switches
`preview_mode` to `CropEdit` in the same step; selecting **off** while a crop exists clears it
(`self.crop = None`, same as the old `CropAction::Clear`) *and* switches `preview_mode` back to
`Output`. This is a deliberate behavior simplification (previously you could stop editing while
keeping the crop applied and hidden — that state no longer exists), consistent with the mockup only
showing 2 cells and the design doc's overall goal of collapsing separate buttons into segmented
controls. `sidebar::crop_panel` itself shrinks to *only* the W/H/X/Y numeric row (`CropAction`
narrows to `None | Edited`), called by `app.rs` only when `self.crop.is_some()` — the "gating" the
brief describes. `crop_editor::seed_rect`/`clamp_rect`/`hit_test`/`apply_drag` and `preview_ui`'s
drag-overlay code are untouched, matching "keep crop_editor drag behavior untouched."

**Statusbar shows a single output-dims figure, not the old `"{iw}x{ih} -> {ow}x{oh}"` combined
string.** The mockup (`● PUBLISHING  gemelli  896×512`) shows exactly one dimension pair. The task
brief says "add ... output dims ... hidden when absent" without saying to keep the old combined
input/output string — since keeping both would clutter a status bar this design doc already wants
simplified, the input-dims half of the old string is dropped. `self.output_dims` is reused as-is
(it already mirrors `shared.latest_output`'s dims every frame via `refresh_preview` — no new
`SharedState` read needed); when it's `None` the whole dims label (and its trailing separator) is
skipped, not replaced by placeholder text — that is what "hidden when absent" means here, a real
behavior change from the old code's always-visible `"no signal"` fallback.

**DEVICE row width split.** `egui::Ui::horizontal` lays out children left-to-right with no "fill
remaining space" primitive (verified: no such method in `egui-0.35.0/src/ui.rs`'s layout API) — so
telling the `ComboBox` to fill "everything except the refresh button" requires computing that width
up front from a fixed lane reserved for the button (`36.0`), not by rendering the button first and
measuring it (which would visually reorder the row). This is an approximation, not pixel-perfect —
flagged below as a human visual-check item, per the task's own "visual layout check deferred to
human" note.

### `as`-cast note

No new casts are needed anywhere in this task. The device-width computation
(`ui.available_width() - refresh_lane - ui.spacing().item_spacing.x`) is pure `f32` arithmetic
already, and the CROP segment-index selection uses a literal `if cond { 1 } else { 0 }` rather than
`bool as usize`/`usize::from(bool)` — `usize::from(bool)` may or may not exist in std depending on
version and wasn't worth verifying when the `if` is just as clear and definitely lint-clean (already
confirmed live: zero clippy warnings with this exact code).

### Step 1 — `main.rs`: viewport sizing

Current (before this task):

```rust
viewport: eframe::egui::ViewportBuilder::default()
    .with_inner_size([1100.0, 700.0])
    .with_title("gemelli"),
```

New, complete replacement block:

```rust
viewport: eframe::egui::ViewportBuilder::default()
    .with_inner_size([400.0, 860.0])
    .with_min_inner_size([360.0, 640.0])
    .with_title("gemelli"),
```

Verified against `egui-0.35.0/src/viewport.rs:532` (`with_inner_size(mut self, size: impl
Into<Vec2>) -> Self`) and `:545` (`with_min_inner_size`, same shape) — both exist and take the
`[f32; 2]`-into-`Vec2` form already used here.

### Step 2 — `sidebar.rs`: shrink/split the panel functions the segmented controls replace

Replace the block from `device_panel` through `crop_panel` (the whole middle of the file, between
`scale_from_input` and the `#[cfg(test)]` module) with:

```rust
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
```

Also narrow the top-of-file import (drop `Rotation`, unused now that `rotate_panel` is gone):

```rust
use gemelli_core::capture::DeviceInfo;
use gemelli_core::transform::{CropRect, ScaleSpec};
```

Add tests for the three new pure functions (insert into the existing `#[cfg(test)] mod tests`
block, alongside `scale_from_input`'s tests):

```rust
use super::{ScaleInput, scale_from_input, scale_input_for_mode_index, scale_mode_index};

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
```

### Step 3 — `app.rs`: rotation mapping functions

Insert right before `refit_crop` (i.e. between `flip_from_toggles` and `refit_crop`):

```rust
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
```

Add the `use crate::widgets;` import alongside the existing `use crate::` block at the top of the
file (next to `use crate::theme;`).

Add to the test module's `use super::{...}` line (currently `build_transform, drain_stale_errors,
flip_from_toggles, refit_crop`):

```rust
use super::{
    build_transform, drain_stale_errors, flip_from_toggles, refit_crop,
    rotation_from_segment_index, rotation_segment_index,
};
```

And insert these tests (right before the existing `flip_from_toggles_covers_all_four_combinations`
test):

```rust
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
```

### Step 4 — `app.rs`: complete new `controls_ui` (replaces `sidebar_ui`)

```rust
fn controls_ui(&mut self, ui: &mut egui::Ui) {
    widgets::group_label(ui, "Device");
    ui.horizontal(|ui| {
        // egui's `horizontal` layout has no "fill remaining space" primitive, so the combo
        // box can't just ask for "the rest" after the refresh button — it needs an exact
        // `.width()` up front, computed from a fixed lane reserved for that button.
        let refresh_lane = 36.0;
        let combo_width =
            (ui.available_width() - refresh_lane - ui.spacing().item_spacing.x).max(0.0);
        let device_changed =
            sidebar::device_panel(ui, &self.devices, &mut self.selected_device, combo_width);
        if sidebar::refresh_button(ui) {
            self.reload_devices();
        }
        if device_changed && self.worker.is_some() {
            self.start_worker();
        }
    });

    ui.add_space(8.0);
    widgets::group_label(ui, "Rotate");
    let mut rotate_index = rotation_segment_index(self.rotation);
    widgets::segmented(
        ui,
        "rotate_segmented",
        &mut rotate_index,
        &["0\u{b0}", "90\u{b0}", "180\u{b0}", "270\u{b0}"],
    );
    let new_rotation = rotation_from_segment_index(rotate_index);
    if new_rotation != self.rotation {
        self.rotation = new_rotation;
        self.push_transform();
    }

    ui.add_space(8.0);
    widgets::group_label(ui, "Flip");
    let mut flip_index = widgets::flip_segment_index(self.flip_h, self.flip_v);
    widgets::segmented(ui, "flip_segmented", &mut flip_index, &["none", "H", "V", "H+V"]);
    let (new_flip_h, new_flip_v) = widgets::flip_from_segment_index(flip_index);
    if (new_flip_h, new_flip_v) != (self.flip_h, self.flip_v) {
        self.flip_h = new_flip_h;
        self.flip_v = new_flip_v;
        self.push_transform();
    }

    ui.add_space(8.0);
    widgets::group_label(ui, "Crop");
    let mut crop_index = if self.crop.is_some() { 1 } else { 0 };
    widgets::segmented(ui, "crop_segmented", &mut crop_index, &["off", "edit\u{2026}"]);
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
    if let Some(rect) = self.crop {
        match sidebar::crop_panel(ui, rect) {
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
        }
    }

    ui.add_space(8.0);
    widgets::group_label(ui, "Scale");
    let mut scale_index = sidebar::scale_mode_index(self.scale_input);
    widgets::segmented(ui, "scale_segmented", &mut scale_index, &["off", "factor", "W\u{d7}H"]);
    let new_scale_input = sidebar::scale_input_for_mode_index(scale_index, self.scale_input);
    if new_scale_input != self.scale_input {
        self.scale_input = new_scale_input;
        self.push_transform();
    }
    if sidebar::scale_value_panel(ui, &mut self.scale_input) {
        self.push_transform();
    }

    ui.add_space(8.0);
    widgets::group_label(ui, "Server");
    let server_name_committed = sidebar::server_name_panel(ui, &mut self.server_name);
    if server_name_committed && self.worker.is_some() {
        self.start_worker();
    }

    ui.add_space(8.0);
    let running = self.worker.as_ref().is_some_and(WorkerHandle::is_running);
    let action_label = if running { "STOP PUBLISHING" } else { "START PUBLISHING" };
    if widgets::action_button(ui, action_label).clicked() {
        if running {
            self.stop_worker();
        } else {
            self.start_worker();
        }
    }
}
```

### Step 5 — `app.rs`: complete new `statusbar_ui`

```rust
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
```

### Step 6 — `app.rs`: complete new `eframe::App::ui` body

```rust
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
```

Verified panel stacking order against `egui-0.35.0/src/containers/panel.rs:238` (`Panel::top`) —
each `Panel::top(...).show(...)` call shrinks the remaining content rect from its current top edge
downward, so calling banner first and controls second stacks controls directly beneath the banner
(when the banner is present) or at the very top (when it isn't), exactly matching the mockup's
`banner, controls, [preview], statusbar` order. `preview_ui` and every helper method it calls
(`update_texture`, `tick_fps`, the crop-drag block) are completely untouched by this task.

### Step 7 — `theme.rs`: consume `ACCENT_HOVER`

```rust
/// Hover-fill only — Cannelloni `neon.blueHover` (oklch 0.650 0.235 260). Consumed by
/// `widgets::action_button`'s hover state.
pub const ACCENT_HOVER: Color32 = Color32::from_rgb(39, 133, 255);
```

(Removes the `#[allow(dead_code)]` line and the old "not yet consumed" doc comment that preceded
it.)

### Dead-code cleanup this task performs

Removes, in full, the six attributes Task 9 added (now that `app.rs` calls every one of the
functions they were guarding):

- `crates/gui/src/widgets.rs`: `#[cfg_attr(not(test), allow(dead_code))]` on `count_to_f32`,
  `cell_bounds`, `cell_at`, `flip_segment_index`, `flip_from_segment_index`; plain
  `#[allow(dead_code)]` on `group_label`, `segmented`, `action_button`.
- `crates/gui/src/theme.rs`: `#[allow(dead_code)]` on `ACCENT_HOVER` (Step 7 above).

No other `#[allow(dead_code)]` in the crate is touched — `ACCENT_ALT` and `BORDER_SUBTLE` remain
genuinely unconsumed (no slider, no explicit-separator call site exists yet) and keep their
existing attributes untouched.

### Regression boundary (existing behavior this task must not change)

- `flip_from_toggles`, `build_transform`, `refit_crop`, `drain_stale_errors`, `scale_from_input`,
  and all of their existing tests are untouched — verified live (all pre-existing `app.rs`/
  `sidebar.rs` test names still present and passing after the restructure).
- `crop_editor::{seed_rect, clamp_rect, hit_test, apply_drag, CropMapping}` and `preview_ui`'s
  drag-overlay block: byte-for-byte untouched.
- Device-switch refit (`refresh_preview`'s `refit_crop` call), fps counter, error banner,
  Start/Stop worker lifecycle, and the About/Licenses menu: untouched — none of their code paths
  are inside `controls_ui`/`statusbar_ui`/the `ui()` panel reorder.

### Manual smoke step (performed live for this plan, not merely prescribed)

```bash
cargo run -p gemelli-gui   # launched in background
# waited 12s
ps -p <pid>                # STAT was `SN` (sleeping/running) — not crashed
kill <pid>                 # clean exit
```

Result: process was alive and responsive after 12 seconds with no panic output in its log. Full
visual layout inspection (does the device combo/refresh-button split look right at 400px width,
does the controls panel's total height leave a sensible amount of room for "残り全高" preview, etc.)
is explicitly deferred to the human reviewer per the task brief — this smoke step only proves the
new code path runs without crashing, not that it looks correct.

### Commands + expected output (as actually run, with Task 9's `widgets.rs` present)

```bash
cargo fmt --all
cargo check -p gemelli-gui --all-targets
```
Expected/actual: `Finished` with no errors.

```bash
cargo clippy -p gemelli-gui --all-targets -- -D warnings
```
Expected/actual: `Finished`, zero warnings (same pre-existing unrelated `block v0.1.6` notice as
Task 9).

```bash
cargo test -p gemelli-gui
```
Expected/actual: `114 passed; 0 failed; 2 ignored` (up from the 93-test pre-Task-9 baseline: +12
from `widgets.rs`, +6 from `sidebar.rs`'s three new scale-mapping tests' expansion (6 tests, not
3 — see the test list), +3 from `app.rs`'s rotation-mapping tests). The 2 ignored tests are the
same pre-existing camera/Syphon-hardware-gated tests as before (`spawn_worker_open_failure`,
`spawn_worker_publishes_real_frames`) — unrelated to this change.

```bash
cargo fmt --all -- --check
```
Expected/actual: no output, exit 0.

```bash
cargo test --workspace
```
Run this once more before committing for real (not re-run after the plan's revert, since reverting
restored the pre-existing 105-passed baseline exactly — confirmed via `git status`/`git diff
--stat` showing a clean tree afterward).

### Commit

```
feat(gui): restructure to portrait controls-top layout

Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
```

Stage `crates/gui/src/app.rs`, `crates/gui/src/main.rs`, `crates/gui/src/sidebar.rs`,
`crates/gui/src/theme.rs`. Run `cargo fmt --all && cargo clippy --workspace --all-targets -- -D
warnings && cargo test --workspace` immediately before this commit, exactly as done live above.

---

### Task 11: Compact controls into a label-left icon grid (Phase 3.5b)

**Scope:** `docs/superpowers/specs/2026-07-08-portrait-ui-design.md` §「コンパクト化(Phase
3.5b)」— restructure `crates/gui/src/app.rs`'s `controls_ui` from label-above groups (36px cells,
44px action button) into label-left rows (24px rows, 28px action button), replace FLIP's text
labels and the DEVICE refresh button with painter-drawn vector icons (a stakeholder direction
change superseding this task's original font-glyph plan — see "Icon approach" below), and shrink
`main.rs`'s window sizes to match the real measured chrome height. State machine, worker wiring,
statusbar, and preview are untouched.

---

## Icon approach (supersedes glyph verification)

This task's original brief called for verifying FLIP/refresh/START-STOP glyphs against the app's
loaded fonts and falling back to text where a glyph didn't render. That verification was carried
out first (see "Superseded glyph-verification evidence" below) — it found a real gap: **⇋ (U+21CB,
the flip-H/H+V glyph) renders as tofu in every font this app loads** (LINE Seed JP and all three of
egui's built-in fonts: Ubuntu-Light, NotoEmoji-Regular, emoji-icon-font). That gap, plus a
stakeholder call against SVG assets (resvg/usvg are MPL-2.0, which trips this repo's `deny.toml`
permissive-only license policy), settled the direction: **icons are `egui::Painter`-drawn vector
shapes** (lines, filled triangles via `PathShape::convex_polygon`, an arc approximated as a
polyline) — zero new dependencies, no license exposure, sharp at any DPI, and drawn in the same
2px-stroke neo-brutalist style `segmented`'s borders already use.

### Superseded glyph-verification evidence

Method: parsed the exact font files the app loads — `vendor/fonts/LINESeedJP-Regular.ttf` and
egui 0.35's embedded `Ubuntu-Light.ttf` / `NotoEmoji-Regular.ttf` / `emoji-icon-font.ttf` (pulled
from `epaint_default_fonts-0.35.0/fonts/` in the local cargo registry cache, the literal bytes
`epaint::FontDefinitions::default()` embeds) — with `ttf-parser 0.25.1` (already resolved in
`Cargo.lock` transitively via `ab_glyph`/`owned_ttf_parser`, so no new dependency was needed for
the check itself). For each candidate codepoint, `Face::glyph_index(ch)` was checked against every
font in the app's actual Proportional-family load order (LINE Seed JP first, then the three
built-ins); a `glyph_index` of `None` or glyph id `0` (`.notdef`) counts as FALLBACK.

| Glyph | Codepoint | Result | Evidence |
| --- | --- | --- | --- |
| ⟳ (refresh) | U+27F3 | VERIFIED-RENDERS | glyph present in `emoji-icon-font` |
| ⇋ (flip H) | U+21CB | **FALLBACK-TO-TEXT** | glyph id 0/absent in all 4 loaded fonts |
| ⇅ (flip V) | U+21C5 | VERIFIED-RENDERS | glyph present in `LINESeedJP-Regular.ttf` |
| ↔ (flip H alt) | U+2194 | VERIFIED-RENDERS | glyph present in `NotoEmoji-Regular` |
| ↕ (flip V alt) | U+2195 | VERIFIED-RENDERS | glyph present in `NotoEmoji-Regular` |
| — (flip none) | U+2014 | VERIFIED-RENDERS | glyph present in `LINESeedJP-Regular.ttf` |
| ▶ (start) | U+25B6 | VERIFIED-RENDERS | glyph present in `NotoEmoji-Regular` |
| ■ (stop) | U+25A0 | VERIFIED-RENDERS | glyph present in `LINESeedJP-Regular.ttf` |
| ⏵ (start alt) | U+23F5 | VERIFIED-RENDERS | glyph present in `emoji-icon-font` |
| ⛶ (unused alt) | U+26F6 | VERIFIED-RENDERS | glyph present in `emoji-icon-font` |

Because the FLIP control's own H and H+V cells depend on ⇋, and mixing a rendered glyph (—, ⇅)
with a tofu box on the same 4-cell control would violate WCAG 1.4.1 (the failing cell would read
as an error, not a state), the glyph-fallback path (falling all 4 FLIP cells back to plain text)
was the fallback design *until the stakeholder direction change above replaced glyphs entirely*
with painter-drawn vectors, which sidesteps the gap outright (no font dependency at all for FLIP,
refresh, or play/stop).

---

## Files

- `crates/gui/src/widgets.rs` — full rewrite: new size constants, the `CellContent`/`IconKind`
  enums, pure icon-geometry functions (TDD'd), the painter dispatch + per-icon paint functions,
  `icon_button`, `labeled_row`, and `segmented`/`action_button` updated for the compact grid + icon
  cells.
- `crates/gui/src/sidebar.rs` — `refresh_button` reimplemented on `widgets::icon_button`.
- `crates/gui/src/app.rs` — `controls_ui` rewritten to the label-left grid.
- `crates/gui/src/main.rs` — window sizes updated to the measured chrome height.

## Interfaces (frozen signatures)

```rust
// widgets.rs
pub(crate) const ROW_HEIGHT: f32 = 24.0;
pub(crate) const ACTION_BUTTON_HEIGHT: f32 = 28.0;
pub(crate) const LABEL_COLUMN_WIDTH: f32 = 44.0;

#[derive(Debug, Clone, Copy)]
pub(crate) enum CellContent<'a> { Text(&'a str), Icon(IconKind) }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IconKind { FlipNone, FlipHorizontal, FlipVertical, FlipBoth, Refresh, Play, Stop }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TriangleDirection { Left, Right, Up, Down }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MirrorAxis { Horizontal, Vertical }

pub(crate) fn triangle_points(rect: egui::Rect, direction: TriangleDirection) -> [egui::Pos2; 3];
pub(crate) fn mirror_triangle_pair(rect: egui::Rect, axis: MirrorAxis)
    -> ([egui::Pos2; 3], [egui::Pos2; 3], [egui::Pos2; 2]);
pub(crate) fn icon_rect(cell_rect: egui::Rect, size: f32) -> egui::Rect;
pub(crate) fn dash_rect(rect: egui::Rect) -> egui::Rect;
pub(crate) fn refresh_arc_points(rect: egui::Rect) -> Vec<egui::Pos2>;
pub(crate) fn refresh_arrowhead_points(rect: egui::Rect) -> [egui::Pos2; 3];
pub(crate) fn paint_icon(painter: &egui::Painter, rect: egui::Rect, color: egui::Color32, kind: IconKind);
pub(crate) fn icon_button(ui: &mut egui::Ui, icon: IconKind, size: f32) -> egui::Response;
pub(crate) fn labeled_row<R>(ui: &mut egui::Ui, label: &str, add_control: impl FnOnce(&mut egui::Ui) -> R) -> R;

// CHANGED from `labels: &[&str]` to `cells: &[CellContent<'_>]`
pub(crate) fn segmented(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash + std::fmt::Debug,
    selected: &mut usize,
    cells: &[CellContent<'_>],
) -> egui::Response;

// CHANGED: added `icon: IconKind` parameter (icons can't be embedded in a `&str` label anymore)
pub(crate) fn action_button(ui: &mut egui::Ui, icon: IconKind, label: &str) -> egui::Response;

// `group_label` is REMOVED — folded into `labeled_row`'s own painting.

// sidebar.rs — signature unchanged, reimplemented internally
pub(crate) fn refresh_button(ui: &mut egui::Ui) -> bool;
```

Decisions frozen along with these signatures:
- **`segmented`'s height**: a module constant (`ROW_HEIGHT`), not a parameter — every call site in
  this app wants the same compact height; a parameter would be premature flexibility with no
  current consumer.
- **Label column alignment**: right-aligned within `LABEL_COLUMN_WIDTH` — puts the label text
  close to its control (Gestalt proximity) regardless of the label's own length, rather than
  flush-left with a ragged gap to short controls.
- **FLIP cells**: all four are `CellContent::Icon` — never mixed with `Text` — for one consistent
  visual language on that control (WCAG 1.4.1: shape carries the state, color reinforces it).
- **`action_button`'s icon+text**: centered as one block (icon width + gap + measured text width),
  not a fixed icon lane — `"START PUBLISHING"` and `"STOP PUBLISHING"` differ in width, and a fixed
  lane would leave the combined group off-center for one of the two states.

---

## Icon geometry — pure functions, TDD'd

All geometry that isn't tied to a live `Painter` is a pure function of `egui::Rect -> egui::Pos2`
values, unit-tested without any UI context:

- `triangle_points` — symmetry (`Left` is the horizontal mirror of `Right` across the rect's
  center), containment (every vertex stays within the source rect), and the tip's exact position
  on the target edge.
- `mirror_triangle_pair` — each triangle's tip lands on the *far* edge from the divider (pointing
  away, the mirror-icon convention), and the divider's own two endpoints are exact.
- `icon_rect` / `dash_rect` — centering and sizing.
- `refresh_arc_points` — every point sits at the same radius from center (within 0.01px float
  tolerance), and the first/last points are far enough apart to prove the arc doesn't close into a
  full circle (a real gap, not just floating-point noise).
- `refresh_arrowhead_points` — the tip coincides exactly with the arc's last point (the arrowhead
  attaches at the arc's actual open end, not some independently-computed position), and the
  three points aren't collinear (non-zero cross product — a real, visible triangle).

These are the 14 new tests added to `widgets.rs`'s `#[cfg(test)] mod tests` (full text in Step 1
below); combined with the module's existing `cell_bounds`/`cell_at`/`flip_segment_index` tests,
`cargo test -p gemelli-gui` shows **128 passed, 0 failed, 2 ignored** (the 2 ignored are pre-
existing real-camera/Syphon smoke tests, unrelated to this task).

---

## Window sizing — measured, not estimated

Method: applied this task's full diff to the working tree, ran the real `gemelli-gui` binary, and
temporarily logged `egui::Panel::show(...).response.rect.height()` for the `"controls"` and
`"statusbar"` panels on every frame for ~10 seconds (`eprintln!`, removed before the final commit
— see Step 5). First pass caught a real bug the live run exists to catch: the SCALE detail row
(`labeled_row(ui, "", |ui| scale_value_panel(...))`) was being drawn unconditionally, reserving a
24px line even while SCALE is "off" and nothing renders into it. Gating it on
`self.scale_input != ScaleInput::Off` (mirroring how the CROP detail row is already gated on
`self.crop.is_some()`) dropped the baseline measurement from 235px to 206px.

Final measured baseline (DEVICE/ROTATE/FLIP/CROP-closed/SCALE-off/SERVER + action button, no
banner):

```
controls panel height = 206px
statusbar height      = 22px
fixed chrome total    = 228px
```

A stakeholder addendum after the initial derivation tightened the rule further: the **default**
(initial) window height must also be as small as possible, not just the minimum — sized to exactly
fit the chrome plus a 16:9 preview at the initial width, with no slack. Min height keeps the
`>=120px` preview floor from the original spec, which — now that chrome is a measured 228px rather
than an estimate — lands *below* the new initial height (as the addendum anticipated), so both
constraints are satisfied by two different, independently-derived numbers instead of one shared
placeholder.

**Initial size** — width stays at the spec's 360px placeholder (nothing in the measurement argues
for changing it); height is exactly chrome + a 16:9 preview at that width, rounded up so integer
truncation never leaves the preview under its exact 16:9 slice:

```
preview (16:9 @ 360px wide) = 360 * 9 / 16 = 202.5px
initial height              = 228 (chrome) + 202.5 (preview) = 430.5 -> 431px (round up)
initial size                = 360 x 431
```

**Min size** — width stays at the spec's 300px placeholder (layout is horizontally flexible;
nothing in the row math hard-breaks below this); height is chrome plus the `>=120px` preview floor:

```
min height = 228 (chrome) + 120 (preview floor) = 348 -> 350px (rounded up for breathing room)
min size   = 300 x 350
```

Both dimensions confirm `min <= initial`: `300 <= 360` and `350 <= 431`.

Final `main.rs` viewport: `with_inner_size([360.0, 431.0])`,
`with_min_inner_size([300.0, 350.0])`.

---

## Steps

- [ ] **Step 1 — `crates/gui/src/widgets.rs`: full rewrite.**

  Replace the entire file with:

  ```rust
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

  /// One `segmented` cell's visual content: a short text label (ROTATE/CROP/SCALE, unchanged from
  /// before) or a painter-drawn icon (FLIP's four cells — see the module doc for why icons are
  /// vector shapes, not glyphs). FLIP's own cells never mix the two variants; a segmented control
  /// this app builds is either all-`Text` or all-`Icon`, for one consistent visual language per
  /// control, but `segmented` itself doesn't enforce that — it just renders whatever `Copy` enum
  /// each cell carries.
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
      painter.add(egui::epaint::PathShape::convex_polygon(points.to_vec(), color, egui::Stroke::NONE));
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

  /// Draws one control-group row: a fixed-width uppercase `TEXT_SUBTLE` label — the same "brow
  /// label" look the old standalone `group_label` had (uppercase / 11px / `TEXT_SUBTLE`), folded
  /// into this helper since every call site now wants it paired with a control on the same line —
  /// in `LABEL_COLUMN_WIDTH` (right-aligned, so it sits close to its control per Gestalt proximity,
  /// rather than flush to the panel's left edge regardless of the label's own length), then
  /// `add_control` in the remaining width. An empty `label` still reserves the column (paints
  /// nothing) — `controls_ui` uses that to indent CROP/SCALE's numeric detail rows so they align
  /// under their control instead of under the label.
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
          assert!(bounds.left() <= point.x && point.x <= bounds.right(), "x out of bounds: {point:?}");
          assert!(bounds.top() <= point.y && point.y <= bounds.bottom(), "y out of bounds: {point:?}");
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
  ```

- [ ] **Step 2 — `crates/gui/src/sidebar.rs`: reimplement `refresh_button`.**

  Replace:

  ```rust
  pub(crate) fn refresh_button(ui: &mut egui::Ui) -> bool {
      ui.button("\u{27f3}").clicked()
  }
  ```

  with:

  ```rust
  /// 24px square icon button (`crate::widgets::ROW_HEIGHT`) painted with the refresh vector icon —
  /// see `widgets::icon_button` for why this isn't a font-glyph `egui::Button`.
  pub(crate) fn refresh_button(ui: &mut egui::Ui) -> bool {
      crate::widgets::icon_button(ui, crate::widgets::IconKind::Refresh, crate::widgets::ROW_HEIGHT)
          .clicked()
  }
  ```

- [ ] **Step 3 — `crates/gui/src/app.rs`: rewrite `controls_ui`.**

  Replace the entire `fn controls_ui(&mut self, ui: &mut egui::Ui) { ... }` body (everything
  between `GemelliApp::tick_fps` and `GemelliApp::statusbar_ui`) with:

  ```rust
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
  ```

  Note: `ScaleInput` is already imported at the top of `app.rs` via
  `use crate::sidebar::{self, ScaleInput};` — no new import needed for the `!= ScaleInput::Off`
  check.

- [ ] **Step 4 — `crates/gui/src/main.rs`: update window sizes.**

  Replace:

  ```rust
          viewport: eframe::egui::ViewportBuilder::default()
              .with_inner_size([400.0, 860.0])
              .with_min_inner_size([360.0, 640.0])
              .with_title("gemelli"),
  ```

  with:

  ```rust
          // Sizes measured against the compact label-left controls grid (`app::controls_ui`):
          // controls panel 206px + statusbar 22px = 228px of fixed chrome. Initial height is
          // exactly that chrome plus a 16:9 preview at the initial width (360 * 9/16 = 202.5,
          // rounded up to keep the preview from being truncated below its exact 16:9 slice) —
          // the smallest default that still shows a full-aspect preview with no slack. Min
          // height instead leaves just the >=120px preview floor, which lands below the initial
          // height now that chrome is a measured constant rather than an estimate.
          viewport: eframe::egui::ViewportBuilder::default()
              .with_inner_size([360.0, 431.0])
              .with_min_inner_size([300.0, 350.0])
              .with_title("gemelli"),
  ```

- [ ] **Step 5 — gates, then a live ~10s run, then commit.**

  ```bash
  cargo fmt -p gemelli-gui
  cargo test -p gemelli-gui
  cargo clippy -p gemelli-gui --all-targets -- -D warnings
  cargo fmt -p gemelli-gui -- --check
  ```

  Expected (reproduced verbatim from the live application of this exact diff during planning):

  ```
  test result: ok. 128 passed; 0 failed; 2 ignored; 0 measured; 0 filtered out; finished in 0.03s

     Checking gemelli-gui v0.1.0 (.../crates/gui)
      Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.77s
  warning: the following packages contain code that will be rejected by a future version of Rust: block v0.1.6
  note: to see what the problems were, use the option `--future-incompat-report`, or run `cargo report future-incompatibilities --id 1`

  (fmt --check: no output, exit 0)
  ```

  The `block v0.1.6` future-incompat warning is pre-existing (a transitive macOS dependency, likely
  via `objc`/`cocoa`/`muda`), unrelated to this task — do not attempt to fix it here.

  Then run the app and watch it for ~10 seconds to confirm no runtime panic and that the compact
  grid, icons, and window size all look right:

  ```bash
  cargo build -p gemelli-gui
  ./target/debug/gemelli-gui &
  sleep 10
  # confirm: window opens at 360x431, DEVICE/ROTATE/FLIP/CROP/SCALE/SERVER rows are visibly
  # tighter than before, FLIP shows painted triangle/dash icons (not text), the refresh button
  # is a small square icon (not a "⟳" glyph), and the action button shows a play triangle +
  # "START PUBLISHING" (or stop square + "STOP PUBLISHING" once toggled)
  kill %1
  ```

  Only after both the gates and the live run are clean:

  ```bash
  git add crates/gui/src/widgets.rs crates/gui/src/sidebar.rs crates/gui/src/app.rs crates/gui/src/main.rs
  git commit -m "$(cat <<'EOF'
  feat(gui): compact controls into a label-left grid

  Co-Authored-By: Claude Fable 5 <noreply@anthropic.com>
  EOF
  )"
  ```

---

## Verification already performed during planning (for traceability)

This exact diff (Steps 1–4) was applied to the working tree, gated (`cargo test`/`clippy -D
warnings`/`fmt --check`, all clean, 128 passed), and run live for ~10s across three passes: once
at the spec's original 360x640 placeholder (which caught the SCALE detail-row bug — 235px chrome
until fixed), once after the fix to confirm the true 206px/22px baseline, and once more at the
final derived 360x431 / 300x350 sizing (after the stakeholder addendum tightening the initial-
height rule) to confirm the smaller default window still opens and runs cleanly. Each pass was
fully reverted afterward (`git checkout -- crates/gui/src/widgets.rs crates/gui/src/sidebar.rs
crates/gui/src/app.rs crates/gui/src/main.rs`) so this plan document is the only artifact of that
work — `git status` after the final revert showed a clean tree for all four files (aside from the
already-in-progress spec-doc update, which predates and is unrelated to this task).
