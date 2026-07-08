# gemelli GUI (Phase 2) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** gemelli-core を使う egui GUI(gemelli-gui)— プレビューを見ながら crop/rotate/flip/scale をライブ調整し Syphon publish を制御する。

**Architecture:** capture 専用スレッド(NokhwaSource + SyphonPublisher をスレッド上で生成、ArcSwap<TransformConfig> snapshot → transform → publish → latest-frame 更新、エラーは mpsc で GUI へ)+ GUI スレッド(eframe/egui 60fps repaint、texture 更新、サイドバー編集、エラーバナー = 唯一の消費点)。crop はプレビュー上ドラッグ編集(CropEdit モードで生フレーム表示)。

**Tech Stack:** eframe/egui 0.35.0 / arc-swap 1.9.2 / gemelli-core + gemelli-syphon(Phase 1)

**Spec:** `docs/superpowers/specs/2026-07-08-gemelli-gui-design.md`

## Global Constraints

- ブランチ: `feature/gui`(main から)。conventional commits、task 単位 commit、push しない
- Rust edition 2024。clippy deny: `unwrap_used` / `expect_used` / `as_conversions`(unwrap/expect はテストのみ免除。**as_conversions はテストでも免除なし**)
- `as` キャストの許可箇所は「隔離ヘルパー + 文書化 `#[allow]`」パターンのみ(core の scale.rs 前例)。本 plan では preview.rs / fps_meter.rs / crop_editor.rs の各ヘルパーに限定 — 追加しない
- 全コードは `.claude/skills/` の 7 skills に従う(exhaustive match `_` 禁止、`?`/コンビネータ、guards、≤3語関数名)。Mutex は `.lock().unwrap_or_else(PoisonError::into_inner)`
- WorkerError の消費(表示)は GUI バナー 1 箇所のみ。core/worker は print しない
- t-wada TDD: 純関数は RED 確認 → 最小実装 → GREEN。egui ウィジェット層は手動チェックリスト
- lint gate per commit: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`
- WCAG 2.1 AA: theme tokens はテストで contrast_ratio ≥4.5(テキスト)/ ≥3.0(UI 状態)を数値 assert
- GUI に fps 指定ウィジェットは無し(requested_fps = None 固定、spec の UI 仕様どおり)

---
## Section A — Tasks 1–3 (crates/gui: scaffold, theme, preview/fps)

All paths below are repo-root relative (`/Users/napochaan/ghq/github.com/naporin0624/web-cam-sharedtexture`).
Run every command from the repo root. Test command form: `cargo test -p gemelli-gui <filter>`.
Lint gate (run after every green step, before every commit):
`cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`

**Dependency versions verified** (via `cargo add --dry-run <crate>` inside `crates/gui`, 2026-07-08):
`eframe = "0.35"` (resolves 0.35.0, default features: `accesskit`, `default_fonts`, `wayland`, `web_screen_reader`, `wgpu`, `x11`), `egui = "0.35"` (resolves 0.35.0, kept in lockstep with eframe so `crates/gui` can write bare `egui::Foo` paths per the contract instead of `eframe::egui::Foo`), `arc-swap = "1.9"` (resolves 1.9.2).

**`gemelli-gui` is a bin-only crate (no `src/lib.rs`)** — every `mod` below is declared from `main.rs`, and `cargo test -p gemelli-gui <filter>` runs the binary's own `#[cfg(test)]` modules directly (no `--lib` flag, matching the contract's test-command form).

**Verified against docs.rs (egui/eframe 0.35.0), recorded here because they shape the code below:**
- `egui::ColorImage::from_rgba_unmultiplied(size: [usize; 2], rgba: &[u8]) -> ColorImage`; fields `size: [usize; 2]`, `pixels: Vec<Color32>`; method `as_raw(&self) -> &[u8]`.
- `Color32::from_rgb` is `pub const fn`, so tokens can be `pub const` values. `Color32::{r,g,b,a}(&self) -> u8`.
- `egui::Context::default()` builds a headless context (no window/backend needed) — used directly in `apply_theme`'s unit test. `Context::set_visuals(&self, visuals: Visuals)`; current visuals are readable back via `ctx.global_style().visuals`.
- `Visuals` fields used: `dark_mode: bool`, `window_fill: Color32`, `panel_fill: Color32`, `override_text_color: Option<Color32>`, `weak_text_color: Option<Color32>`, `hyperlink_color: Color32`, `selection: Selection { bg_fill: Color32, stroke: Stroke }`. `Visuals::dark()` constructor exists.
- **`std::convert` gap that shapes every numeric cast below** (checked with `rustc` directly, not assumed): `usize: From<u32>` does **not** exist (only `From<u8>`/`From<u16>`), so `gemelli_core::frame::Frame`'s own `usize::try_from(width).unwrap_or(usize::MAX)` idiom (`crates/core/src/frame.rs`) is the correct precedent to reuse, not a `From` conversion. Separately, `f64: From<u32>` **does** exist (lossless — f64's 52-bit mantissa covers all of u32) but `f32: From<u32>` does **not** (f32's 24-bit mantissa can't cover all of u32), which is why `preview.rs` and `fps_meter.rs` each need one isolated, documented `as`-cast in the style of `crates/core/src/transform/scale.rs`'s `scale_dimension`.

---

### Task 1: gui crate scaffold

**Files:**
- Modify: `Cargo.toml` (root) — add `eframe`, `egui`, `arc-swap` to `[workspace.dependencies]`
- Modify: `crates/gui/Cargo.toml` — add the three deps + macOS-only `gemelli-syphon` path dep
- Create: `crates/gui/build.rs`
- Modify: `crates/gui/src/main.rs` — eframe bootstrap, `mod app;`
- Create: `crates/gui/src/app.rs` — `GemelliApp` placeholder

**Interfaces:**
- Consumes: `gemelli-core` (path dep, already wired in the existing skeleton; not used by any code yet — `Frame` arrives in Task 3), `gemelli-syphon` (macOS target dep, wired but unused until the `worker.rs` task), `DEP_SYPHON_BRIDGE_RPATH` build-script env var (same mechanism `crates/cli/build.rs` reads).
- Produces:
  ```rust
  // app.rs
  pub struct GemelliApp { /* empty in this task; fields land in the app.rs/sidebar.rs task */ }
  impl GemelliApp {
      pub fn new(cc: &eframe::CreationContext<'_>) -> Self;
  }
  impl eframe::App for GemelliApp {
      fn update(&mut self, ctx: &eframe::egui::Context, frame: &mut eframe::Frame);
  }
  ```

This task has no pure functions to TDD — it is infrastructure (dependency wiring, a build script, and an eframe bootstrap that opens a window). It is one sequence of scaffolding steps ending in a single commit, matching how Phase 1's Task 1 handled its own `Step 0 — workspace scaffolding (not a TDD cycle)`.

#### Step 0 — workspace scaffolding (not a TDD cycle)

- [ ] Edit `Cargo.toml` (root): add to `[workspace.dependencies]` (after the existing `ctrlc = "3.5"` line):
  ```toml
  eframe = "0.35"
  egui = "0.35"
  arc-swap = "1.9"
  ```
- [ ] Edit `crates/gui/Cargo.toml` to:
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
  arc-swap = { workspace = true }

  [target.'cfg(target_os = "macos")'.dependencies]
  gemelli-syphon = { path = "../syphon" }
  ```
- [ ] Create `crates/gui/build.rs` — same shape as `crates/cli/build.rs` (reads the rpath metadata `gemelli-syphon`'s build script publishes so the GUI binary can resolve `@rpath/Syphon.framework/...` at launch; see that file's comment for the full rationale, unchanged here):
  ```rust
  use std::process::ExitCode;

  fn main() -> ExitCode {
      match run() {
          Ok(()) => ExitCode::SUCCESS,
          Err(reason) => {
              eprintln!("crates/gui build.rs failed: {reason}");
              ExitCode::FAILURE
          }
      }
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
  fn run() -> Result<(), String> {
      let Ok(rpaths) = std::env::var("DEP_SYPHON_BRIDGE_RPATH") else {
          return Ok(());
      };

      for rel in rpaths.split(';').filter(|rel| !rel.is_empty()) {
          println!("cargo:rustc-link-arg=-Wl,-rpath,{rel}");
      }

      Ok(())
  }
  ```
- [ ] Run `cargo build -p gemelli-gui`. **Expect success**, but note this is the first pull of `eframe`'s dependency graph (`wgpu`, `winit`, `accesskit`, and friends — well over a hundred transitive crates); the *first* build after this step can take several minutes. Nothing uses `eframe`/`egui`/`arc-swap` yet (the old placeholder `main.rs` is still in place), so this step only proves the new deps resolve and compile — it is not yet a functional check.
- [ ] Replace `crates/gui/src/main.rs` with:
  ```rust
  //! GUI entry point for the webcam -> Spout/Syphon sharing tool.

  use std::process::ExitCode;

  mod app;

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
  ```
- [ ] Create `crates/gui/src/app.rs`:
  ```rust
  //! `GemelliApp`: the eframe root. This task only wires the window shell —
  //! device/transform/worker state lands in a later task.

  pub struct GemelliApp {}

  impl GemelliApp {
      pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
          Self {}
      }
  }

  impl eframe::App for GemelliApp {
      fn update(&mut self, ctx: &eframe::egui::Context, _frame: &mut eframe::Frame) {
          eframe::egui::CentralPanel::default().show(ctx, |ui| {
              ui.heading("gemelli");
          });
      }
  }
  ```
- [ ] Run `cargo build -p gemelli-gui`. Expect success — this time it actually compiles `app.rs`/`main.rs` against the `eframe`/`egui` APIs.
- [ ] Manual verification (cannot be scripted/asserted headlessly — this is the one non-automated check in this section): run `cargo run -p gemelli-gui` with a bounded lifetime, e.g. from the repo root:
  ```sh
  (cargo run -p gemelli-gui &) ; sleep 8 ; pkill -f target/debug/gemelli-gui
  ```
  Expect: a native window titled "gemelli" appears within a few seconds showing a centered "gemelli" heading on the (still-default, un-themed) background, then the process is killed. If the window doesn't appear, stop and diagnose before continuing to Task 2 — do not paper over a broken bootstrap with `#[allow]`s or by skipping the check.
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`.
- [ ] `git add Cargo.toml crates/gui/Cargo.toml crates/gui/build.rs crates/gui/src/main.rs crates/gui/src/app.rs`, commit:
  ```
  feat(gui): scaffold eframe bootstrap and syphon rpath build script
  ```

---

### Task 2: theme.rs

**Files:**
- Create: `crates/gui/src/theme.rs`
- Modify: `crates/gui/src/main.rs` — add `mod theme;`

**Interfaces:**
- Consumes: `egui::Color32`, `egui::Context`, `egui::Visuals` (0.35.0, verified above); no other crate code.
- Produces:
  ```rust
  pub fn contrast_ratio(a: egui::Color32, b: egui::Color32) -> f64;

  pub mod tokens {
      use egui::Color32;
      pub const BG_BASE: Color32;
      pub const BG_PANEL: Color32;
      pub const TEXT_PRIMARY: Color32;
      pub const TEXT_MUTED: Color32;
      pub const ACCENT_PUBLISH: Color32;
      pub const ACCENT_IDLE: Color32;
      pub const DANGER: Color32;
      pub const CROP_OVERLAY: Color32;
  }

  pub fn apply_theme(ctx: &egui::Context);
  ```

**Palette (picked in this task, arithmetic proved below and re-verified with a script before writing the assertions — see Return summary for the raw numbers):**

| token | hex | rgb |
|---|---|---|
| `BG_BASE` | `#1a1b1e` | (26, 27, 30) |
| `BG_PANEL` | `#212226` | (33, 34, 38) — deliberately a hair lighter than `BG_BASE` for panel/sidebar layering |
| `TEXT_PRIMARY` | `#e6e6e6` | (230, 230, 230) |
| `TEXT_MUTED` | `#a0a0a8` | (160, 160, 168) |
| `ACCENT_PUBLISH` | `#3ddc84` | (61, 220, 132) — green, "publishing" |
| `ACCENT_IDLE` | `#7d8590` | (125, 133, 144) — desaturated blue-gray, "idle" |
| `DANGER` | `#ff6b6b` | (255, 107, 107) |
| `CROP_OVERLAY` | `#ffffff` | (255, 255, 255) — opaque white; the contract notes this token's contrast against arbitrary live video is untestable, so the crop-rect stroke is drawn twice at the `crop_editor.rs` call site (a wider black stroke underneath a thinner white one) rather than relying on a single token's contrast ratio |

Verified ratios (WCAG 2.1 relative luminance, computed both by hand and cross-checked with a throwaway script before committing to these hex values):
- `TEXT_PRIMARY` vs `BG_BASE` = **13.80** (>= 4.5 required)
- `TEXT_MUTED` vs `BG_BASE` = **6.63** (>= 4.5 required)
- `DANGER` vs `BG_BASE` = **6.21** (>= 4.5 required)
- `ACCENT_PUBLISH` vs `BG_PANEL` = **8.91** (>= 3.0 required)
- `ACCENT_IDLE` vs `BG_PANEL` = **4.26** (>= 3.0 required)
- (sanity, not a contract requirement) `BG_PANEL` vs `BG_BASE` = 1.08, confirming panel really is lighter than base, not accidentally darker

#### Cycle 1 — `contrast_ratio`

- [ ] Write the failing test. Create `crates/gui/src/theme.rs` with just:
  ```rust
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
  }
  ```
- [ ] Add `mod theme;` to `crates/gui/src/main.rs` (below `mod app;`).
- [ ] Run `cargo test -p gemelli-gui theme::tests`. Expect **compile failure**: `error[E0425]: cannot find function `contrast_ratio` in this scope` (repeated per call site).
- [ ] Minimal implementation. Prepend to `theme.rs` (above the test module):
  ```rust
  //! WCAG 2.1 AA color tokens for the gemelli GUI, plus the contrast-ratio
  //! calculation used to prove them (see `tokens` below).

  use egui::Color32;

  /// WCAG 2.1 relative-luminance contrast ratio between two colors.
  /// Formula: <https://www.w3.org/TR/WCAG21/#dfn-contrast-ratio>.
  pub fn contrast_ratio(a: Color32, b: Color32) -> f64 {
      let luminance_a = relative_luminance(a);
      let luminance_b = relative_luminance(b);
      let (lighter, darker) =
          if luminance_a >= luminance_b { (luminance_a, luminance_b) } else { (luminance_b, luminance_a) };
      (lighter + 0.05) / (darker + 0.05)
  }

  fn relative_luminance(color: Color32) -> f64 {
      let red = linearize(color.r());
      let green = linearize(color.g());
      let blue = linearize(color.b());
      0.2126 * red + 0.7152 * green + 0.0722 * blue
  }

  fn linearize(channel: u8) -> f64 {
      let normalized = f64::from(channel) / 255.0;
      if normalized <= 0.03928 {
          normalized / 12.92
      } else {
          ((normalized + 0.055) / 1.055).powf(2.4)
      }
  }
  ```
- [ ] Run `cargo test -p gemelli-gui theme::tests`. Expect `test result: ok. 3 passed; 0 failed;`.
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`.
- [ ] `git add crates/gui/src/theme.rs crates/gui/src/main.rs`, commit:
  ```
  feat(gui): add WCAG 2.1 contrast ratio calculation
  ```

#### Cycle 2 — `tokens` module, WCAG-proved

- [ ] Write the failing test. Add to `theme.rs`'s `mod tests`:
  ```rust
      #[test]
      fn text_primary_meets_normal_text_contrast_on_bg_base() {
          assert!(contrast_ratio(tokens::TEXT_PRIMARY, tokens::BG_BASE) >= 4.5);
      }

      #[test]
      fn text_muted_meets_normal_text_contrast_on_bg_base() {
          assert!(contrast_ratio(tokens::TEXT_MUTED, tokens::BG_BASE) >= 4.5);
      }

      #[test]
      fn danger_meets_normal_text_contrast_on_bg_base() {
          assert!(contrast_ratio(tokens::DANGER, tokens::BG_BASE) >= 4.5);
      }

      #[test]
      fn accent_publish_meets_ui_component_contrast_on_bg_panel() {
          assert!(contrast_ratio(tokens::ACCENT_PUBLISH, tokens::BG_PANEL) >= 3.0);
      }

      #[test]
      fn accent_idle_meets_ui_component_contrast_on_bg_panel() {
          assert!(contrast_ratio(tokens::ACCENT_IDLE, tokens::BG_PANEL) >= 3.0);
      }
  ```
- [ ] Run `cargo test -p gemelli-gui theme::tests`. Expect **compile failure**: `error[E0433]: failed to resolve: use of undeclared crate or module `tokens`` (repeated per call site).
- [ ] Minimal implementation. Add to `theme.rs`, below `linearize`:
  ```rust
  /// Dark-theme color tokens. Every token's contrast ratio against the
  /// background(s) it is meant to sit on is proved by the tests in this
  /// module — see the plan doc for the hand-computed numbers behind each
  /// choice.
  pub mod tokens {
      use egui::Color32;

      /// Window background. Deliberately not pure black — `#1a1b1e`.
      pub const BG_BASE: Color32 = Color32::from_rgb(26, 27, 30);
      /// Sidebar/status-bar background — a hair lighter than `BG_BASE` so
      /// panels read as a distinct layer.
      pub const BG_PANEL: Color32 = Color32::from_rgb(33, 34, 38);
      pub const TEXT_PRIMARY: Color32 = Color32::from_rgb(230, 230, 230);
      pub const TEXT_MUTED: Color32 = Color32::from_rgb(160, 160, 168);
      /// Publishing state. Paired with the "● publishing" text label at the
      /// call site — never color alone (WCAG 1.4.1).
      pub const ACCENT_PUBLISH: Color32 = Color32::from_rgb(61, 220, 132);
      /// Idle state. Paired with the "○ stopped" text label at the call site.
      pub const ACCENT_IDLE: Color32 = Color32::from_rgb(125, 133, 144);
      pub const DANGER: Color32 = Color32::from_rgb(255, 107, 107);
      /// Crop-rect stroke. Drawn as a dual stroke (black outline + white
      /// core) at the crop_editor.rs call site, since no single color has a
      /// provable contrast ratio against arbitrary live video content.
      pub const CROP_OVERLAY: Color32 = Color32::WHITE;
  }
  ```
- [ ] Run `cargo test -p gemelli-gui theme::tests`. Expect `test result: ok. 8 passed; 0 failed;`.
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`.
- [ ] `git add crates/gui/src/theme.rs`, commit:
  ```
  feat(gui): add WCAG-verified dark theme color tokens
  ```

#### Cycle 3 — `apply_theme`

- [ ] Write the failing test. Add to `theme.rs`'s `mod tests`:
  ```rust
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
      }
  ```
- [ ] Run `cargo test -p gemelli-gui theme::tests`. Expect **compile failure**: `error[E0425]: cannot find function `apply_theme` in this scope`.
- [ ] Minimal implementation. Add to `theme.rs`, below the `tokens` module:
  ```rust
  /// Applies the `tokens` palette to `ctx`'s `Visuals`. Called once at
  /// startup from `GemelliApp::new`.
  pub fn apply_theme(ctx: &egui::Context) {
      let mut visuals = egui::Visuals::dark();
      visuals.window_fill = tokens::BG_BASE;
      visuals.panel_fill = tokens::BG_PANEL;
      visuals.override_text_color = Some(tokens::TEXT_PRIMARY);
      visuals.weak_text_color = Some(tokens::TEXT_MUTED);
      visuals.hyperlink_color = tokens::ACCENT_PUBLISH;
      visuals.selection.bg_fill = tokens::ACCENT_PUBLISH;
      visuals.selection.stroke = egui::Stroke::new(1.0, tokens::TEXT_PRIMARY);
      ctx.set_visuals(visuals);
  }
  ```
- [ ] Run `cargo test -p gemelli-gui theme::tests`. Expect `test result: ok. 9 passed; 0 failed;`.
- [ ] Wire it into the app shell. In `crates/gui/src/app.rs`, add `mod theme;`'s usage — call it from `GemelliApp::new`:
  ```rust
  impl GemelliApp {
      pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
          crate::theme::apply_theme(&cc.egui_ctx);
          Self {}
      }
  }
  ```
- [ ] Run `cargo build -p gemelli-gui`. Expect success.
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`.
- [ ] `git add crates/gui/src/theme.rs crates/gui/src/app.rs`, commit:
  ```
  feat(gui): apply WCAG token palette to egui visuals at startup
  ```

---

### Task 3: preview.rs + fps_meter.rs

**Files:**
- Create: `crates/gui/src/preview.rs`
- Create: `crates/gui/src/fps_meter.rs`
- Modify: `crates/gui/src/main.rs` — add `mod preview;` and `mod fps_meter;`

**Interfaces:**
- Consumes: `gemelli_core::frame::Frame` (`width()`, `height()`, `data()` — `crates/core/src/frame.rs`), `egui::{ColorImage, Rect}` (0.35.0, verified above), `std::time::{Duration, Instant}`.
- Produces:
  ```rust
  // preview.rs
  pub fn bgra_to_rgba(bgra: &[u8]) -> Vec<u8>;
  pub fn color_image(frame: &gemelli_core::frame::Frame) -> egui::ColorImage;
  pub fn fit_rect(frame_width: u32, frame_height: u32, avail: egui::Rect) -> egui::Rect;

  // fps_meter.rs
  pub struct FpsMeter { /* samples: VecDeque<std::time::Instant> */ }
  impl FpsMeter {
      pub fn new() -> Self;
      pub fn record(&mut self, now: std::time::Instant);
      pub fn rate(&mut self, now: std::time::Instant) -> f32;
  }
  ```

**The one non-obvious design decision in this task:** `fit_rect`'s letterbox math is written as cross-multiplication (`frame_w * avail_h` vs `avail_w * frame_h`) followed by a single multiply-then-divide for the constrained dimension, rather than computing `frame_width / frame_height` as an intermediate aspect ratio and dividing by it twice. For the exact test values below (e.g. `1920x1080` into an `800x600` avail rect), computing an intermediate `16.0/9.0`-ish f32 aspect ratio and dividing `800.0` by it does **not** reliably reproduce the exact expected `450.0` (division compounds rounding twice); reordering to `800.0 * 1080.0 / 1920.0` produces the exact integer-fraction result because both the multiplication and the division are individually exact in IEEE 754 for these magnitudes. This was checked against `rustc`/Python float semantics before locking in the test assertions, not assumed.

#### Cycle 1 — `bgra_to_rgba`

- [ ] Write the failing test. Create `crates/gui/src/preview.rs` with just:
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn bgra_to_rgba_swaps_red_and_blue_channels() {
          let bgra = [10u8, 20, 30, 40, 15, 25, 35, 45]; // two BGRA pixels
          assert_eq!(bgra_to_rgba(&bgra), vec![30, 20, 10, 40, 35, 25, 15, 45]);
      }

      #[test]
      fn bgra_to_rgba_empty_input_is_empty() {
          assert_eq!(bgra_to_rgba(&[]), Vec::<u8>::new());
      }
  }
  ```
- [ ] Add `mod preview;` and `mod fps_meter;` to `crates/gui/src/main.rs` (below `mod theme;`).
- [ ] Run `cargo test -p gemelli-gui preview::tests`. Expect **compile failure**: `error[E0425]: cannot find function `bgra_to_rgba` in this scope`.
- [ ] Minimal implementation. Prepend to `preview.rs` (above the test module):
  ```rust
  //! BGRA8 -> RGBA8 conversion, `egui::ColorImage` construction, and
  //! letterbox layout for the live preview panel.

  use gemelli_core::frame::Frame;

  /// BGRA8 -> RGBA8 byte swizzle (the channel order `egui::ColorImage`
  /// expects). Pure; any trailing bytes that don't form a full BGRA pixel
  /// are dropped, matching `Frame`'s own tightly-packed invariant.
  pub fn bgra_to_rgba(bgra: &[u8]) -> Vec<u8> {
      bgra.chunks_exact(4).flat_map(|pixel| [pixel[2], pixel[1], pixel[0], pixel[3]]).collect()
  }
  ```
- [ ] Run `cargo test -p gemelli-gui preview::tests`. Expect `test result: ok. 2 passed; 0 failed;`.
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`.
- [ ] `git add crates/gui/src/preview.rs crates/gui/src/main.rs`, commit:
  ```
  feat(gui): add BGRA8 to RGBA8 pixel conversion
  ```

#### Cycle 2 — `color_image`

- [ ] Write the failing test. Add to `preview.rs`'s `mod tests`:
  ```rust
      #[test]
      fn color_image_reports_frame_size() {
          let data = vec![10, 20, 30, 255, 11, 21, 31, 255]; // 2x1 BGRA
          let frame = Frame::new(2, 1, data).unwrap();
          let image = color_image(&frame);
          assert_eq!(image.size, [2, 1]);
      }

      #[test]
      fn color_image_swizzles_pixel_bytes() {
          let data = vec![10, 20, 30, 255, 11, 21, 31, 255]; // 2x1 BGRA
          let frame = Frame::new(2, 1, data).unwrap();
          let image = color_image(&frame);
          assert_eq!(image.as_raw(), [30, 20, 10, 255, 31, 21, 11, 255].as_slice());
      }
  ```
- [ ] Run `cargo test -p gemelli-gui preview::tests`. Expect **compile failure**: `error[E0425]: cannot find function `color_image` in this scope`.
- [ ] Minimal implementation. Add to `preview.rs`, below `bgra_to_rgba`:
  ```rust
  /// `Frame` -> `egui::ColorImage` (thin wrapper over `bgra_to_rgba`).
  pub fn color_image(frame: &Frame) -> egui::ColorImage {
      // u32 -> usize: no `From<u32> for usize` exists in std (checked with
      // rustc directly — only `From<u8>`/`From<u16>` do), so this reuses the
      // same fallible-but-practically-infallible `try_from` + `unwrap_or`
      // idiom `Frame::new` itself already uses in crates/core/src/frame.rs;
      // usize is >= 32 bits on every platform this workspace targets.
      let width = usize::try_from(frame.width()).unwrap_or(usize::MAX);
      let height = usize::try_from(frame.height()).unwrap_or(usize::MAX);
      let rgba = bgra_to_rgba(frame.data());
      egui::ColorImage::from_rgba_unmultiplied([width, height], &rgba)
  }
  ```
- [ ] Run `cargo test -p gemelli-gui preview::tests`. Expect `test result: ok. 4 passed; 0 failed;`.
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`.
- [ ] `git add crates/gui/src/preview.rs`, commit:
  ```
  feat(gui): build egui::ColorImage from Frame
  ```

#### Cycle 3 — `fit_rect`

- [ ] Write the failing test. Add to `preview.rs`'s `mod tests`:
  ```rust
      fn avail() -> egui::Rect {
          egui::Rect::from_min_size(egui::pos2(100.0, 50.0), egui::vec2(800.0, 600.0))
      }

      #[test]
      fn fit_rect_letterboxes_a_wide_frame_into_a_narrower_rect() {
          // 1920x1080 (16:9) is wider-than-avail (4:3) -> width-constrained,
          // centered with top/bottom bars.
          let rect = fit_rect(1920, 1080, avail());
          assert_eq!(
              rect,
              egui::Rect::from_min_size(egui::pos2(100.0, 125.0), egui::vec2(800.0, 450.0))
          );
      }

      #[test]
      fn fit_rect_pillarboxes_a_tall_frame_into_a_wider_rect() {
          // 1080x1920 (9:16) is taller-than-avail -> height-constrained,
          // centered with left/right bars.
          let rect = fit_rect(1080, 1920, avail());
          assert_eq!(
              rect,
              egui::Rect::from_min_size(egui::pos2(331.25, 50.0), egui::vec2(337.5, 600.0))
          );
      }

      #[test]
      fn fit_rect_matching_aspect_fills_avail_with_no_offset() {
          // Same 4:3 aspect as avail -> exact fill, zero letterbox offset.
          let rect = fit_rect(800, 600, avail());
          assert_eq!(rect, avail());
      }
  ```
- [ ] Run `cargo test -p gemelli-gui preview::tests`. Expect **compile failure**: `error[E0425]: cannot find function `fit_rect` in this scope`.
- [ ] Minimal implementation. Add to `preview.rs`, below `color_image`:
  ```rust
  /// gui's isolated `as`-cast, mirroring gemelli-core's `scale_dimension`
  /// precedent (crates/core/src/transform/scale.rs): u32 -> f32 has no
  /// lossless std conversion (`f32::from(u32)` does not exist — checked with
  /// rustc directly — because f32's 24-bit mantissa can't represent every
  /// u32 value), and camera/window dimensions never remotely approach 2^24,
  /// so one documented cast site is preferable to threading a fallible
  /// conversion through the render path.
  #[allow(clippy::as_conversions)]
  fn dim_to_f32(v: u32) -> f32 {
      v as f32
  }

  /// Letterbox-fits a `frame_width x frame_height` frame into `avail`,
  /// preserving aspect ratio and centering the result.
  pub fn fit_rect(frame_width: u32, frame_height: u32, avail: egui::Rect) -> egui::Rect {
      let frame_w = dim_to_f32(frame_width);
      let frame_h = dim_to_f32(frame_height);
      let avail_w = avail.width();
      let avail_h = avail.height();

      // Compare frame_w/frame_h against avail_w/avail_h via
      // cross-multiplication instead of dividing twice, so the constrained
      // dimension comes from a single multiply + divide (exact for
      // realistic frame/window sizes) rather than compounding two
      // divisions' rounding error.
      let (draw_w, draw_h) = if frame_w * avail_h > avail_w * frame_h {
          (avail_w, avail_w * frame_h / frame_w)
      } else {
          (avail_h * frame_w / frame_h, avail_h)
      };

      let offset_x = (avail_w - draw_w) / 2.0;
      let offset_y = (avail_h - draw_h) / 2.0;
      let min = avail.min + egui::vec2(offset_x, offset_y);

      egui::Rect::from_min_size(min, egui::vec2(draw_w, draw_h))
  }
  ```
- [ ] Run `cargo test -p gemelli-gui preview::tests`. Expect `test result: ok. 7 passed; 0 failed;`.
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`.
- [ ] `git add crates/gui/src/preview.rs`, commit:
  ```
  feat(gui): letterbox-fit frame dimensions into the preview rect
  ```

#### Cycle 4 — `FpsMeter`

- [ ] Write the failing test. Create `crates/gui/src/fps_meter.rs` with just:
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;
      use std::time::Duration;

      #[test]
      fn rate_counts_samples_within_the_one_second_window() {
          let mut meter = FpsMeter::new();
          let base = std::time::Instant::now();
          for i in 0..10u64 {
              meter.record(base + Duration::from_millis(i * 100));
          }
          // last sample at 900ms; queried at 900ms, every sample is <= 900ms
          // old, so all 10 remain in the 1s window.
          assert_eq!(meter.rate(base + Duration::from_millis(900)), 10.0);
      }

      #[test]
      fn rate_evicts_samples_older_than_one_second() {
          let mut meter = FpsMeter::new();
          let base = std::time::Instant::now();
          meter.record(base); // ages out by t=1500ms (age 1500ms > 1000ms)
          meter.record(base + Duration::from_millis(900)); // age 600ms, kept
          assert_eq!(meter.rate(base + Duration::from_millis(1500)), 1.0);
      }

      #[test]
      fn rate_on_empty_meter_is_zero() {
          let mut meter = FpsMeter::new();
          assert_eq!(meter.rate(std::time::Instant::now()), 0.0);
      }
  }
  ```
- [ ] Run `cargo test -p gemelli-gui fps_meter::tests`. Expect **compile failure**: `error[E0433]: failed to resolve: use of undeclared type `FpsMeter`` (repeated per call site).
- [ ] Minimal implementation. Prepend to `fps_meter.rs` (above the test module):
  ```rust
  //! Sliding 1-second window frame-rate counter, driven by injected
  //! `Instant`s so it's unit-testable without a real clock/thread.

  use std::collections::VecDeque;
  use std::time::{Duration, Instant};

  const WINDOW: Duration = Duration::from_secs(1);

  pub struct FpsMeter {
      samples: VecDeque<Instant>,
  }

  impl FpsMeter {
      pub fn new() -> Self {
          Self { samples: VecDeque::new() }
      }

      /// Records one frame-published event at `now`.
      pub fn record(&mut self, now: Instant) {
          self.samples.push_back(now);
      }

      /// Evicts samples older than the 1-second window as of `now`, then
      /// returns the remaining sample count as an approximate frames/sec.
      pub fn rate(&mut self, now: Instant) -> f32 {
          self.evict_stale(now);
          count_to_f32(self.samples.len())
      }

      fn evict_stale(&mut self, now: Instant) {
          while let Some(&oldest) = self.samples.front() {
              if now.saturating_duration_since(oldest) <= WINDOW {
                  break;
              }
              self.samples.pop_front();
          }
      }
  }

  impl Default for FpsMeter {
      fn default() -> Self {
          Self::new()
      }
  }

  /// gui's second isolated `as`-cast (see `preview::dim_to_f32` for the
  /// std-conversion-gap rationale — the same `f32: From<u32>` gap applies to
  /// `usize -> f32` here). Sample counts are bounded by the 1-second sliding
  /// window (never remotely near 2^24), so this is lossless in practice.
  #[allow(clippy::as_conversions)]
  fn count_to_f32(count: usize) -> f32 {
      let count = u32::try_from(count).unwrap_or(u32::MAX);
      count as f32
  }
  ```
- [ ] Run `cargo test -p gemelli-gui fps_meter::tests`. Expect `test result: ok. 3 passed; 0 failed;`.
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`.
- [ ] `git add crates/gui/src/fps_meter.rs crates/gui/src/main.rs`, commit:
  ```
  feat(gui): add sliding 1-second FpsMeter
  ```
## Section B — Tasks 4–5 (crates/gui: worker.rs + crop_editor.rs)

All paths below are repo-root relative (`/Users/napochaan/ghq/github.com/naporin0624/web-cam-sharedtexture`).
Run every command from the repo root. Test command form: `cargo test -p gemelli-gui <filter>`
(`gemelli-gui` is currently a binary-only crate — `src/main.rs` with `mod`
declarations, no `src/lib.rs` — so unit tests live inline in `#[cfg(test)]
mod` blocks inside each source file, exactly like `crates/cli` already does;
`cargo test -p gemelli-gui` builds and runs the bin target's test harness
directly, no `--lib`/`--bin` flag needed).
Lint gate (run after every green step, before every commit):
`cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`

**Assumed prior state (Tasks 1–3, out of scope here):** `crates/gui/Cargo.toml`
already depends on `gemelli-core` (path) and `eframe` (workspace, default
features) per the contract's dependency note, `crates/gui/src/main.rs` boots
`eframe::run_native` and already has some `mod ...;` declarations for
`app`/`theme`/`preview`/`fps_meter`/`sidebar`, and `build.rs` reads
`DEP_SYPHON_BRIDGE_RPATH`. That last fact means `gemelli-syphon` must already
be a macOS-target dependency of `gemelli-gui` by Task 1 (the rpath var is
only set when it's an active dependency) — Task 4's Step 0 below verifies
this instead of blindly re-adding it, since a duplicate `[target...]` table
would conflict with what Task 1 wrote.

**Contract ambiguities resolved in this section (flagged per instructions):**
1. **No `tests/worker_smoke.rs` integration file.** The contract says to
   "mirror core's `camera_smoke` pattern" for hardware-dependent tests. Core
   can put that in `crates/core/tests/camera_smoke.rs` because `gemelli-core`
   has a `src/lib.rs` (a library target `tests/*.rs` can `use gemelli_core::
   ...` against). `gemelli-gui` is binary-only (see above) — Rust integration
   tests in a crate's `tests/` directory only link against a `[lib]` target,
   which does not exist here, and adding one is Task 1/3's scaffolding
   decision, not this section's to make unilaterally. Resolution: the
   hardware-dependent `spawn_worker` tests live in `worker.rs`'s own
   `#[cfg(test)] mod spawn_worker_tests`, `#[ignore]`d, with the same
   "run manually with `-- --ignored`" doc-comment convention `camera_smoke.rs`
   and `crates/cli/src/run.rs`'s `run_smoke_test` use.
2. **`CropMapping::to_frame`'s "clamped into frame bounds" reuses
   `clamp_rect`** rather than hand-rolling separate bounds-only clamping
   logic. `clamp_rect`'s contract ("min size 16x16, fully inside frame") is a
   strict superset of "inside frame bounds," so calling it from `to_frame`
   is one code path instead of two, and a drag-produced rect is never
   smaller than 16px in frame space either way.
3. **Corner-handle hit box is an 8px axis-aligned square, not an 8px-radius
   circle.** The contract says "corner handles 8px" without specifying the
   hit-region shape. A square (`|dx| <= 8 && |dy| <= 8`) avoids a `sqrt` in a
   per-frame-repaint hit-test and gives hand-computable, exact test values;
   a circular hit region would not change any UI-visible behavior at the
   pixel scale involved.
4. **`WorkerError` does not derive `PartialEq`**, matching the contract's
   literal `#[derive(Debug, thiserror::Error)]` (no `PartialEq` in that
   list). This isn't a correction — it's confirmation the contract already
   got this right, unlike `TransformError` in Section A: `WorkerError` wraps
   `CaptureError` and `PublishError`, and neither of those derives
   `PartialEq` in `gemelli-core` (only `Debug` + `thiserror::Error`), so a
   `#[derive(PartialEq)]` here would not compile. Tests match on error
   variants with `matches!(...)`, same as `crates/core/src/pipeline.rs`'s
   own tests do for `PipelineError`.

---

### Task 4: worker.rs

**Files:**
- Modify: `crates/gui/Cargo.toml` — verify/add `arc-swap`, `thiserror`, and
  the macOS-only `gemelli-syphon` target dependency
- Modify: `Cargo.toml` (root) — verify/add `arc-swap` to `[workspace.dependencies]`
- Create: `crates/gui/src/worker.rs`
- Modify: `crates/gui/src/main.rs` — add `mod worker;`
- Test: `crates/gui/src/worker.rs` (`#[cfg(test)] mod` blocks)

**Interfaces:**
- Consumes: `gemelli_core::frame::Frame`, `gemelli_core::capture::{CaptureSource,
  CaptureError, NokhwaSource}`, `gemelli_core::publish::{TexturePublisher,
  PublishError}`, `gemelli_core::transform::{self, TransformConfig,
  TransformError}` (all Phase 1 / `gemelli-core`), `gemelli_syphon::SyphonPublisher`
  (macOS only)
- Produces:
  ```rust
  pub struct SharedState {
      pub transform: arc_swap::ArcSwap<TransformConfig>,
      pub latest_output: std::sync::Mutex<Option<Frame>>,
      pub latest_raw: std::sync::Mutex<Option<Frame>>,
      pub frames_published: std::sync::atomic::AtomicU64,
  }
  impl SharedState { pub fn new(config: TransformConfig) -> Self; }

  #[derive(Debug, thiserror::Error)]
  pub enum WorkerError {
      #[error(transparent)] Capture(#[from] CaptureError),
      #[error(transparent)] Transform(#[from] TransformError),
      #[error(transparent)] Publish(#[from] PublishError),
  }

  pub fn run_capture(
      source: &mut dyn CaptureSource,
      publisher: &mut dyn TexturePublisher,
      shared: &SharedState,
      stop: &std::sync::atomic::AtomicBool,
      errors: &std::sync::mpsc::Sender<WorkerError>,
  );

  pub struct WorkerHandle { /* stop: Arc<AtomicBool>, join: Option<JoinHandle<()>> */ }
  impl WorkerHandle {
      pub fn stop(&mut self);
      pub fn is_running(&self) -> bool;
  }
  impl Drop for WorkerHandle { fn drop(&mut self); }

  pub struct WorkerSpec { pub device_index: u32, pub requested_fps: Option<u32>, pub server_name: String }
  pub fn spawn_worker(
      spec: WorkerSpec,
      shared: std::sync::Arc<SharedState>,
      errors: std::sync::mpsc::Sender<WorkerError>,
  ) -> WorkerHandle;
  ```

#### Step 0 — Cargo.toml wiring (not a TDD cycle)

- [ ] Run `grep -n "arc-swap" Cargo.toml` (root). If it prints nothing, edit
  the root `Cargo.toml`'s `[workspace.dependencies]` table (add after the
  existing `ctrlc = "3.5"` line):
  ```toml
  ctrlc = "3.5"
  arc-swap = "1.9"
  ```
  (Verified current version at plan-authoring time: `arc-swap = "1.9.2"` —
  `"1.9"` accepts any compatible `1.9.x`/`1.x` per Cargo's caret rules,
  matching the `thiserror = "2"` / `clap = "4.6"` precision level already
  used in this table.) If the grep already found a line, leave the table
  alone.
- [ ] Run `grep -n "thiserror\|arc-swap\|gemelli-syphon" crates/gui/Cargo.toml`.
  Using that output, edit `crates/gui/Cargo.toml` so it reads exactly:
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
  thiserror = { workspace = true }
  arc-swap = { workspace = true }

  [target.'cfg(target_os = "macos")'.dependencies]
  gemelli-syphon = { path = "../syphon" }
  ```
  (Keep whatever `eframe`/`egui` lines Task 1/3 already added — this step
  only adds `thiserror`, `arc-swap`, and confirms/adds the macOS target
  table; do not remove pre-existing dependency lines.)
- [ ] Create an empty `crates/gui/src/worker.rs` (zero bytes).
- [ ] Edit `crates/gui/src/main.rs`: add `mod worker;` alongside whatever
  other `mod ...;` lines Task 1/3 already put there (do not remove them).
  If `main.rs` is still the untouched placeholder at this point, replace it
  with:
  ```rust
  //! GUI entry point for the webcam -> Spout/Syphon sharing tool.

  mod worker;

  fn main() {
      println!("gemelli-gui: not yet implemented");
  }
  ```
- [ ] Run `cargo build --workspace`. Expect success (an empty `worker.rs`
  with no items compiles fine as a module).
- [ ] `git add Cargo.toml crates/gui/Cargo.toml crates/gui/src/worker.rs crates/gui/src/main.rs`,
  commit:
  ```
  chore(gui): wire up worker module and thread/config dependencies
  ```

#### Cycle 1 — `SharedState::new`

- [ ] Write the failing test. Set `crates/gui/src/worker.rs` to:
  ```rust
  #[cfg(test)]
  mod shared_state_tests {
      use gemelli_core::transform::{Rotation, TransformConfig};
      use std::sync::atomic::Ordering;

      use super::SharedState;

      #[test]
      fn new_starts_empty_with_the_given_config() {
          let config = TransformConfig { rotation: Rotation::R90, ..TransformConfig::default() };
          let shared = SharedState::new(config.clone());

          assert_eq!(**shared.transform.load(), config);
          assert_eq!(*shared.latest_output.lock().unwrap(), None);
          assert_eq!(*shared.latest_raw.lock().unwrap(), None);
          assert_eq!(shared.frames_published.load(Ordering::SeqCst), 0);
      }
  }
  ```
- [ ] Run `cargo test -p gemelli-gui shared_state_tests`. Expect **compile
  failure**: `error[E0433]: failed to resolve: use of undeclared type
  `SharedState`` (plus "unresolved import `super::SharedState`").
- [ ] Minimal implementation. Prepend to `worker.rs` (above the test module):
  ```rust
  //! Capture-thread worker: owns the camera + publisher lifecycle on a
  //! dedicated OS thread, exchanging state with the GUI thread via
  //! `SharedState` (latest frames + a live-editable transform config) and an
  //! `mpsc` error channel.

  use std::sync::atomic::AtomicU64;
  use std::sync::{Arc, Mutex};

  use arc_swap::ArcSwap;
  use gemelli_core::frame::Frame;
  use gemelli_core::transform::TransformConfig;

  /// Shared between the GUI thread and the capture thread.
  pub struct SharedState {
      pub transform: ArcSwap<TransformConfig>,
      pub latest_output: Mutex<Option<Frame>>,
      pub latest_raw: Mutex<Option<Frame>>,
      pub frames_published: AtomicU64,
  }

  impl SharedState {
      pub fn new(config: TransformConfig) -> Self {
          Self {
              transform: ArcSwap::new(Arc::new(config)),
              latest_output: Mutex::new(None),
              latest_raw: Mutex::new(None),
              frames_published: AtomicU64::new(0),
          }
      }
  }
  ```
- [ ] Run `cargo test -p gemelli-gui shared_state_tests`. Expect
  `test result: ok. 1 passed; 0 failed;`.
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`.
- [ ] `git add crates/gui/src/worker.rs`, commit:
  ```
  feat(gui): add SharedState for capture-thread <-> GUI exchange
  ```

#### Cycle 2 — `run_capture`: raw + output stored, `frames_published` increments

This is the first cycle that needs in-file test doubles. They are modeled on
`crates/core/src/pipeline.rs`'s private `FakeSource`/`CollectingPublisher`
but are **not** the same types — those are private to core's test module and
inaccessible here — so this crate defines its own, extended with a
per-publish hook so later cycles (config swap, stop-after-N) can reuse the
same `CollectingPublisher` instead of adding new fields per scenario.

- [ ] Write the failing test. Add to `worker.rs`:
  ```rust
  #[cfg(test)]
  mod run_capture_tests {
      use std::collections::VecDeque;
      use std::sync::atomic::{AtomicBool, Ordering};
      use std::sync::mpsc;

      use gemelli_core::capture::{CaptureError, CaptureSource};
      use gemelli_core::frame::Frame;
      use gemelli_core::publish::{PublishError, TexturePublisher};
      use gemelli_core::transform::{self, Rotation, TransformConfig};

      use super::{SharedState, WorkerError, run_capture};

      struct FakeSource {
          frames: VecDeque<Frame>,
      }

      impl FakeSource {
          fn new(frames: Vec<Frame>) -> Self {
              Self { frames: frames.into() }
          }
      }

      impl CaptureSource for FakeSource {
          fn next_frame(&mut self) -> Result<Frame, CaptureError> {
              self.frames
                  .pop_front()
                  .ok_or_else(|| CaptureError::FrameRead { reason: "exhausted".to_string() })
          }
      }

      /// Records every frame handed to `publish`, then runs `hook` with the
      /// running publish count. Tests use the hook to flip `stop` or swap
      /// `shared.transform` after a chosen number of publishes, instead of
      /// each scenario needing its own publisher type.
      struct CollectingPublisher<F: FnMut(usize)> {
          published: Vec<Frame>,
          hook: F,
      }

      impl<F: FnMut(usize)> CollectingPublisher<F> {
          fn new(hook: F) -> Self {
              Self { published: Vec::new(), hook }
          }
      }

      impl<F: FnMut(usize)> TexturePublisher for CollectingPublisher<F> {
          fn publish(&mut self, frame: &Frame) -> Result<(), PublishError> {
              self.published.push(frame.clone());
              (self.hook)(self.published.len());
              Ok(())
          }
      }

      struct FailingPublisher;

      impl TexturePublisher for FailingPublisher {
          fn publish(&mut self, _frame: &Frame) -> Result<(), PublishError> {
              Err(PublishError::Publish { reason: "sink closed".to_string() })
          }
      }

      fn asymmetric_frame() -> Frame {
          // 2 wide x 3 tall, every pixel a unique BGRA value, row-major —
          // copied from crates/core/src/pipeline.rs's test fixture so a
          // rotation visibly changes both dimensions and pixel order.
          let data = vec![
              10, 20, 30, 255, 40, 50, 60, 255, // row 0
              70, 80, 90, 255, 100, 110, 120, 255, // row 1
              130, 140, 150, 255, 160, 170, 180, 255, // row 2
          ];
          Frame::new(2, 3, data).unwrap()
      }

      #[test]
      fn stores_raw_and_output_frames_and_counts_published() {
          let frame = asymmetric_frame();
          let config = TransformConfig { rotation: Rotation::R90, ..TransformConfig::default() };
          let expected_output = transform::apply(&frame, &config).unwrap();
          let shared = SharedState::new(config);
          let mut source = FakeSource::new(vec![frame.clone()]);
          let stop = AtomicBool::new(false);
          let mut publisher = CollectingPublisher::new(|n| {
              if n == 1 {
                  stop.store(true, Ordering::SeqCst);
              }
          });
          let (tx, rx) = mpsc::channel::<WorkerError>();

          run_capture(&mut source, &mut publisher, &shared, &stop, &tx);

          assert_eq!(*shared.latest_raw.lock().unwrap(), Some(frame));
          assert_eq!(*shared.latest_output.lock().unwrap(), Some(expected_output.clone()));
          assert_eq!(publisher.published, vec![expected_output]);
          assert_eq!(shared.frames_published.load(Ordering::SeqCst), 1);
          assert!(rx.try_recv().is_err(), "no error should have been sent");
      }
  }
  ```
- [ ] Run `cargo test -p gemelli-gui run_capture_tests`. Expect **compile
  failure**: `error[E0432]: unresolved import `super::run_capture`` (`WorkerError`
  resolves already from Cycle 1's neighboring — no, it doesn't exist yet
  either: expect both `run_capture` and `WorkerError` reported unresolved).
- [ ] Minimal implementation. Add to `worker.rs`, after the `SharedState`
  block and before the `run_capture_tests` module:
  ```rust
  use std::sync::atomic::{AtomicBool, Ordering};
  use std::sync::{MutexGuard, PoisonError, mpsc};

  use gemelli_core::capture::{CaptureError, CaptureSource};
  use gemelli_core::publish::{PublishError, TexturePublisher};
  use gemelli_core::transform::{self, TransformError};

  #[derive(Debug, thiserror::Error)]
  pub enum WorkerError {
      #[error(transparent)]
      Capture(#[from] CaptureError),
      #[error(transparent)]
      Transform(#[from] TransformError),
      #[error(transparent)]
      Publish(#[from] PublishError),
  }

  /// Recovers a possibly-poisoned lock instead of propagating the poison:
  /// the guarded value is a plain `Option<Frame>`, so a panic elsewhere
  /// while holding the lock never leaves it in a state unsafe to read.
  fn recover_lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
      mutex.lock().unwrap_or_else(PoisonError::into_inner)
  }

  fn run_capture_step(
      source: &mut dyn CaptureSource,
      publisher: &mut dyn TexturePublisher,
      shared: &SharedState,
  ) -> Result<(), WorkerError> {
      let raw = source.next_frame()?;
      *recover_lock(&shared.latest_raw) = Some(raw.clone());

      let config = shared.transform.load();
      let output = transform::apply(&raw, &config)?;
      publisher.publish(&output)?;

      *recover_lock(&shared.latest_output) = Some(output);
      shared.frames_published.fetch_add(1, Ordering::SeqCst);

      Ok(())
  }

  /// Loops until `stop`: next_frame -> store raw -> apply(shared.transform
  /// snapshot) -> publish -> store output -> frames_published += 1. On
  /// error: send it on `errors` and return (the thread ends; the GUI
  /// decides whether to respawn).
  pub fn run_capture(
      source: &mut dyn CaptureSource,
      publisher: &mut dyn TexturePublisher,
      shared: &SharedState,
      stop: &AtomicBool,
      errors: &mpsc::Sender<WorkerError>,
  ) {
      while !stop.load(Ordering::SeqCst) {
          if let Err(error) = run_capture_step(source, publisher, shared) {
              // If the GUI already dropped its receiver there is nothing
              // left to notify; the thread still ends, which is what
              // matters.
              let _ = errors.send(error);
              return;
          }
      }
  }
  ```
- [ ] Run `cargo test -p gemelli-gui run_capture_tests`. Expect
  `test result: ok. 1 passed; 0 failed;`.
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`.
- [ ] `git add crates/gui/src/worker.rs`, commit:
  ```
  feat(gui): add run_capture core loop with raw/output frame capture
  ```

#### Cycle 3 — config swap mid-run takes effect on later output

- [ ] Write the test. Add to `run_capture_tests`. `shared`, `new_config`,
  and `stop` are all borrowed immutably by the hook closure below — this is
  the reason the closure calls `shared.transform.store(...)` and
  `stop.store(...)` (both take `&self`) rather than needing `&mut shared`,
  which would otherwise conflict with `run_capture`'s own `&shared` borrow
  and the post-run assertions' reads:
  ```rust
      #[test]
      fn config_swap_mid_run_affects_later_output_only() {
          // Same frame content published twice; the second config rotates
          // it 90°, so a changed *shape* (3x2 vs 2x3) proves the swap took
          // effect, independent of any pixel-order subtlety.
          let frame = asymmetric_frame();
          let old_config = TransformConfig::default();
          let new_config = TransformConfig { rotation: Rotation::R90, ..TransformConfig::default() };
          let expected_first = transform::apply(&frame, &old_config).unwrap();
          let expected_second = transform::apply(&frame, &new_config).unwrap();
          let shared = SharedState::new(old_config);
          let mut source = FakeSource::new(vec![frame.clone(), frame]);
          let stop = AtomicBool::new(false);
          let mut publisher = CollectingPublisher::new(|n| {
              if n == 1 {
                  shared.transform.store(std::sync::Arc::new(new_config.clone()));
              }
              if n == 2 {
                  stop.store(true, Ordering::SeqCst);
              }
          });
          let (tx, rx) = mpsc::channel::<WorkerError>();

          run_capture(&mut source, &mut publisher, &shared, &stop, &tx);

          assert_eq!(publisher.published, vec![expected_first, expected_second.clone()]);
          assert_eq!(*shared.latest_output.lock().unwrap(), Some(expected_second));
          assert!(rx.try_recv().is_err());
      }
  ```
- [ ] Run `cargo test -p gemelli-gui run_capture_tests`. Since
  `run_capture`'s Cycle 2 implementation already reloads
  `shared.transform.load()` on every iteration, this test is expected to
  **pass immediately** with no further production-code change — run it to
  confirm that pass explicitly. This cycle exists to *lock in* per-iteration
  config reload as a tested behavior, not to add new code; unlike Cycles
  1–2, there is no RED step here because the behavior under test was
  already an unavoidable side effect of Cycle 2's implementation.
- [ ] Run `cargo test -p gemelli-gui run_capture_tests`. Expect
  `test result: ok. 2 passed; 0 failed;`.
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`.
- [ ] `git add crates/gui/src/worker.rs`, commit:
  ```
  test(gui): lock in per-iteration transform reload in run_capture
  ```

#### Cycle 4 — stop flag ends the loop cleanly, no error sent

- [ ] Write the failing test. Add to `run_capture_tests`:
  ```rust
      #[test]
      fn stop_flag_ends_loop_with_no_error() {
          let pixel = Frame::new(1, 1, vec![0, 0, 0, 255]).unwrap();
          let frames = vec![pixel.clone(), pixel.clone(), pixel];
          let mut source = FakeSource::new(frames);
          let shared = SharedState::new(TransformConfig::default());
          let stop = AtomicBool::new(false);
          let mut publisher = CollectingPublisher::new(|n| {
              if n == 2 {
                  stop.store(true, Ordering::SeqCst);
              }
          });
          let (tx, rx) = mpsc::channel::<WorkerError>();

          // 3 frames are available but stop_after=2 must end the loop before
          // the 3rd next_frame() call — if run_capture ignored `stop` this
          // would instead exhaust FakeSource and send a Capture error.
          run_capture(&mut source, &mut publisher, &shared, &stop, &tx);

          assert_eq!(publisher.published.len(), 2);
          assert_eq!(shared.frames_published.load(Ordering::SeqCst), 2);
          assert!(rx.try_recv().is_err());
      }
  ```
- [ ] Run `cargo test -p gemelli-gui run_capture_tests`. This also passes
  immediately with Cycle 2's implementation (the `while !stop.load(...)`
  guard at the top of the loop already has this property, mirroring
  `crates/core/src/pipeline.rs::run_pipeline`'s identical test). Run it to
  confirm the pass explicitly, same rationale as Cycle 3.
- [ ] Run `cargo test -p gemelli-gui run_capture_tests`. Expect
  `test result: ok. 3 passed; 0 failed;`.
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`.
- [ ] `git add crates/gui/src/worker.rs`, commit:
  ```
  test(gui): lock in clean stop-flag exit in run_capture
  ```

#### Cycle 5 — capture error sent on channel, loop returns

- [ ] Write the failing test. Add to `run_capture_tests`:
  ```rust
      #[test]
      fn capture_error_is_sent_and_loop_returns() {
          let mut source = FakeSource::new(vec![]); // next_frame() errors immediately
          let shared = SharedState::new(TransformConfig::default());
          let stop = AtomicBool::new(false);
          let mut publisher = CollectingPublisher::new(|_| {});
          let (tx, rx) = mpsc::channel::<WorkerError>();

          run_capture(&mut source, &mut publisher, &shared, &stop, &tx);

          let error = rx.try_recv().expect("an error must have been sent");
          assert!(matches!(error, WorkerError::Capture(CaptureError::FrameRead { .. })));
          assert_eq!(publisher.published.len(), 0);
          assert_eq!(shared.frames_published.load(Ordering::SeqCst), 0);
      }
  ```
- [ ] Run `cargo test -p gemelli-gui run_capture_tests`. This, too, already
  passes with Cycle 2's implementation (`?` on `source.next_frame()` inside
  `run_capture_step` converts via `#[from] CaptureError`, and the `Err(...)`
  arm in `run_capture` sends + returns). Confirm the pass explicitly.
- [ ] Run `cargo test -p gemelli-gui run_capture_tests`. Expect
  `test result: ok. 4 passed; 0 failed;`.
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`.
- [ ] `git add crates/gui/src/worker.rs`, commit:
  ```
  test(gui): lock in capture-error channel reporting in run_capture
  ```

#### Cycle 6 — publish error sent on channel, loop returns, output not overwritten

- [ ] Write the failing test. Add to `run_capture_tests`:
  ```rust
      #[test]
      fn publish_error_is_sent_and_output_is_not_overwritten() {
          let frame = Frame::new(1, 1, vec![0, 0, 0, 255]).unwrap();
          let mut source = FakeSource::new(vec![frame.clone()]);
          let shared = SharedState::new(TransformConfig::default());
          let stop = AtomicBool::new(false);
          let mut publisher = FailingPublisher;
          let (tx, rx) = mpsc::channel::<WorkerError>();

          run_capture(&mut source, &mut publisher, &shared, &stop, &tx);

          let error = rx.try_recv().expect("an error must have been sent");
          assert!(matches!(error, WorkerError::Publish(PublishError::Publish { .. })));
          // Raw is stored before publish is attempted; output is only
          // stored *after* a successful publish — proves the step order
          // documented on run_capture (store raw -> apply -> publish ->
          // store output).
          assert_eq!(*shared.latest_raw.lock().unwrap(), Some(frame));
          assert_eq!(*shared.latest_output.lock().unwrap(), None);
          assert_eq!(shared.frames_published.load(Ordering::SeqCst), 0);
      }
  ```
- [ ] Run `cargo test -p gemelli-gui run_capture_tests`. Expect this one to
  already pass too (same reasoning as Cycles 3–5 — `run_capture_step`'s `?`
  on `publisher.publish(&output)` returns before the `latest_output` store
  line is ever reached). Confirm explicitly.
- [ ] Run `cargo test -p gemelli-gui run_capture_tests`. Expect
  `test result: ok. 5 passed; 0 failed;`.
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`.
- [ ] `git add crates/gui/src/worker.rs`, commit:
  ```
  test(gui): lock in publish-error channel reporting and step ordering
  ```

> Cycles 3–6 all landed as "confirm-passing" cycles rather than
> implement-then-pass, because `run_capture`'s Cycle 2 implementation already
> has every required-behavior property in one small function — this is
> expected and correct t-wada TDD: each cycle still starts RED-by-not-yet-
> written (the test doesn't exist until you write it) and the run step is
> what proves the behavior, even when no production code changes. Do not
> skip running each test individually just because you expect it to pass.

#### Cycle 7 — `WorkerHandle`: stop, idempotency, `is_running`, `Drop`

Uses `run_capture` with fakes wired into a real OS thread (no camera/Syphon
involved) to prove the handle's lifecycle mechanics, since `WorkerHandle`'s
fields are private and can only be constructed by code inside `worker.rs`
itself.

- [ ] Write the failing test. Add to `worker.rs`:
  ```rust
  #[cfg(test)]
  mod handle_tests {
      use std::sync::atomic::{AtomicBool, Ordering};
      use std::sync::{Arc, mpsc};
      use std::thread;
      use std::time::Duration;

      use gemelli_core::capture::{CaptureError, CaptureSource};
      use gemelli_core::frame::Frame;
      use gemelli_core::publish::{PublishError, TexturePublisher};
      use gemelli_core::transform::TransformConfig;

      use super::{SharedState, WorkerError, WorkerHandle, run_capture};

      /// Always returns the same 1x1 frame — lets a test run a real
      /// `run_capture` thread that only stops when told to, with no bound
      /// on frame count.
      struct InfiniteSource {
          frame: Frame,
      }

      impl CaptureSource for InfiniteSource {
          fn next_frame(&mut self) -> Result<Frame, CaptureError> {
              Ok(self.frame.clone())
          }
      }

      struct NullPublisher;

      impl TexturePublisher for NullPublisher {
          fn publish(&mut self, _frame: &Frame) -> Result<(), PublishError> {
              Ok(())
          }
      }

      fn spawn_fake_worker(shared: Arc<SharedState>, errors: mpsc::Sender<WorkerError>) -> WorkerHandle {
          let stop = Arc::new(AtomicBool::new(false));
          let thread_stop = Arc::clone(&stop);
          let join = thread::spawn(move || {
              let mut source = InfiniteSource { frame: Frame::new(1, 1, vec![0, 0, 0, 255]).unwrap() };
              let mut publisher = NullPublisher;
              run_capture(&mut source, &mut publisher, &shared, &thread_stop, &errors);
          });
          WorkerHandle { stop, join: Some(join) }
      }

      /// Busy-waits for the fake worker to have processed at least one
      /// frame — a deterministic readiness signal instead of a blind sleep.
      fn wait_for_first_frame(shared: &SharedState) {
          while shared.frames_published.load(Ordering::SeqCst) == 0 {
              thread::sleep(Duration::from_millis(1));
          }
      }

      #[test]
      fn is_running_reflects_thread_lifecycle() {
          let shared = Arc::new(SharedState::new(TransformConfig::default()));
          let (tx, _rx) = mpsc::channel();
          let mut handle = spawn_fake_worker(Arc::clone(&shared), tx);

          wait_for_first_frame(&shared);
          assert!(handle.is_running());

          handle.stop(); // blocks until the thread actually joins

          assert!(!handle.is_running());
      }

      #[test]
      fn stop_is_idempotent() {
          let shared = Arc::new(SharedState::new(TransformConfig::default()));
          let (tx, _rx) = mpsc::channel();
          let mut handle = spawn_fake_worker(shared, tx);

          handle.stop();
          handle.stop(); // must not panic or block forever

          assert!(!handle.is_running());
      }

      #[test]
      fn drop_stops_the_worker_thread() {
          let shared = Arc::new(SharedState::new(TransformConfig::default()));
          let (tx, _rx) = mpsc::channel();
          let handle = spawn_fake_worker(Arc::clone(&shared), tx);

          wait_for_first_frame(&shared);
          drop(handle);

          // Drop's stop() joins before returning, so the thread is already
          // dead by this point — no polling needed for this assertion to
          // be non-flaky.
          let count_at_drop = shared.frames_published.load(Ordering::SeqCst);
          thread::sleep(Duration::from_millis(20));
          assert_eq!(shared.frames_published.load(Ordering::SeqCst), count_at_drop);
      }
  }
  ```
- [ ] Run `cargo test -p gemelli-gui handle_tests`. Expect **compile
  failure**: `error[E0433]: failed to resolve: use of undeclared type
  `WorkerHandle`` (and the struct-literal `WorkerHandle { stop, join:
  Some(join) }` failing to resolve fields on a nonexistent type).
- [ ] Minimal implementation. Add to `worker.rs`, after `run_capture` and
  before the `run_capture_tests` module:
  ```rust
  /// Owns the capture thread. Dropping (or calling `stop`) sets the stop
  /// flag and joins.
  pub struct WorkerHandle {
      stop: Arc<AtomicBool>,
      join: Option<std::thread::JoinHandle<()>>,
  }

  impl WorkerHandle {
      /// Idempotent: safe to call more than once (`Drop` calls it too).
      pub fn stop(&mut self) {
          self.stop.store(true, Ordering::SeqCst);
          if let Some(handle) = self.join.take() {
              // A panicked worker thread has nothing further this handle
              // can do about it beyond having already requested `stop`.
              let _ = handle.join();
          }
      }

      pub fn is_running(&self) -> bool {
          self.join.as_ref().is_some_and(|handle| !handle.is_finished())
      }
  }

  impl Drop for WorkerHandle {
      fn drop(&mut self) {
          self.stop();
      }
  }
  ```
  This needs `use std::sync::Arc;` added to the top-level `use` block (it's
  already imported for `SharedState`'s constructor, so this is likely
  already present — verify, don't duplicate).
- [ ] Run `cargo test -p gemelli-gui handle_tests`. Expect
  `test result: ok. 3 passed; 0 failed;`.
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`.
- [ ] `git add crates/gui/src/worker.rs`, commit:
  ```
  feat(gui): add WorkerHandle with idempotent stop and Drop-triggered join
  ```

#### Cycle 8 — `WorkerSpec` + `spawn_worker` (thread creation, platform-gated publisher)

`spawn_worker` opens `NokhwaSource` and the publisher **on the spawned
thread**: neither needs to exist before the thread starts, and constructing
a `Camera` off-thread just to hand it across would add a `Send` requirement
for no benefit (`run_capture` never needs the source/publisher anywhere but
inside the thread that also runs the loop).

- [ ] Write the failing test — a pure, hardware-free check that `WorkerSpec`
  has the documented fields and that `spawn_worker` returns a
  `WorkerHandle`. (Anything that actually opens a camera or a Syphon server
  is hardware-dependent and deferred to the `#[ignore]`d tests below.) Add
  to `worker.rs`:
  ```rust
  #[cfg(test)]
  mod spawn_worker_tests {
      use std::sync::Arc;
      use std::sync::mpsc;

      use gemelli_core::transform::TransformConfig;

      use super::{SharedState, WorkerSpec, spawn_worker};

      #[test]
      fn worker_spec_holds_the_given_fields() {
          let spec = WorkerSpec {
              device_index: 2,
              requested_fps: Some(30),
              server_name: "gemelli".to_string(),
          };

          assert_eq!(spec.device_index, 2);
          assert_eq!(spec.requested_fps, Some(30));
          assert_eq!(spec.server_name, "gemelli");
      }

      #[test]
      #[ignore = "opens a real camera device (index 9999 is out of range on \
                  every real machine, but nokhwa still touches the OS camera \
                  subsystem to discover that, which is flaky/slow in CI); \
                  run manually with `cargo test -p gemelli-gui \
                  spawn_worker_open_failure -- --ignored`"]
      fn spawn_worker_open_failure() {
          let shared = Arc::new(SharedState::new(TransformConfig::default()));
          let (tx, rx) = mpsc::channel();
          let spec = WorkerSpec { device_index: 9999, requested_fps: None, server_name: "gemelli-test".to_string() };

          let mut handle = spawn_worker(spec, shared, tx);
          let error = rx.recv().expect("open failure must be sent on the errors channel");
          assert!(matches!(error, super::WorkerError::Capture(_)));

          handle.stop();
          assert!(!handle.is_running());
      }

      #[test]
      #[cfg(target_os = "macos")]
      #[ignore = "requires a real camera and a real macOS GPU/Syphon session; \
                  run manually with `cargo test -p gemelli-gui \
                  spawn_worker_publishes_real_frames -- --ignored`, then \
                  check a Syphon client (e.g. Syphon Recorder) sees \
                  \"gemelli-worker-smoke\" publishing"]
      fn spawn_worker_publishes_real_frames() {
          use std::thread;
          use std::time::Duration;
          use std::sync::atomic::Ordering;

          let shared = Arc::new(SharedState::new(TransformConfig::default()));
          let (tx, _rx) = mpsc::channel();
          let spec = WorkerSpec {
              device_index: 0,
              requested_fps: None,
              server_name: "gemelli-worker-smoke".to_string(),
          };

          let mut handle = spawn_worker(spec, Arc::clone(&shared), tx);
          thread::sleep(Duration::from_secs(3));

          assert!(shared.frames_published.load(Ordering::SeqCst) > 0);
          assert!(shared.latest_output.lock().unwrap().is_some());

          handle.stop();
          assert!(!handle.is_running());
      }
  }
  ```
- [ ] Run `cargo test -p gemelli-gui worker_spec_holds_the_given_fields`.
  Expect **compile failure**: `WorkerSpec` and `spawn_worker` unresolved.
- [ ] Minimal implementation. Add to `worker.rs`, after the `WorkerHandle`
  block and before `run_capture_tests`:
  ```rust
  #[cfg(target_os = "macos")]
  fn open_publisher(server_name: &str) -> Result<Box<dyn TexturePublisher>, PublishError> {
      let publisher = gemelli_syphon::SyphonPublisher::new(server_name)?;
      Ok(Box::new(publisher))
  }

  #[cfg(not(target_os = "macos"))]
  fn open_publisher(server_name: &str) -> Result<Box<dyn TexturePublisher>, PublishError> {
      Err(PublishError::ServerCreate {
          name: server_name.to_string(),
          reason: "Syphon/Spout publishing is not supported on this platform".to_string(),
      })
  }

  /// Parameters for one capture-thread run. Changing device, fps, or server
  /// name needs a fresh `NokhwaSource`/publisher, so the GUI stops the old
  /// worker and calls `spawn_worker` again with a new spec rather than
  /// mutating a running one.
  pub struct WorkerSpec {
      pub device_index: u32,
      pub requested_fps: Option<u32>,
      pub server_name: String,
  }

  /// Opens `NokhwaSource` and the publisher on the new thread; open
  /// failures are reported the same way as any other capture-loop error —
  /// sent on `errors`, thread ends without ever calling `run_capture`.
  pub fn spawn_worker(
      spec: WorkerSpec,
      shared: Arc<SharedState>,
      errors: mpsc::Sender<WorkerError>,
  ) -> WorkerHandle {
      let stop = Arc::new(AtomicBool::new(false));
      let thread_stop = Arc::clone(&stop);

      let join = std::thread::spawn(move || {
          let mut source = match gemelli_core::capture::NokhwaSource::open(spec.device_index, spec.requested_fps) {
              Ok(source) => source,
              Err(error) => {
                  let _ = errors.send(WorkerError::Capture(error));
                  return;
              }
          };

          let mut publisher = match open_publisher(&spec.server_name) {
              Ok(publisher) => publisher,
              Err(error) => {
                  let _ = errors.send(WorkerError::Publish(error));
                  return;
              }
          };

          run_capture(&mut source, publisher.as_mut(), &shared, &thread_stop, &errors);
      });

      WorkerHandle { stop, join: Some(join) }
  }
  ```
- [ ] Run `cargo test -p gemelli-gui worker_spec_holds_the_given_fields`.
  Expect `test result: ok. 1 passed; 0 failed; ... filtered out`.
- [ ] Run `cargo test -p gemelli-gui --workspace 2>&1 | tail -5` — confirm
  the two `#[ignore]`d tests are reported as `ignored`, not run, in the
  default test pass (`test result: ok. N passed; 0 failed; 2 ignored;`
  somewhere in the `gemelli-gui` block).
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`.
- [ ] `git add crates/gui/src/worker.rs`, commit:
  ```
  feat(gui): add WorkerSpec and spawn_worker with platform-gated publisher
  ```

**Manual verification note (mirrors `crates/core/tests/camera_smoke.rs` and
`crates/syphon/src/lib.rs`'s ignored test):** after this task lands, run on
a Mac with a camera attached and Syphon Recorder (or similar) open:
```
cargo test -p gemelli-gui spawn_worker_publishes_real_frames -- --ignored --nocapture
```
Confirm the client app shows a `gemelli-worker-smoke` server publishing
frames for the ~3 second window, and that `cargo test -p gemelli-gui
spawn_worker_open_failure -- --ignored` reports the `WorkerError::Capture`
variant on the channel without hanging or panicking.

---

### Task 5: crop_editor.rs

**Files:**
- Modify: `crates/gui/Cargo.toml` — verify/add `egui` as a direct dependency
- Modify: `Cargo.toml` (root) — verify/add `egui` to `[workspace.dependencies]`
- Create: `crates/gui/src/crop_editor.rs`
- Modify: `crates/gui/src/main.rs` — add `mod crop_editor;`
- Test: `crates/gui/src/crop_editor.rs` (`#[cfg(test)] mod` blocks)

**Interfaces:**
- Consumes: `gemelli_core::transform::CropRect` (Section A), `egui::{Pos2,
  Rect, Vec2, pos2, vec2}` (egui 0.35.0 — verified current version matching
  `eframe`'s bundled egui; see Step 0)
- Produces:
  ```rust
  pub struct CropMapping { pub frame_width: u32, pub frame_height: u32, pub draw: egui::Rect }
  impl CropMapping {
      pub fn to_screen(&self, rect: CropRect) -> egui::Rect;
      pub fn to_frame(&self, rect: egui::Rect) -> CropRect;
  }

  pub fn clamp_rect(rect: CropRect, frame_width: u32, frame_height: u32) -> CropRect;

  #[derive(Debug, Clone, Copy, PartialEq)]
  pub enum DragMode { Move, ResizeNw, ResizeNe, ResizeSw, ResizeSe }
  #[derive(Debug, Clone, Copy)]
  pub struct DragState { pub mode: DragMode, pub start_rect: CropRect, pub start_pointer: egui::Pos2 }
  pub fn apply_drag(state: &DragState, mapping: &CropMapping, pointer: egui::Pos2) -> CropRect;
  pub fn hit_test(rect_screen: egui::Rect, pointer: egui::Pos2) -> Option<DragMode>;
  ```

**Fixture used throughout (verified by actually executing the arithmetic in
a scratch Rust program with plain `f32`, not hand-computed — see below for
why that matters):** frame `1920x1080`; `draw = egui::Rect::from_min_size(
egui::pos2(100.0, 50.0), egui::vec2(640.0, 360.0))` (screen rect from
`(100,50)` to `(740,410)`, i.e. `640x360`, same `16:9` aspect as the frame,
so `scale_x == scale_y == 640.0/1920.0`). In `f32` this division is
`0.33333334`, **not** exactly `1/3` — `1/3` has no exact binary
floating-point representation. Every `to_screen`/`to_frame` value below was
produced by compiling and running the real arithmetic (not estimated), so
the rounding is already accounted for; do not "simplify" any of these
numbers when transcribing the tests.

#### Step 0 — Cargo.toml wiring + the sanctioned `as`-cast pair (not a TDD cycle for the Cargo.toml part; the coord functions get their own RED/GREEN below)

- [ ] Run `grep -n "^egui" crates/gui/Cargo.toml Cargo.toml`. If
  `crates/gui/Cargo.toml` has no `egui` line, add one. If root `Cargo.toml`'s
  `[workspace.dependencies]` has no `egui` line either, add
  `egui = "0.35"` there first (verified current version at plan-authoring
  time: `egui = "0.35.0"`, matching `eframe = "0.35.0"` exactly — pin the
  same version as `eframe`'s bundled egui, since mismatched versions produce
  incompatible `Rect`/`Pos2` types that won't type-check against
  `eframe::egui`'s types used elsewhere in this crate). Then in
  `crates/gui/Cargo.toml`'s `[dependencies]`, add:
  ```toml
  egui = { workspace = true }
  ```
  (If Task 1/3 already added a workspace-level `egui` entry and/or the
  crate-level line, leave them as-is — don't create a duplicate/conflicting
  version pin.)
- [ ] Create an empty `crates/gui/src/crop_editor.rs` (zero bytes).
- [ ] Edit `crates/gui/src/main.rs`: add `mod crop_editor;` alongside the
  existing `mod worker;` and any Task 1/3 `mod` lines (do not remove them).
- [ ] Run `cargo build --workspace`. Expect success.
- [ ] Write the failing test for the sanctioned coord-cast pair. Set
  `crates/gui/src/crop_editor.rs` to:
  ```rust
  #[cfg(test)]
  mod coord_tests {
      use super::{to_frame_coord, to_screen_coord};

      #[test]
      fn to_frame_coord_rounds_to_nearest() {
          assert_eq!(to_frame_coord(479.6), 480);
          assert_eq!(to_frame_coord(479.4), 479);
      }

      #[test]
      fn to_frame_coord_clamps_negative_to_zero() {
          assert_eq!(to_frame_coord(-3.0), 0);
      }

      #[test]
      fn to_screen_coord_is_a_lossless_widening() {
          assert_eq!(to_screen_coord(480), 480.0_f32);
          assert_eq!(to_screen_coord(0), 0.0_f32);
      }
  }
  ```
- [ ] Run `cargo test -p gemelli-gui coord_tests`. Expect **compile
  failure**: `error[E0432]: unresolved import `super::to_frame_coord``
  (and `to_screen_coord`).
- [ ] Minimal implementation. Prepend to `crop_editor.rs` (above the test
  module):
  ```rust
  //! Pure screen<->frame geometry for the crop-drag UI: coordinate
  //! conversion, clamping, and the corner/move drag state machine. No egui
  //! widget code lives here — `sidebar.rs`/`app.rs` call these functions
  //! and draw the result.

  use gemelli_core::transform::CropRect;

  /// The only `as` casts in this module: f32 (egui screen space) <-> u32
  /// (frame pixel space). `f32 -> u32` has no infallible `TryFrom` in std
  /// (same gap core's `transform::scale::scale_dimension` hits for
  /// `f64 -> u32`), so `round()` + `clamp()` bound the value into u32's
  /// range before the cast. `u32 -> f32` has no infallible std conversion
  /// either (`From<u32> for f32` doesn't exist — only `u8`/`u16` do); frame
  /// dimensions never approach 2^24 px, so the precision loss the cast can
  /// introduce above that threshold is immaterial here.
  #[allow(clippy::as_conversions)]
  fn to_frame_coord(v: f32) -> u32 {
      v.round().clamp(0.0, u32::MAX as f32) as u32
  }

  #[allow(clippy::as_conversions)]
  fn to_screen_coord(v: u32) -> f32 {
      v as f32
  }
  ```
- [ ] Run `cargo test -p gemelli-gui coord_tests`. Expect
  `test result: ok. 3 passed; 0 failed;`.
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`
  (confirms the two `#[allow(clippy::as_conversions)]` are the *only* ones
  needed — if any other file in the crate has a bare `as`, this step fails).
- [ ] `git add Cargo.toml crates/gui/Cargo.toml crates/gui/src/crop_editor.rs crates/gui/src/main.rs`,
  commit:
  ```
  chore(gui): wire up crop_editor module and egui dependency
  feat(gui): add sanctioned f32<->u32 coord cast pair
  ```
  (Two logical changes in one commit is acceptable here only because the
  Cargo.toml wiring has no independent test of its own — if your git
  history convention prefers separate commits, split into `chore(gui): wire
  up crop_editor module and egui dependency` and `feat(gui): add sanctioned
  f32<->u32 coord cast pair` instead.)

#### Cycle 1 — `CropMapping::to_screen`

- [ ] Write the failing test. Add to `crop_editor.rs`:
  ```rust
  #[cfg(test)]
  mod crop_mapping_tests {
      use gemelli_core::transform::CropRect;

      use super::CropMapping;

      fn fixture_mapping() -> CropMapping {
          CropMapping {
              frame_width: 1920,
              frame_height: 1080,
              draw: egui::Rect::from_min_size(egui::pos2(100.0, 50.0), egui::vec2(640.0, 360.0)),
          }
      }

      #[test]
      fn to_screen_scales_and_offsets_by_the_draw_rect() {
          let mapping = fixture_mapping();
          let rect = CropRect { width: 960, height: 540, x: 480, y: 270 };

          let screen = mapping.to_screen(rect);

          assert_eq!(screen.min, egui::pos2(260.0, 140.0));
          assert_eq!(screen.max, egui::pos2(580.0, 320.0));
      }
  }
  ```
- [ ] Run `cargo test -p gemelli-gui crop_mapping_tests`. Expect **compile
  failure**: `CropMapping` unresolved.
- [ ] Minimal implementation. Add to `crop_editor.rs`, after the coord
  functions:
  ```rust
  /// Maps a `CropRect` (frame pixel coords) into the preview draw rect
  /// (screen coords) and back.
  pub struct CropMapping {
      pub frame_width: u32,
      pub frame_height: u32,
      pub draw: egui::Rect,
  }

  impl CropMapping {
      fn scale_factors(&self) -> (f32, f32) {
          (
              self.draw.width() / to_screen_coord(self.frame_width),
              self.draw.height() / to_screen_coord(self.frame_height),
          )
      }

      pub fn to_screen(&self, rect: CropRect) -> egui::Rect {
          let (scale_x, scale_y) = self.scale_factors();
          let min = egui::pos2(
              self.draw.min.x + to_screen_coord(rect.x) * scale_x,
              self.draw.min.y + to_screen_coord(rect.y) * scale_y,
          );
          let size = egui::vec2(to_screen_coord(rect.width) * scale_x, to_screen_coord(rect.height) * scale_y);
          egui::Rect::from_min_size(min, size)
      }
  }
  ```
- [ ] Run `cargo test -p gemelli-gui crop_mapping_tests`. Expect
  `test result: ok. 1 passed; 0 failed;`.
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`.
- [ ] `git add crates/gui/src/crop_editor.rs`, commit:
  ```
  feat(gui): add CropMapping::to_screen frame->screen mapping
  ```

#### Cycle 2 — `clamp_rect` (needed by `to_frame`, so it comes first)

- [ ] Write the failing test. Add to `crop_editor.rs`:
  ```rust
  #[cfg(test)]
  mod clamp_rect_tests {
      use gemelli_core::transform::CropRect;

      use super::clamp_rect;

      const FRAME_W: u32 = 1920;
      const FRAME_H: u32 = 1080;

      #[test]
      fn below_minimum_size_grows_to_16x16() {
          let rect = CropRect { width: 10, height: 10, x: 0, y: 0 };
          assert_eq!(clamp_rect(rect, FRAME_W, FRAME_H), CropRect { width: 16, height: 16, x: 0, y: 0 });
      }

      #[test]
      fn overflow_right_edge_slides_x_left() {
          let rect = CropRect { width: 100, height: 100, x: 1900, y: 0 };
          assert_eq!(clamp_rect(rect, FRAME_W, FRAME_H), CropRect { width: 100, height: 100, x: 1820, y: 0 });
      }

      #[test]
      fn overflow_bottom_edge_slides_y_up() {
          let rect = CropRect { width: 100, height: 100, x: 0, y: 1060 };
          assert_eq!(clamp_rect(rect, FRAME_W, FRAME_H), CropRect { width: 100, height: 100, x: 0, y: 980 });
      }

      #[test]
      fn oversize_both_dimensions_shrinks_to_full_frame() {
          let rect = CropRect { width: 3000, height: 3000, x: 0, y: 0 };
          assert_eq!(clamp_rect(rect, FRAME_W, FRAME_H), CropRect { width: FRAME_W, height: FRAME_H, x: 0, y: 0 });
      }

      #[test]
      fn already_valid_rect_is_unchanged() {
          let rect = CropRect { width: 960, height: 540, x: 480, y: 270 };
          assert_eq!(clamp_rect(rect, FRAME_W, FRAME_H), rect);
      }
  }
  ```
- [ ] Run `cargo test -p gemelli-gui clamp_rect_tests`. Expect **compile
  failure**: `clamp_rect` unresolved.
- [ ] Minimal implementation. Add to `crop_editor.rs`, after `CropMapping`'s
  `impl` block:
  ```rust
  const MIN_CROP_SIDE: u32 = 16;

  /// Normalizes a (possibly drag-produced) rect: at least `16x16` frame
  /// px, fully inside `[0, frame_width) x [0, frame_height)`. Width/height
  /// are bounded first so the position clamp's `frame_width - width`
  /// subtraction can never underflow.
  pub fn clamp_rect(rect: CropRect, frame_width: u32, frame_height: u32) -> CropRect {
      let width = rect.width.clamp(MIN_CROP_SIDE.min(frame_width), frame_width);
      let height = rect.height.clamp(MIN_CROP_SIDE.min(frame_height), frame_height);
      let x = rect.x.min(frame_width - width);
      let y = rect.y.min(frame_height - height);
      CropRect { width, height, x, y }
  }
  ```
- [ ] Run `cargo test -p gemelli-gui clamp_rect_tests`. Expect
  `test result: ok. 5 passed; 0 failed;`.
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`.
- [ ] `git add crates/gui/src/crop_editor.rs`, commit:
  ```
  feat(gui): add clamp_rect for min-size and bounds enforcement
  ```

#### Cycle 3 — `CropMapping::to_frame` + round-trip identity

- [ ] Write the failing test. Add to `crop_mapping_tests`:
  ```rust
      #[test]
      fn to_frame_is_the_inverse_of_to_screen() {
          let mapping = fixture_mapping();
          let original = CropRect { width: 960, height: 540, x: 480, y: 270 };

          let round_tripped = mapping.to_frame(mapping.to_screen(original));

          // The scale factor (640/1920) has no exact f32 representation, but
          // to_frame_coord's round() absorbs that error at these magnitudes
          // — verified by running the real arithmetic (see the fixture note
          // above), not assumed. Assert within 1px per the contract, even
          // though this fixture happens to land exactly on `original`.
          assert!(round_tripped.x.abs_diff(original.x) <= 1);
          assert!(round_tripped.y.abs_diff(original.y) <= 1);
          assert!(round_tripped.width.abs_diff(original.width) <= 1);
          assert!(round_tripped.height.abs_diff(original.height) <= 1);
      }

      #[test]
      fn to_frame_clamps_a_rect_that_overhangs_the_draw_area() {
          let mapping = fixture_mapping();
          // Screen rect starting right at the draw origin but 3x too wide/
          // tall for the frame at this scale (960 screen px / (1/3 scale)
          // = 2880 frame px > 1920 frame_width) — must clamp into bounds.
          let overhanging = egui::Rect::from_min_size(mapping.draw.min, egui::vec2(960.0, 540.0));

          let frame_rect = mapping.to_frame(overhanging);

          assert_eq!(frame_rect, CropRect { width: 1920, height: 1080, x: 0, y: 0 });
      }
  ```
- [ ] Run `cargo test -p gemelli-gui crop_mapping_tests`. Expect **compile
  failure**: `no method named `to_frame` found for struct `CropMapping``.
- [ ] Minimal implementation. Add to `CropMapping`'s `impl` block, after
  `to_screen`:
  ```rust
      pub fn to_frame(&self, rect: egui::Rect) -> CropRect {
          let (scale_x, scale_y) = self.scale_factors();
          let x = to_frame_coord((rect.min.x - self.draw.min.x) / scale_x);
          let y = to_frame_coord((rect.min.y - self.draw.min.y) / scale_y);
          let width = to_frame_coord(rect.width() / scale_x);
          let height = to_frame_coord(rect.height() / scale_y);
          clamp_rect(CropRect { width, height, x, y }, self.frame_width, self.frame_height)
      }
  ```
- [ ] Run `cargo test -p gemelli-gui crop_mapping_tests`. Expect
  `test result: ok. 3 passed; 0 failed;`.
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`.
- [ ] `git add crates/gui/src/crop_editor.rs`, commit:
  ```
  feat(gui): add CropMapping::to_frame with bounds clamping
  ```

#### Cycle 4 — `DragMode`/`DragState` + `apply_drag`

Each arm moves only its own corner and leaves the opposite corner fixed
(`Move` translates both corners equally, so size is preserved). All five
expected values below came from actually running this exact match
arm-by-arm in the scratch verification program — see the arithmetic
sidebar after the code if you want to re-derive `ResizeSe`'s numbers by
hand as a sanity check; the other three follow the same pattern.

- [ ] Write the failing test. Add to `crop_editor.rs`:
  ```rust
  #[cfg(test)]
  mod apply_drag_tests {
      use gemelli_core::transform::CropRect;

      use super::{CropMapping, DragMode, DragState, apply_drag};

      fn fixture_mapping() -> CropMapping {
          CropMapping {
              frame_width: 1920,
              frame_height: 1080,
              draw: egui::Rect::from_min_size(egui::pos2(100.0, 50.0), egui::vec2(640.0, 360.0)),
          }
      }

      // start_rect's screen projection is (260,140)-(580,320) under
      // fixture_mapping — see Cycle 1's to_screen test.
      fn start_rect() -> CropRect {
          CropRect { width: 960, height: 540, x: 480, y: 270 }
      }

      #[test]
      fn move_translates_without_changing_size() {
          let mapping = fixture_mapping();
          let start_pointer = egui::pos2(420.0, 230.0); // center of the screen rect
          let state = DragState { mode: DragMode::Move, start_rect: start_rect(), start_pointer };

          let result = apply_drag(&state, &mapping, egui::pos2(435.0, 239.0)); // +15,+9 screen px

          assert_eq!(result, CropRect { width: 960, height: 540, x: 525, y: 297 });
      }

      #[test]
      fn resize_se_grows_from_the_fixed_top_left_corner() {
          let mapping = fixture_mapping();
          let start_pointer = egui::pos2(580.0, 320.0); // screen rect's max corner
          let state = DragState { mode: DragMode::ResizeSe, start_rect: start_rect(), start_pointer };

          let result = apply_drag(&state, &mapping, egui::pos2(610.0, 350.0)); // +30,+30 screen px

          assert_eq!(result, CropRect { width: 1050, height: 630, x: 480, y: 270 });
      }

      #[test]
      fn resize_nw_moves_the_origin_and_shrinks_from_the_fixed_bottom_right_corner() {
          let mapping = fixture_mapping();
          let start_pointer = egui::pos2(260.0, 140.0); // screen rect's min corner
          let state = DragState { mode: DragMode::ResizeNw, start_rect: start_rect(), start_pointer };

          let result = apply_drag(&state, &mapping, egui::pos2(290.0, 170.0)); // +30,+30 screen px

          assert_eq!(result, CropRect { width: 870, height: 450, x: 570, y: 360 });
      }

      #[test]
      fn resize_ne_moves_the_top_edge_and_grows_the_right_edge() {
          let mapping = fixture_mapping();
          let start_pointer = egui::pos2(580.0, 140.0); // top-right corner
          let state = DragState { mode: DragMode::ResizeNe, start_rect: start_rect(), start_pointer };

          let result = apply_drag(&state, &mapping, egui::pos2(610.0, 110.0)); // +30 right, -30 up screen px

          assert_eq!(result, CropRect { width: 1050, height: 630, x: 480, y: 180 });
      }

      #[test]
      fn resize_sw_moves_the_left_edge_and_grows_the_bottom_edge() {
          let mapping = fixture_mapping();
          let start_pointer = egui::pos2(260.0, 320.0); // bottom-left corner
          let state = DragState { mode: DragMode::ResizeSw, start_rect: start_rect(), start_pointer };

          let result = apply_drag(&state, &mapping, egui::pos2(230.0, 350.0)); // -30 left, +30 down screen px

          assert_eq!(result, CropRect { width: 1050, height: 630, x: 390, y: 270 });
      }
  }
  ```
- [ ] Run `cargo test -p gemelli-gui apply_drag_tests`. Expect **compile
  failure**: `DragMode`, `DragState`, and `apply_drag` all unresolved.
- [ ] Minimal implementation. Add to `crop_editor.rs`, after `clamp_rect`:
  ```rust
  /// Drag interaction state. Exhaustive-matched everywhere it's consumed —
  /// no `_` arm — so adding a sixth handle is a compile error at every call
  /// site until it's handled, not a silent no-op.
  #[derive(Debug, Clone, Copy, PartialEq)]
  pub enum DragMode {
      Move,
      ResizeNw,
      ResizeNe,
      ResizeSw,
      ResizeSe,
  }

  #[derive(Debug, Clone, Copy)]
  pub struct DragState {
      pub mode: DragMode,
      pub start_rect: CropRect,
      pub start_pointer: egui::Pos2,
  }

  /// Given a drag delta in screen coords, produces the new (clamped)
  /// `CropRect`. Each resize arm moves only its own corner, leaving the
  /// opposite corner fixed; `Move` translates both corners equally.
  pub fn apply_drag(state: &DragState, mapping: &CropMapping, pointer: egui::Pos2) -> CropRect {
      let delta = pointer - state.start_pointer;
      let start_screen = mapping.to_screen(state.start_rect);
      let new_screen = match state.mode {
          DragMode::Move => start_screen.translate(delta),
          DragMode::ResizeNw => egui::Rect::from_min_max(start_screen.min + delta, start_screen.max),
          DragMode::ResizeNe => egui::Rect::from_min_max(
              egui::pos2(start_screen.min.x, start_screen.min.y + delta.y),
              egui::pos2(start_screen.max.x + delta.x, start_screen.max.y),
          ),
          DragMode::ResizeSw => egui::Rect::from_min_max(
              egui::pos2(start_screen.min.x + delta.x, start_screen.min.y),
              egui::pos2(start_screen.max.x, start_screen.max.y + delta.y),
          ),
          DragMode::ResizeSe => egui::Rect::from_min_max(start_screen.min, start_screen.max + delta),
      };
      mapping.to_frame(new_screen)
  }
  ```
- [ ] Run `cargo test -p gemelli-gui apply_drag_tests`. Expect
  `test result: ok. 5 passed; 0 failed;`.
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`.
- [ ] `git add crates/gui/src/crop_editor.rs`, commit:
  ```
  feat(gui): add DragMode/DragState and apply_drag corner/move math
  ```

  **Hand-check sidebar for `resize_se_grows_from_the_fixed_top_left_corner`**
  (the others follow identically): `start_screen` = `(260,140)-(580,320)`
  (Cycle 1's fixture result). `ResizeSe` keeps `min` fixed and adds `delta =
  (30,30)` to `max`, giving `new_screen = (260,140)-(610,350)`. `to_frame`
  with `scale ≈ 0.33333334`: `x = round((260-100)/0.33333334) = round(479.9…)
  = 480` (unchanged, as expected — `min` didn't move); `width =
  round((610-260)/0.33333334) = round(1050.0…) = 1050`; same pattern for `y`
  (unchanged, `270`) and `height` (`630`). `clamp_rect` leaves it unchanged
  since `480+1050=1530 <= 1920` and `270+630=900 <= 1080`. Matches the
  asserted `CropRect { width: 1050, height: 630, x: 480, y: 270 }`.

#### Cycle 5 — `hit_test`

- [ ] Write the failing test. Add to `crop_editor.rs`:
  ```rust
  #[cfg(test)]
  mod hit_test_tests {
      use super::{DragMode, hit_test};

      // Same screen rect as apply_drag_tests: (260,140)-(580,320).
      fn rect_screen() -> egui::Rect {
          egui::Rect::from_min_max(egui::pos2(260.0, 140.0), egui::pos2(580.0, 320.0))
      }

      #[test]
      fn exact_corner_hits_return_the_matching_resize_mode() {
          let rect = rect_screen();
          assert_eq!(hit_test(rect, egui::pos2(260.0, 140.0)), Some(DragMode::ResizeNw));
          assert_eq!(hit_test(rect, egui::pos2(580.0, 320.0)), Some(DragMode::ResizeSe));
          assert_eq!(hit_test(rect, egui::pos2(580.0, 140.0)), Some(DragMode::ResizeNe));
          assert_eq!(hit_test(rect, egui::pos2(260.0, 320.0)), Some(DragMode::ResizeSw));
      }

      #[test]
      fn point_within_8px_of_a_corner_but_outside_the_rect_still_hits_the_handle() {
          let rect = rect_screen();
          assert_eq!(hit_test(rect, egui::pos2(255.0, 135.0)), Some(DragMode::ResizeNw));
      }

      #[test]
      fn corner_handle_takes_priority_over_move_when_both_would_match() {
          let rect = rect_screen();
          // (265,145) is inside the rect (contains() would say Move) AND
          // within the 8px NW handle box — the handle must win.
          assert_eq!(hit_test(rect, egui::pos2(265.0, 145.0)), Some(DragMode::ResizeNw));
      }

      #[test]
      fn point_on_an_edge_but_far_from_any_corner_is_move() {
          let rect = rect_screen();
          assert_eq!(hit_test(rect, egui::pos2(420.0, 140.0)), Some(DragMode::Move));
      }

      #[test]
      fn point_inside_and_far_from_every_corner_is_move() {
          let rect = rect_screen();
          assert_eq!(hit_test(rect, egui::pos2(420.0, 230.0)), Some(DragMode::Move));
      }

      #[test]
      fn point_outside_the_rect_and_every_handle_is_none() {
          let rect = rect_screen();
          assert_eq!(hit_test(rect, egui::pos2(1000.0, 1000.0)), None);
      }
  }
  ```
- [ ] Run `cargo test -p gemelli-gui hit_test_tests`. Expect **compile
  failure**: `hit_test` unresolved.
- [ ] Minimal implementation. Add to `crop_editor.rs`, after `apply_drag`:
  ```rust
  const HANDLE_PX: f32 = 8.0;

  /// 8px axis-aligned square hit box, not a circular radius — avoids a
  /// `sqrt` in a per-repaint hit test and gives exact, hand-checkable
  /// pixel boundaries.
  fn hits_corner(pointer: egui::Pos2, corner: egui::Pos2) -> bool {
      (pointer.x - corner.x).abs() <= HANDLE_PX && (pointer.y - corner.y).abs() <= HANDLE_PX
  }

  /// Which handle (or the move area) a pointer press grabs. Corner handles
  /// are checked before the move-area fallback, so a point inside both a
  /// corner's hit box and the rect's interior always resolves to the
  /// corner.
  pub fn hit_test(rect_screen: egui::Rect, pointer: egui::Pos2) -> Option<DragMode> {
      let corners = [
          (rect_screen.min, DragMode::ResizeNw),
          (egui::pos2(rect_screen.max.x, rect_screen.min.y), DragMode::ResizeNe),
          (egui::pos2(rect_screen.min.x, rect_screen.max.y), DragMode::ResizeSw),
          (rect_screen.max, DragMode::ResizeSe),
      ];
      for (corner, mode) in corners {
          if hits_corner(pointer, corner) {
              return Some(mode);
          }
      }
      if rect_screen.contains(pointer) {
          return Some(DragMode::Move);
      }
      None
  }
  ```
- [ ] Run `cargo test -p gemelli-gui hit_test_tests`. Expect
  `test result: ok. 6 passed; 0 failed;`.
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`.
- [ ] `git add crates/gui/src/crop_editor.rs`, commit:
  ```
  feat(gui): add hit_test with corner-priority handle detection
  ```

#### Final check for this section

- [ ] Run `cargo test -p gemelli-gui` (no filter) from the repo root.
  Expect all `worker.rs` and `crop_editor.rs` tests green, with exactly 2
  `ignored` (the hardware-dependent `spawn_worker` tests from Task 4, Cycle
  8 — confirm none from Task 5, since `crop_editor.rs` has no
  hardware-dependent paths at all).
- [ ] Run the full lint gate once more:
  `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`.
## Section C — Tasks 6–8 (crates/gui: app, sidebar, crop interaction, E2E)

Continues from Tasks 1–5 (crate scaffold, `theme.rs`, `preview.rs`, `fps_meter.rs`, `worker.rs`,
`crop_editor.rs` — per `GUI-CONTRACT.md`). All API signatures for `egui`/`eframe` below were
verified against the actual `egui 0.35.0` / `eframe 0.35.0` source (current latest, confirmed via
`cargo add --dry-run` against this workspace) — not assumed from memory. Sources checked:
`crates/egui/src/{context.rs, ui.rs, response.rs, widgets/{image.rs,drag_value.rs,slider.rs},
containers/combo_box.rs, load.rs}` at tag `0.35.0` in `emilk/egui`.

Workspace-wide rules that apply to every step below (restated from `GUI-CONTRACT.md`): edition
2024; `cargo clippy --workspace --all-targets -- -D warnings` denies `unwrap_used`,
`expect_used`, `as_conversions` with **no test exemption** for `as_conversions` — every `Result`
from a `Mutex::lock()` in this task is resolved via `.unwrap_or_else(std::sync::PoisonError::into_inner)`
(a combinator, not `unwrap`/`expect` — it recovers the guard even if the capture thread panicked
while holding the lock, so the GUI degrades instead of also panicking); exhaustive `match` without
`_` on owned enums (`PreviewMode`, the `flip_from_toggles` mapping, `CropAction` below); early-return
guards; function names ≤3 words. No `as` casts appear anywhere in Tasks 6–8 — every u32⇄u32 or
usize→u32 conversion that would otherwise need one is avoided by threading `Frame::width()`/
`height()` (already `u32`) through instead of round-tripping via `egui::TextureHandle::size()`
(`[usize; 2]`).

---

### Task 6: App state, sidebar, and status bar wiring

Builds `crates/gui/src/app.rs` (`GemelliApp`, `PreviewMode`, the pure config-rebuild functions, and
the full `eframe::App::update()` loop) and `crates/gui/src/sidebar.rs` (device / rotate / flip /
scale / server-name / transport widgets — pure widget functions that borrow only the fields they
touch, per the contract's "pure-ish: take `&mut` ...-like structs" note). Crop editing UI is
deliberately **not** added yet — Task 7 owns it, and inserts it into the exact call sites marked
below.

**Files:**
- `crates/gui/src/app.rs` (new — currently only exists as a `main.rs` placeholder; this task
  creates the module)
- `crates/gui/src/sidebar.rs` (new)
- `crates/gui/src/main.rs` (rewritten — replaces the `println!` placeholder with the real eframe
  bootstrap)
- `crates/gui/Cargo.toml` (verified/updated — see 6.0)

**Interfaces (consumed, exact signatures from Tasks 1–5 per `GUI-CONTRACT.md`):**
```rust
// crate::worker (Task 4)
pub struct SharedState { pub transform: arc_swap::ArcSwap<TransformConfig>,
    pub latest_output: std::sync::Mutex<Option<Frame>>,
    pub latest_raw: std::sync::Mutex<Option<Frame>>,
    pub frames_published: std::sync::atomic::AtomicU64 }
impl SharedState { pub fn new(config: TransformConfig) -> Self; }
pub enum WorkerError { Capture(CaptureError), Transform(TransformError), Publish(PublishError) }
pub struct WorkerHandle;
impl WorkerHandle { pub fn stop(&mut self); pub fn is_running(&self) -> bool; }
pub struct WorkerSpec { pub device_index: u32, pub requested_fps: Option<u32>, pub server_name: String }
pub fn spawn_worker(spec: WorkerSpec, shared: std::sync::Arc<SharedState>,
    errors: std::sync::mpsc::Sender<WorkerError>) -> WorkerHandle;

// crate::preview (Task 2)
pub fn color_image(frame: &Frame) -> egui::ColorImage;
pub fn fit_rect(frame_width: u32, frame_height: u32, avail: egui::Rect) -> egui::Rect;

// crate::fps_meter (Task 3)
pub struct FpsMeter;
impl FpsMeter { pub fn new() -> Self; pub fn record(&mut self, now: std::time::Instant);
    pub fn rate(&mut self, now: std::time::Instant) -> f32; }

// crate::theme (Task 5)
pub fn apply_theme(ctx: &egui::Context);
pub mod tokens { pub const DANGER: egui::Color32; pub const ACCENT_PUBLISH: egui::Color32;
    pub const ACCENT_IDLE: egui::Color32; /* + others unused by Task 6 */ }

// gemelli_core (Phase 1, unchanged)
pub mod capture { pub struct DeviceInfo { pub index: u32, pub name: String }
    pub fn list_devices() -> Result<Vec<DeviceInfo>, CaptureError>; }
pub mod frame { pub struct Frame; impl Frame { pub fn width(&self) -> u32; pub fn height(&self) -> u32; } }
pub mod transform { pub struct CropRect { pub width: u32, pub height: u32, pub x: u32, pub y: u32 }
    pub enum Rotation { R0, R90, R180, R270 }
    pub enum Flip { Keep, Horizontal, Vertical, Both }
    pub enum ScaleSpec { Exact { width: u32, height: u32 }, Factor(f64) }
    pub struct TransformConfig { pub crop: Option<CropRect>, pub rotation: Rotation,
        pub flip: Flip, pub scale: Option<ScaleSpec> } }
```

**Produces (consumed by Task 7):**
```rust
// crate::app
pub enum PreviewMode { Output, CropEdit }         // Eq, Copy
pub struct GemelliApp { /* private fields, see 6.5 */ }
impl GemelliApp { pub fn new(cc: &eframe::CreationContext<'_>) -> Self; }
impl eframe::App for GemelliApp { fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame); }
// private: fn flip_from_toggles(h: bool, v: bool) -> Flip
// private: fn build_transform(crop, rotation, flip_h, flip_v, scale_input) -> TransformConfig

// crate::sidebar
pub(crate) enum ScaleInput { Off, Factor(f64), Exact { width: u32, height: u32 } }  // Default = Off
pub(crate) fn scale_from_input(input: ScaleInput) -> Option<ScaleSpec>;
pub(crate) fn device_panel(ui: &mut egui::Ui, devices: &[DeviceInfo], selected: &mut usize) -> bool;
pub(crate) fn refresh_button(ui: &mut egui::Ui) -> bool;
pub(crate) fn rotate_panel(ui: &mut egui::Ui, rotation: &mut Rotation) -> bool;
pub(crate) fn flip_panel(ui: &mut egui::Ui, flip_h: &mut bool, flip_v: &mut bool) -> bool;
pub(crate) fn scale_panel(ui: &mut egui::Ui, scale_input: &mut ScaleInput) -> bool;
pub(crate) fn server_name_panel(ui: &mut egui::Ui, server_name: &mut String) -> bool;
pub(crate) fn transport_button(ui: &mut egui::Ui, running: bool) -> bool;
```
All `bool` returns above mean "this widget's value changed this frame" (`Response::changed()` /
`clicked()`), so `app.rs` can decide, at a single call site, whether to `push_transform()` /
restart the worker — the widgets themselves never touch `SharedState` or `WorkerHandle`.

---

#### 6.0 Verify/update `crates/gui/Cargo.toml`

The crate currently only depends on `gemelli-core` (a Task-1 placeholder). Confirm the exact
current major versions before pinning (per contract: "verify current versions with
`cargo add --dry-run`"):

- [ ] Run:
  ```bash
  cd crates/gui
  cargo add --dry-run eframe
  cargo add --dry-run arc-swap
  ```
  Expected: `Adding eframe v0.35.x to dependencies` and `Adding arc-swap v1.9.x to dependencies`
  (any `0.35.x` / `1.9.x` is fine — pin the minor version actually printed, not necessarily `.0`).

- [ ] Edit `crates/gui/Cargo.toml` to (values below assume the dry-run above printed `0.35` /
  `1.9` — substitute the versions actually printed if different):
  ```toml
  [package]
  name = "gemelli-gui"
  version = "0.1.0"
  edition.workspace = true
  license.workspace = true
  repository.workspace = true

  [lints]
  workspace = true

  [[bin]]
  name = "gemelli-gui"
  path = "src/main.rs"

  [dependencies]
  gemelli-core = { path = "../core" }
  eframe = "0.35"
  arc-swap = "1.9"
  thiserror = { workspace = true }

  [target.'cfg(target_os = "macos")'.dependencies]
  gemelli-syphon = { path = "../syphon" }
  ```
  If Task 1 already added these exact entries, this step is a no-op confirmation — diff the file
  before editing and skip if already correct.

- [ ] Run `cargo build -p gemelli-gui` (macOS: requires `vendor/Syphon.framework` built per the
  README Setup section, same prerequisite as `gemelli-cli`). Expected: fails to compile only
  because `src/main.rs` still has stale content, or succeeds trivially if `main.rs` is still the
  Task-1 placeholder — either is fine at this checkpoint; 6.7 replaces `main.rs`.

- [ ] Commit (see 6.8) only after 6.7's `cargo run` verification passes — this step's edit is
  folded into that commit, not committed alone.

#### 6.1 Pure fn: `flip_from_toggles` (TDD)

**RED** — add to `crates/gui/src/app.rs` (the file does not exist yet; this is its first
content):
```rust
//! GUI application state and the eframe update loop.

#[cfg(test)]
mod tests {
    use gemelli_core::transform::Flip;

    use super::flip_from_toggles;

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
}
```
- [ ] Run:
  ```bash
  cargo test -p gemelli-gui flip_from_toggles
  ```
  Expected: **compile error** — `cannot find function `flip_from_toggles` in this scope`. This
  confirms RED (the test exists and fails to even build, since the function is missing).

**GREEN** — add the implementation above the test module:
```rust
use gemelli_core::transform::Flip;

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
```
- [ ] Run:
  ```bash
  cargo test -p gemelli-gui flip_from_toggles
  ```
  Expected:
  ```
  running 1 test
  test app::tests::flip_from_toggles_covers_all_four_combinations ... ok

  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
  ```

#### 6.2 Pure fn: `ScaleInput` → `Option<ScaleSpec>` (TDD)

`ScaleInput` models the sidebar's scale widget (mutually-exclusive Off / Factor-slider / WxH
fields) — it lives in `sidebar.rs` since it's the widget's own input shape, not a general-purpose
core type. The contract flags one open question explicitly: what does zero width/height in the WxH
fields mean? **Decision: clamp to 1×1, do not fall back to `Off`.** A `DragValue` can pass through
`0` transiently while the user is actively dragging it upward from empty — treating that as "turn
scaling off" would silently discard the user's in-progress edit and flip a different part of the
UI (the mode radio buttons) out from under them. Clamping keeps the widget's mode selection
authoritative; only the numeric *value* is defended.

**RED** — add to `crates/gui/src/sidebar.rs` (new file):
```rust
//! Left-panel widgets. Each function borrows only the fields it needs — none of them touch
//! `SharedState` or `WorkerHandle` directly; `app.rs` owns all side effects.

use gemelli_core::transform::ScaleSpec;

/// Scale widget's own input shape — mutually exclusive Off / Factor / Exact, mirroring the three
/// controls the sidebar shows. Maps down to `Option<ScaleSpec>` via `scale_from_input`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub(crate) enum ScaleInput {
    #[default]
    Off,
    Factor(f64),
    Exact { width: u32, height: u32 },
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
```
- [ ] Run:
  ```bash
  cargo test -p gemelli-gui scale_from_input
  ```
  Expected: compile error — `cannot find function `scale_from_input` in this scope`. RED confirmed.

**GREEN** — add above the test module:
```rust
const SCALE_FACTOR_MIN: f64 = 0.1;
const SCALE_FACTOR_MAX: f64 = 2.0;

/// Pure mapping from the widget's input shape to core's `ScaleSpec`. See the doc comment above
/// `ScaleInput` for why zero WxH clamps to 1×1 instead of collapsing to `None`.
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
```
- [ ] Run:
  ```bash
  cargo test -p gemelli-gui scale_from_input
  ```
  Expected:
  ```
  running 6 tests
  test sidebar::tests::scale_from_input_off_is_none ... ok
  test sidebar::tests::scale_from_input_factor_within_range_passes_through ... ok
  test sidebar::tests::scale_from_input_factor_clamps_below_minimum ... ok
  test sidebar::tests::scale_from_input_factor_clamps_above_maximum ... ok
  test sidebar::tests::scale_from_input_exact_zero_dims_clamp_to_one ... ok
  test sidebar::tests::scale_from_input_exact_normal_dims_pass_through ... ok

  test result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
  ```

#### 6.3 Pure fn: `build_transform` (TDD)

**RED** — add to `crates/gui/src/app.rs`'s test module (extend the `use` list and add two tests):
```rust
#[cfg(test)]
mod tests {
    use gemelli_core::transform::{CropRect, Flip, Rotation, ScaleSpec, TransformConfig};

    use super::{build_transform, flip_from_toggles};
    use crate::sidebar::ScaleInput;

    // ... flip_from_toggles_covers_all_four_combinations stays above ...

    #[test]
    fn build_transform_assembles_all_fields() {
        let crop = Some(CropRect { width: 100, height: 80, x: 10, y: 5 });
        let config =
            build_transform(crop, Rotation::R90, true, false, ScaleInput::Factor(0.5));

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
```
- [ ] Run:
  ```bash
  cargo test -p gemelli-gui build_transform
  ```
  Expected: compile error — `cannot find function `build_transform` in this scope`. RED confirmed.

**GREEN** — add above the test module (below `flip_from_toggles`):
```rust
use crate::sidebar::{self, ScaleInput};

/// Rebuilds the full `TransformConfig` from the sidebar's current widget state. Called after
/// every widget edit that affects the transform chain; the result is stored into
/// `shared.transform` by `GemelliApp::push_transform` (6.5).
fn build_transform(
    crop: Option<gemelli_core::transform::CropRect>,
    rotation: gemelli_core::transform::Rotation,
    flip_h: bool,
    flip_v: bool,
    scale_input: ScaleInput,
) -> gemelli_core::transform::TransformConfig {
    gemelli_core::transform::TransformConfig {
        crop,
        rotation,
        flip: flip_from_toggles(flip_h, flip_v),
        scale: sidebar::scale_from_input(scale_input),
    }
}
```
- [ ] Run:
  ```bash
  cargo test -p gemelli-gui build_transform
  ```
  Expected:
  ```
  running 2 tests
  test app::tests::build_transform_assembles_all_fields ... ok
  test app::tests::build_transform_defaults_to_no_op ... ok

  test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
  ```

- [ ] Run the full pure-fn suite for this task together:
  ```bash
  cargo test -p gemelli-gui
  ```
  Expected: `test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` (1 from
  6.1 + 6 from 6.2 + 2 from 6.3 — `sidebar.rs` has no other tests yet, `app.rs` has no others yet).

#### 6.4 `sidebar.rs` widgets

Not unit-tested (egui widget layer, per contract) — but shown here in full, no placeholders. Add
below `scale_from_input` in `crates/gui/src/sidebar.rs`:
```rust
use gemelli_core::capture::DeviceInfo;
use gemelli_core::transform::Rotation;

/// Device combo box. Returns `true` if the selection changed this frame.
pub(crate) fn device_panel(ui: &mut egui::Ui, devices: &[DeviceInfo], selected: &mut usize) -> bool {
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

/// 2x2 segmented rotation selector, matching the UI仕様 ASCII layout: `(0)(90)` / `(180)(270)`.
/// Returns `true` if the selection changed this frame.
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
            ScaleInput::Off | ScaleInput::Factor(_) => ScaleInput::Exact { width: 960, height: 540 },
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
                value_edited |= ui.add(egui::DragValue::new(width).range(1..=7680).prefix("w:")).changed();
                value_edited |= ui.add(egui::DragValue::new(height).range(1..=4320).prefix("h:")).changed();
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
```

Note: `Mode` is a **local** enum scoped inside `scale_panel` (not exported) — it only exists to
drive the three radio buttons and has no meaning outside this function, so it stays private
rather than becoming a fourth public type next to `ScaleInput`.

- [ ] Run `cargo build -p gemelli-gui`. Expected: fails only on unresolved imports in `app.rs`
  (not yet written) or succeeds if `app.rs` already compiles standalone — either is fine before
  6.5.

#### 6.5 `app.rs`: `GemelliApp` struct + `new()`

Add to `crates/gui/src/app.rs` (below the `build_transform` block):
```rust
use std::sync::atomic::Ordering;
use std::sync::{mpsc, Arc, PoisonError};
use std::time::Instant;

use gemelli_core::capture::{self, DeviceInfo};
use gemelli_core::frame::Frame;
use gemelli_core::transform::{CropRect, Rotation, TransformConfig};

use crate::fps_meter::FpsMeter;
use crate::preview;
use crate::sidebar::ScaleInput;
use crate::theme;
use crate::worker::{spawn_worker, SharedState, WorkerError, WorkerHandle, WorkerSpec};

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
        let config = build_transform(self.crop, self.rotation, self.flip_h, self.flip_v, self.scale_input);
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
}
```
`requested_fps` stays `None` for the lifetime of the app — the Phase 2 UI仕様 ASCII layout has no
fps control, so this mirrors the CLI's own default ("none: highest resolution, then best fps at
that resolution") rather than inventing a widget the spec doesn't call for.

- [ ] Run `cargo build -p gemelli-gui`. Expected: compiles (the `eframe::App` impl and
  `main.rs`'s reference to `GemelliApp` are still missing/stale — this step only proves
  `app.rs` itself is well-formed; wire it up in 6.6/6.7 before expecting a full build).

#### 6.6 `app.rs`: `eframe::App::update()` wiring

Add to `crates/gui/src/app.rs`:
```rust
impl GemelliApp {
    fn drain_errors(&mut self) {
        // THE single consumption point for errors_rx (contract requirement) — worker.rs's
        // run_capture sends an error and then returns, ending its thread, so receiving one here
        // means the worker is no longer running even though nothing else told us so directly.
        while let Ok(error) = self.errors_rx.try_recv() {
            self.banner = Some(error.to_string());
            self.worker = None;
        }
    }

    fn refresh_preview(&mut self, ctx: &egui::Context) {
        let raw = self.shared.latest_raw.lock().unwrap_or_else(PoisonError::into_inner).clone();
        let output = self.shared.latest_output.lock().unwrap_or_else(PoisonError::into_inner).clone();

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
            None => self.texture = Some(ctx.load_texture("preview", image, egui::TextureOptions::LINEAR)),
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
            let device_changed = crate::sidebar::device_panel(ui, &self.devices, &mut self.selected_device);
            if crate::sidebar::refresh_button(ui) {
                self.reload_devices();
            }
            if device_changed && self.worker.is_some() {
                self.start_worker();
            }
        });

        ui.add_space(8.0);
        ui.heading("Rotate");
        if crate::sidebar::rotate_panel(ui, &mut self.rotation) {
            self.push_transform();
        }

        ui.add_space(8.0);
        ui.heading("Flip");
        if crate::sidebar::flip_panel(ui, &mut self.flip_h, &mut self.flip_v) {
            self.push_transform();
        }

        // Crop controls are inserted here by Task 7, between Flip and Scale (matches the
        // UI仕様 ASCII layout).

        ui.add_space(8.0);
        ui.heading("Scale");
        if crate::sidebar::scale_panel(ui, &mut self.scale_input) {
            self.push_transform();
        }

        ui.add_space(8.0);
        ui.heading("Server name");
        let server_name_committed = crate::sidebar::server_name_panel(ui, &mut self.server_name);
        if server_name_committed && self.worker.is_some() {
            self.start_worker();
        }

        ui.add_space(8.0);
        let running = self.worker.as_ref().is_some_and(WorkerHandle::is_running);
        if crate::sidebar::transport_button(ui, running) {
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
        ui.put(draw, egui::Image::new(texture));

        // Crop overlay + drag interaction are inserted here by Task 7 (only drawn/wired when
        // preview_mode == CropEdit).
    }
}

impl eframe::App for GemelliApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.drain_errors();
        self.refresh_preview(ctx);

        if let Some(message) = self.banner.clone() {
            egui::TopBottomPanel::top("banner").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.colored_label(theme::tokens::DANGER, &message);
                    if ui.button("Dismiss").clicked() {
                        self.banner = None;
                    }
                });
            });
        }

        egui::SidePanel::left("sidebar").resizable(false).min_width(220.0).show(ctx, |ui| {
            self.sidebar_ui(ui);
        });

        egui::TopBottomPanel::bottom("statusbar").show(ctx, |ui| {
            self.statusbar_ui(ui);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.preview_ui(ui);
        });

        // The capture thread pushes frames asynchronously (SharedState), not through egui's own
        // event loop, so nothing else would trigger a repaint once idle — request one every
        // frame to keep the preview and fps counter live.
        ctx.request_repaint();
    }
}
```

#### 6.7 `main.rs` bootstrap

Replace the placeholder content of `crates/gui/src/main.rs`:
```rust
//! GUI entry point: boots the eframe window and hands control to `GemelliApp`.

mod app;
mod crop_editor;
mod fps_meter;
mod preview;
mod sidebar;
mod theme;
mod worker;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([960.0, 640.0]),
        ..Default::default()
    };

    eframe::run_native("gemelli", options, Box::new(|cc| Ok(Box::new(app::GemelliApp::new(cc)))))
}
```
If Tasks 1–5 already declared some of these `mod` lines, merge rather than duplicate — the full
list above is the target end state.

#### 6.8 Lint gate + commit

- [ ] Run:
  ```bash
  cargo test -p gemelli-gui
  ```
  Expected: `test result: ok. 9 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` (same 9
  as the end of 6.3 — this task added no further `#[test]`s, only widget/wiring code).

- [ ] Run:
  ```bash
  cargo clippy --workspace --all-targets -- -D warnings
  cargo fmt --all -- --check
  ```
  Expected: both exit `0` with no output (clippy: no warnings; fmt: already formatted).

- [ ] `git add crates/gui/Cargo.toml crates/gui/src/app.rs crates/gui/src/sidebar.rs crates/gui/src/main.rs`
  and commit:
  ```
  feat(gui): wire app state, sidebar, and status bar

  Adds GemelliApp (device/rotate/flip/scale/server-name controls, Start/Stop,
  error banner) and the sidebar/statusbar widget layer, with the config-rebuild
  path (flip_from_toggles, ScaleInput, build_transform) TDD'd as pure functions.
  Crop editing lands in the next commit.
  ```
  No push.

#### 6.9 Manual verification

- [ ] `cargo run -p gemelli-gui` opens a window titled "gemelli" with the sidebar on the left,
  preview pane on the right, and status bar at the bottom.
- [ ] The device combo lists at least one entry per attached camera (cross-check against
  `cargo run -p gemelli-cli -- --list-devices`).
- [ ] Clicking a rotate segment, toggling h/v flip, and moving the scale slider/fields all
  respond visibly (button highlight / slider thumb moves) even with no camera running yet.
- [ ] Selecting a device and clicking **Start**: the preview pane shows the live camera feed
  within ~1 second, the status bar shows real input/output dimensions and a non-zero fps, and the
  indicator reads "● publishing".
- [ ] Open a Syphon client (Syphon Recorder or Simple Client) and confirm a server named
  `gemelli` appears and shows the same image as the GUI's preview pane.
- [ ] Adjusting rotate/flip/scale while running updates both the GUI preview and the Syphon
  client's image live.
- [ ] Clicking **Stop** halts the preview, the indicator reads "○ stopped", and the Syphon server
  disappears from the client.
- [ ] Quit the window (close button): the process exits without hanging.

---

### Task 7: Crop-edit interaction

Adds the crop editing UI the contract's UI仕様 calls for: an "Edit crop" / "Done" toggle, a
draggable rectangle overlay on the raw preview with corner-resize handles, and a numeric W/H/X/Y
row synced both ways with the drag. `crop = None` is handled with an "Add crop" button that seeds
a centered rect at half the frame size (`seed_rect`, new — not part of the original contract,
added here to close the gap the contract explicitly leaves open: *"'Add crop' button seeds a
centered rect at half frame size"*).

**Files:**
- `crates/gui/src/crop_editor.rs` (append `seed_rect` + tests — everything else in this file is
  Task 5's and is only *called*, never modified)
- `crates/gui/src/sidebar.rs` (append `CropAction` + `crop_panel`)
- `crates/gui/src/app.rs` (extend `sidebar_ui` and `preview_ui` at the two marked insertion
  points from Task 6)

**Interfaces (consumed, exact signatures from Task 5 per `GUI-CONTRACT.md`):**
```rust
// crate::crop_editor (Task 5 — unmodified by this task, only called)
pub struct CropMapping { pub frame_width: u32, pub frame_height: u32, pub draw: egui::Rect }
impl CropMapping {
    pub fn to_screen(&self, rect: CropRect) -> egui::Rect;
    pub fn to_frame(&self, rect: egui::Rect) -> CropRect;
}
pub fn clamp_rect(rect: CropRect, frame_width: u32, frame_height: u32) -> CropRect;
pub enum DragMode { Move, ResizeNw, ResizeNe, ResizeSw, ResizeSe }
pub struct DragState { pub mode: DragMode, pub start_rect: CropRect, pub start_pointer: egui::Pos2 }
pub fn apply_drag(state: &DragState, mapping: &CropMapping, pointer: egui::Pos2) -> CropRect;
pub fn hit_test(rect_screen: egui::Rect, pointer: egui::Pos2) -> Option<DragMode>;

// crate::theme::tokens (Task 5)
pub const CROP_OVERLAY: egui::Color32;
```

**Produces:**
```rust
// crate::crop_editor (this task's addition)
pub fn seed_rect(frame_width: u32, frame_height: u32) -> CropRect;

// crate::sidebar (this task's addition)
pub(crate) enum CropAction { None, ToggleEdit, Add, Clear, Edited(CropRect) }
pub(crate) fn crop_panel(ui: &mut egui::Ui, crop: Option<CropRect>, editing: bool) -> CropAction;
```

---

#### 7.1 Pure fn: `seed_rect` (TDD)

**RED** — append to `crates/gui/src/crop_editor.rs`'s existing `#[cfg(test)] mod tests` block (add
the import and the three test functions; do not touch Task 5's existing tests):
```rust
#[cfg(test)]
mod tests {
    // ... Task 5's existing imports/tests stay above ...
    use super::seed_rect;

    #[test]
    fn seed_rect_centers_a_half_size_rect_in_a_1080p_frame() {
        let rect = seed_rect(1920, 1080);

        assert_eq!(rect, CropRect { width: 960, height: 540, x: 480, y: 270 });
    }

    #[test]
    fn seed_rect_centers_a_half_size_rect_in_a_480p_frame() {
        let rect = seed_rect(640, 480);

        assert_eq!(rect, CropRect { width: 320, height: 240, x: 160, y: 120 });
    }

    #[test]
    fn seed_rect_handles_odd_dimensions_via_integer_division() {
        let rect = seed_rect(101, 101);

        assert_eq!(rect, CropRect { width: 50, height: 50, x: 25, y: 25 });
        // Sanity: still fully inside the frame.
        assert!(rect.x + rect.width <= 101);
        assert!(rect.y + rect.height <= 101);
    }
}
```
- [ ] Run:
  ```bash
  cargo test -p gemelli-gui seed_rect
  ```
  Expected: compile error — `cannot find function `seed_rect` in this scope`. RED confirmed.

**GREEN** — add above the test module, next to Task 5's other public functions:
```rust
/// Seeds a centered crop rect at half the frame's width/height — used when the sidebar's "Add
/// crop" button is clicked with `crop == None`. Delegates the min-size/bounds invariant to
/// `clamp_rect` (Task 5) instead of re-deriving it here, since a half-frame rect for any frame
/// at or above typical webcam resolutions already satisfies it and degenerate tiny frames are
/// exactly what `clamp_rect` exists to handle.
pub fn seed_rect(frame_width: u32, frame_height: u32) -> CropRect {
    let half_width = frame_width / 2;
    let half_height = frame_height / 2;
    let x = (frame_width - half_width) / 2;
    let y = (frame_height - half_height) / 2;
    let seeded = CropRect { width: half_width, height: half_height, x, y };
    clamp_rect(seeded, frame_width, frame_height)
}
```
- [ ] Run:
  ```bash
  cargo test -p gemelli-gui seed_rect
  ```
  Expected:
  ```
  running 3 tests
  test crop_editor::tests::seed_rect_centers_a_half_size_rect_in_a_1080p_frame ... ok
  test crop_editor::tests::seed_rect_centers_a_half_size_rect_in_a_480p_frame ... ok
  test crop_editor::tests::seed_rect_handles_odd_dimensions_via_integer_division ... ok

  test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
  ```

- [ ] Run `cargo test -p gemelli-gui crop_editor::` and confirm none of Task 5's existing
  `crop_editor` tests regressed (their count is whatever Task 5's own section documented — this
  step only proves `seed_rect` didn't touch them, not the exact number).

#### 7.2 `sidebar.rs`: `CropAction` + `crop_panel`

Not unit-tested (widget layer). Append to `crates/gui/src/sidebar.rs`:
```rust
use gemelli_core::transform::CropRect;

/// What the crop panel's buttons/fields did this frame. Exhaustively matched by `app.rs` — no
/// `_` arm, so a new action here forces the call site to decide what it means instead of
/// silently doing nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CropAction {
    None,
    ToggleEdit,
    Add,
    Clear,
    Edited(CropRect),
}

/// Crop controls: Edit/Done toggle, Add/Clear crop, and (when a crop exists) a W/H/X/Y numeric
/// row. The numeric fields and the on-screen drag rect (Task 7's `preview_ui` addition) are kept
/// in sync purely by both reading `self.crop` fresh every frame in `app.rs` — there is no
/// separate "pending edit" state to desync.
pub(crate) fn crop_panel(ui: &mut egui::Ui, crop: Option<CropRect>, editing: bool) -> CropAction {
    let mut action = CropAction::None;

    ui.horizontal(|ui| {
        let edit_label = if editing { "Done" } else { "Edit crop" };
        if ui.button(edit_label).clicked() {
            action = CropAction::ToggleEdit;
        }
        match crop {
            Some(_) => {
                if ui.button("Clear crop").clicked() {
                    action = CropAction::Clear;
                }
            }
            None => {
                if ui.button("Add crop").clicked() {
                    action = CropAction::Add;
                }
            }
        }
    });

    let Some(mut rect) = crop else {
        return action;
    };

    let mut edited = false;
    ui.horizontal(|ui| {
        edited |= ui.add(egui::DragValue::new(&mut rect.width).prefix("w:")).changed();
        edited |= ui.add(egui::DragValue::new(&mut rect.height).prefix("h:")).changed();
    });
    ui.horizontal(|ui| {
        edited |= ui.add(egui::DragValue::new(&mut rect.x).prefix("x:")).changed();
        edited |= ui.add(egui::DragValue::new(&mut rect.y).prefix("y:")).changed();
    });
    if edited {
        action = CropAction::Edited(rect);
    }

    action
}
```

#### 7.3 `app.rs`: `sidebar_ui` dispatches `CropAction`

Replace the "Crop controls are inserted here by Task 7" comment left in Task 6's `sidebar_ui`
(6.6) with:
```rust
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
                    self.banner = Some("no frame yet — start capture before adding a crop".to_string());
                }
            },
            crate::sidebar::CropAction::Clear => {
                self.crop = None;
                self.drag = None;
                self.push_transform();
            }
            crate::sidebar::CropAction::Edited(rect) => {
                let clamped = match self.input_dims {
                    Some((frame_w, frame_h)) => crate::crop_editor::clamp_rect(rect, frame_w, frame_h),
                    None => rect,
                };
                self.crop = Some(clamped);
                self.push_transform();
            }
        }
```
This block goes between the "Flip" section and the "Scale" section, matching the UI仕様 ASCII
layout order (`Flip [h][v]` → `Crop [Edit]` / `W H X Y` → `Scale …`).

The `CropAction::Clear` arm references `self.drag`, which does not exist on `GemelliApp` yet — add
it now. Extend the struct from 6.5:
```rust
pub struct GemelliApp {
    // ... all fields from 6.5 unchanged ...
    drag: Option<crate::crop_editor::DragState>, // add this field
}
```
and `GemelliApp::new`'s struct literal:
```rust
        Self {
            // ... all fields from 6.5 unchanged ...
            drag: None, // add this field
        }
```

#### 7.4 `app.rs`: `preview_ui` — overlay + drag interaction

Replace the "Crop overlay + drag interaction are inserted here by Task 7" comment left in Task
6's `preview_ui` (6.6) with:
```rust
        if self.preview_mode == PreviewMode::CropEdit {
            if let Some(rect) = self.crop {
                let mapping =
                    crate::crop_editor::CropMapping { frame_width: frame_w, frame_height: frame_h, draw };
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

                let response = ui.interact(draw, ui.id().with("crop_overlay"), egui::Sense::click_and_drag());

                if response.drag_started() {
                    if let Some(pointer) = response.interact_pointer_pos() {
                        if let Some(mode) = crate::crop_editor::hit_test(rect_screen, pointer) {
                            self.drag = Some(crate::crop_editor::DragState {
                                mode,
                                start_rect: rect,
                                start_pointer: pointer,
                            });
                        }
                    }
                }

                if response.dragged() {
                    if let (Some(drag), Some(pointer)) = (&self.drag, response.interact_pointer_pos()) {
                        let updated = crate::crop_editor::apply_drag(drag, &mapping, pointer);
                        self.crop = Some(updated);
                        self.push_transform();
                    }
                }

                if response.drag_stopped() {
                    self.drag = None;
                }
            }
        }
```
Full `preview_ui` after this edit (shown complete, replacing 6.6's version — no placeholders):
```rust
    fn preview_ui(&mut self, ui: &mut egui::Ui) {
        let avail = ui.available_rect_before_wrap();
        let (Some(texture), Some((frame_w, frame_h))) = (&self.texture, self.preview_dims) else {
            ui.centered_and_justified(|ui| {
                ui.label("No preview — start capture to see the feed");
            });
            return;
        };
        let draw = preview::fit_rect(frame_w, frame_h, avail);
        ui.put(draw, egui::Image::new(texture));

        if self.preview_mode == PreviewMode::CropEdit {
            if let Some(rect) = self.crop {
                let mapping =
                    crate::crop_editor::CropMapping { frame_width: frame_w, frame_height: frame_h, draw };
                let rect_screen = mapping.to_screen(rect);

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

                let response = ui.interact(draw, ui.id().with("crop_overlay"), egui::Sense::click_and_drag());

                if response.drag_started() {
                    if let Some(pointer) = response.interact_pointer_pos() {
                        if let Some(mode) = crate::crop_editor::hit_test(rect_screen, pointer) {
                            self.drag = Some(crate::crop_editor::DragState {
                                mode,
                                start_rect: rect,
                                start_pointer: pointer,
                            });
                        }
                    }
                }

                if response.dragged() {
                    if let (Some(drag), Some(pointer)) = (&self.drag, response.interact_pointer_pos()) {
                        let updated = crate::crop_editor::apply_drag(drag, &mapping, pointer);
                        self.crop = Some(updated);
                        self.push_transform();
                    }
                }

                if response.drag_stopped() {
                    self.drag = None;
                }
            }
        }
    }
```

Note on `apply_drag`'s contract: it already returns a clamped `CropRect` ("produce the new
(clamped) CropRect" per `GUI-CONTRACT.md`), so this call site does not re-clamp — doing so would
duplicate Task 5's own invariant enforcement, which is exactly what the "reuse, don't add"
instruction rules out.

#### 7.5 Lint gate + commit

- [ ] Run:
  ```bash
  cargo test -p gemelli-gui
  ```
  Expected: `test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` (9 from
  Task 6 + 3 new `seed_rect` tests from 7.1).

- [ ] Run:
  ```bash
  cargo clippy --workspace --all-targets -- -D warnings
  cargo fmt --all -- --check
  ```
  Expected: both exit `0` with no output.

- [ ] `git add crates/gui/src/crop_editor.rs crates/gui/src/sidebar.rs crates/gui/src/app.rs` and
  commit:
  ```
  feat(gui): add crop-edit interaction

  Adds the Edit-crop/Done toggle, a draggable+resizable crop overlay on the raw
  preview, and a numeric W/H/X/Y row kept in sync by reading crop state fresh
  each frame. seed_rect gives "Add crop" a sensible centered starting rect.
  ```
  No push.

#### 7.6 Manual verification

- [ ] With the worker running, click **Add crop** in the sidebar: a white-on-black bordered
  rectangle appears centered on the preview at roughly half its size, and the W/H/X/Y fields show
  matching numbers.
- [ ] Click **Edit crop**: the button label changes to **Done**, and the preview switches to the
  raw (pre-transform) feed with the overlay still shown.
- [ ] Drag the rectangle's interior: it moves with the pointer, stays clamped inside the frame,
  and the W/H/X/Y fields update live as you drag.
- [ ] Drag a corner handle: the rectangle resizes from that corner, respecting a visible minimum
  size, and the numeric fields track it.
- [ ] Edit the W field directly (type a new value): the on-screen rectangle jumps to match.
- [ ] With the worker running and a crop set, confirm the Syphon client's image is cropped to
  match the on-screen rectangle, updating live as you drag.
- [ ] Click **Done**: the preview switches back to the transformed (Output) view, still showing
  the crop's effect (now composed with rotate/flip/scale).
- [ ] Click **Clear crop**: the rectangle disappears, the button reverts to **Add crop**, and the
  Syphon output returns to the full (uncropped) frame.

---

### Task 8: E2E verification and docs close-out

**Files:**
- `README.md` (append a "GUI" section)
- No source changes — this task is verification + documentation only.

**Interfaces:** none (consumes nothing new; this task only runs and documents what Tasks 1–7
built).

#### 8.1 Full workspace gate

- [ ] Run each command and record the actual numbers where marked — do not guess a total; if the
  printed count doesn't match the sum of every task's own documented GREEN checkpoint (Task 6:
  9, Task 7: +3 ⇒ 12 for `gemelli-gui`, plus whatever Tasks 1–5's sections documented for
  `theme`/`preview`/`fps_meter`/`worker`/`crop_editor`, plus Phase 1's unchanged `core`/`cli`
  counts from `2026-07-07-core-cli-implementation.md`), something regressed and must be
  investigated before continuing:
  ```bash
  cargo build --workspace
  ```
  Expected: `Finished` with no errors, no warnings.
  ```bash
  cargo test --workspace
  ```
  Expected: one `test result: ok. N passed; 0 failed; 0 ignored; 0 measured; 0 filtered out` line
  per crate (`gemelli-core`, `gemelli-cli`, `gemelli-syphon` if it has tests, `gemelli-gui`) — the
  `gemelli-gui` line's `N` must be ≥12 (this task's own two commits contribute exactly 12; Tasks
  1–5 add more on top).
  ```bash
  cargo clippy --workspace --all-targets -- -D warnings
  ```
  Expected: exits `0`, no output.
  ```bash
  cargo fmt --all -- --check
  ```
  Expected: exits `0`, no output.

#### 8.2 Manual E2E checklist (real camera + Syphon Recorder)

- [ ] Launch `cargo run -p gemelli-gui`, select a real attached camera, click **Start**. Open
  Syphon Recorder and confirm the server named per the sidebar's "Server name" field appears and
  its image matches the GUI's own preview pane pixel-for-pixel (same content, same orientation).
- [ ] While publishing, change rotate, flip, scale, and crop (via drag and via the numeric
  fields) one at a time and confirm each change appears in **both** the GUI preview and the
  Syphon Recorder image at the same time, with no visible lag or tearing beyond normal frame
  latency.
- [ ] While publishing, switch to a second attached camera in the device combo (if more than one
  is available — otherwise skip and note it as untested in the PR description) and confirm the
  Syphon Recorder image switches to the new camera's feed within ~1 second, without the server
  disappearing from the client's server list.
- [ ] While publishing, edit the server name field and click away (or press Tab) to commit it:
  confirm the old-named server disappears from Syphon Recorder's list and a new one under the new
  name appears, still showing the live feed.
- [ ] While publishing, physically unplug the active camera (or otherwise force it offline):
  confirm the GUI shows an error banner within a few seconds, the status bar switches to
  "○ stopped", and the process does **not** panic or hang.
- [ ] With the banner still showing, plug the camera back in, click **Refresh** to repopulate the
  device list, select the camera, and click **Start**: confirm publishing resumes and the Syphon
  client sees the server reappear.
- [ ] Close the GUI window: confirm the process exits promptly (no hang) and the Syphon server
  disappears from the client's list (clean publisher drop, matching the CLI's own documented
  Ctrl+C behavior).

#### 8.3 README: add a "GUI" section

- [ ] Insert a new section into `README.md` immediately after the existing "## CLI usage" section
  (before "## Manual verification checklist"):
  ```markdown
  ## GUI usage

  ```bash
  cargo run -p gemelli-gui
  ```

  A sidebar/preview layout (see `docs/superpowers/specs/2026-07-08-gemelli-gui-design.md` for the
  full design) for adjusting transforms live while previewing the camera feed, instead of
  re-launching the CLI with new flags each time.

  | Control | Effect |
  |---|---|
  | Device combo + Refresh | Selects the capture device; Refresh re-queries attached cameras |
  | Rotate (0/90/180/270) | Clockwise rotation, applied after crop |
  | Flip (h / v) | Independent horizontal/vertical mirror toggles; both = `hv` |
  | Crop: Edit crop / Done | Switches the preview to the raw feed with a draggable crop rect |
  | Crop: Add crop / Clear crop | Seeds a centered half-frame crop, or removes the crop entirely |
  | Crop: W / H / X / Y fields | Numeric crop editing, synced live with the on-screen drag |
  | Scale: Off / Factor / WxH | Off (no resize), a 0.1–2.0x slider, or exact target dimensions |
  | Server name | Syphon server name; committing a change restarts the server under the new name |
  | Start / Stop | Begins/stops capture and Syphon publishing on the selected device |
  | Status bar | Input→output resolution, measured fps, and a publishing/stopped indicator |

  Transform order is the same as the CLI: **crop → rotate → flip → scale**. The GUI is an
  additional front end, not a replacement — `gemelli-cli` remains the headless path (e.g. for
  scripted/unattended launches), and both share the same `gemelli-core` transform and Syphon
  publish pipeline.
  ```

- [ ] Run `cargo fmt --all -- --check` (README changes don't affect this, but re-run as a sanity
  check that no code was touched) and visually diff the README section against the table above
  for typos.

#### 8.4 Final commit

- [ ] `git add README.md` and commit:
  ```
  docs: document gemelli-gui usage and controls

  Closes out Phase 2: adds a GUI section to the README (launch command,
  control reference table, transform-order note) alongside the manual E2E
  checklist run against a real camera and Syphon Recorder.
  ```
  No push.
