# web-cam-sharedtexture Phase 1 (core + CLI) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** webcam の映像を crop → rotate → flip → scale 変換して Syphon サーバーとして publish する CLI ツール(macOS)を完成させる。

**Architecture:** cargo workspace の 3 crate 構成。`webcam-sharedtexture-core`(BGRA8 `Frame` + 変換関数 + `CaptureSource`/`TexturePublisher` トレイト + `run_pipeline`)、`webcam-sharedtexture-syphon`(ObjC++ シム + `cc` crate で Syphon.framework をブリッジする `SyphonPublisher`)、`webcam-sharedtexture-cli`(clap parse → run() → main で一度だけ match)。GUI(egui)は Phase 2 の別 plan。

**Tech Stack:** Rust edition 2024 / thiserror 2.x / nokhwa 0.10.11 (input-native) / clap 4.6 (derive) / ctrlc 3.5 / cc 1.x + ObjC++ (Syphon.framework は git submodule + xcodebuild)

**Spec:** `docs/superpowers/specs/2026-07-07-webcam-sharedtexture-design.md`

## Global Constraints

- Rust edition 2024。clippy は workspace lints で `unwrap_used` / `expect_used` / `as_conversions` を **deny**(テストは Task 1 で作る `clippy.toml` により unwrap/expect 免除)
- `as` キャストの例外は 2 箇所のみ: `transform/scale.rs` の `scale_dimension` ヘルパー(文書化済み `#[allow]`)と FFI 境界(必要な場合のみ、SAFETY コメント付き)
- 全コードは `.claude/skills/` の 7 skills に従う: enum の exhaustive `match`(`_` arm 禁止)、`?`/コンビネータ(テスト外 unwrap/expect 禁止)、early-return guards、関数名 ≤3 語、明示的型変換
- t-wada TDD: 失敗するテストを先に書き、FAIL を確認してから最小実装で PASS させる。テストの画素検証は非対称画像(2×3px、全画素一意)で全画素を厳密検証
- 各タスク末尾で lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`
- commit は conventional commits(`feat(core): ...` 等)で task 単位。**push はしない**(ユーザー承認済み: task commit は許可)
- ピクセル形式は全域 BGRA8 tightly packed(stride = width×4)、`idx = (y*width + x)*4`
- 対象 OS: macOS(Syphon)。`crates/syphon` は macOS 専用、CLI は他 OS で `UnsupportedPlatform` エラー
- crate 名: `webcam-sharedtexture-core` / `webcam-sharedtexture-syphon` / `webcam-sharedtexture-cli`(bin: `webcam-sharedtexture`)

### Interface-contract deviations (approved during plan writing)

- `TransformError` は `Eq` を derive しない(`ScaleFactorInvalid.factor: f64` のため)。エラー検証は `matches!` で行う
- `CliError` に `UnsupportedPlatform` と `CtrlcSetup(#[from] ctrlc::Error)` を追加
- Syphon bridge の C ABI は契約の opaque-struct + `bool` 形状を採用(参照実装の `void*` + `int` から意図的に変更)。edition 2024 のため FFI 宣言は `unsafe extern "C"`

---
## Section A — Tasks 1–6 (crates/core: Frame + Transform pipeline)

All paths below are repo-root relative (`/Users/napochaan/ghq/github.com/naporin0624/web-cam-sharedtexture`).
Run every command from the repo root. Test command form: `cargo test -p webcam-sharedtexture-core --lib <module::path>`.
Lint gate (run after every green step, before every commit):
`cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`

**BGRA8 layout (read once, applies to every task below):** `Frame::data()` is a
tightly-packed, row-major byte buffer: `width * height * 4` bytes, no row
padding, 4 bytes per pixel in **B, G, R, A** order. Pixel `(x, y)` (x = column,
y = row, both 0-indexed) starts at byte offset `(y * width + x) * 4`. E.g. for
a 2-wide image, pixel `(1, 0)` starts at byte 4, and pixel `(0, 1)` starts at
byte 8.

**Contract correction (flag prominently):** the interface contract's
`TransformError` derives `#[derive(Debug, PartialEq, Eq, thiserror::Error)]`,
but one variant (`ScaleFactorInvalid { factor: f64 }`) holds an `f64`, and
`f64` does not implement `Eq` (`NaN != NaN`). Deriving `Eq` on a type
containing an `f64` field is a compile error. This plan derives
`#[derive(Debug, PartialEq, thiserror::Error)]` for `TransformError` (drops
`Eq`, keeps `PartialEq`) — same fix already correctly applied to `ScaleSpec`
in the contract. All other types/derives below are copied verbatim from the
contract.

**Internal addition beyond the contract:** `Frame` gets one extra
`pub(crate)` constructor, `from_validated`, used only by transform functions
(crop/rotate/flip/scale) that already know their output length is exactly
`width * height * 4` and shouldn't pay for/handle a redundant `Result` from
`Frame::new`. It does not change any public signature in the contract.

---

### Task 1: Frame type

**Files:**
- Modify: `Cargo.toml` (root) — add `[workspace.dependencies]`
- Create: `clippy.toml` (root)
- Modify: `crates/core/Cargo.toml` — add `thiserror` dependency
- Modify: `crates/core/src/lib.rs` — replace placeholder with module declarations
- Modify: `crates/cli/src/main.rs`, `crates/gui/src/main.rs` — stop calling the
  removed `core::crate_name()` placeholder (otherwise `cargo clippy --workspace
  --all-targets` fails to compile once it's removed)
- Create: `crates/core/src/frame.rs`
- Test: `crates/core/src/frame.rs` (`#[cfg(test)] mod tests`)

**Interfaces:**
- Consumes: none (first task)
- Produces:
  ```rust
  pub struct Frame { /* private: width: u32, height: u32, data: Vec<u8> */ }

  #[derive(Debug, PartialEq, Eq, thiserror::Error)]
  pub enum FrameError {
      #[error("frame data length {actual} does not match {width}x{height}x4 = {expected}")]
      DataLengthMismatch { width: u32, height: u32, expected: usize, actual: usize },
      #[error("frame dimensions must be non-zero (got {width}x{height})")]
      ZeroDimension { width: u32, height: u32 },
  }

  impl Frame {
      pub fn new(width: u32, height: u32, data: Vec<u8>) -> Result<Self, FrameError>;
      pub fn width(&self) -> u32;
      pub fn height(&self) -> u32;
      pub fn data(&self) -> &[u8];
      pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]>;
      pub(crate) fn from_validated(width: u32, height: u32, data: Vec<u8>) -> Self; // internal, not in the public contract
  }
  ```

#### Step 0 — workspace scaffolding (not a TDD cycle)

- [ ] Edit `Cargo.toml` (root) to add a `[workspace.dependencies]` table right after `[workspace.package]`:
  ```toml
  [workspace]
  resolver = "2"
  members = ["crates/core", "crates/cli", "crates/gui"]

  [workspace.package]
  edition = "2024"
  license = "MIT"
  repository = "https://github.com/naporin0624/web-cam-sharedtexture"

  [workspace.dependencies]
  thiserror = "2"

  [workspace.lints.clippy]
  unwrap_used = "deny"
  expect_used = "deny"
  as_conversions = "deny"
  ```
- [ ] Create `clippy.toml` (root) with exactly:
  ```toml
  allow-unwrap-in-tests = true
  allow-expect-in-tests = true
  ```
- [ ] Edit `crates/core/Cargo.toml` to add the dependency:
  ```toml
  [package]
  name = "webcam-sharedtexture-core"
  version = "0.1.0"
  edition.workspace = true
  license.workspace = true
  repository.workspace = true

  [lints]
  workspace = true

  [dependencies]
  thiserror = { workspace = true }
  ```
- [ ] Replace `crates/core/src/lib.rs` entirely with:
  ```rust
  //! Core library for the webcam -> Spout/Syphon sharing tool.

  pub mod frame;
  ```
- [ ] Create an empty `crates/core/src/frame.rs` (zero bytes) so `pub mod frame;` compiles.
- [ ] Replace `crates/cli/src/main.rs` with:
  ```rust
  //! CLI entry point placeholder for the webcam -> Spout/Syphon sharing tool.

  fn main() {
      println!("webcam-sharedtexture-cli: not yet implemented");
  }
  ```
- [ ] Replace `crates/gui/src/main.rs` with:
  ```rust
  //! GUI entry point placeholder for the webcam -> Spout/Syphon sharing tool.

  fn main() {
      println!("webcam-sharedtexture-gui: not yet implemented");
  }
  ```
- [ ] Run `cargo build --workspace` — expect success (no tests yet, just confirming the workspace still links).
- [ ] `git add Cargo.toml clippy.toml crates/core/Cargo.toml crates/core/src/lib.rs crates/core/src/frame.rs crates/cli/src/main.rs crates/gui/src/main.rs`
  commit:
  ```
  chore(workspace): wire up frame module and thiserror dependency
  ```

#### Cycle 1 — `Frame::new` + accessors + `FrameError`

- [ ] Write the failing test. Set `crates/core/src/frame.rs` to just:
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;

      fn sample_frame() -> Frame {
          let data = vec![
              10, 20, 30, 255, // (0,0)
              11, 21, 31, 255, // (1,0)
              12, 22, 32, 255, // (0,1)
              13, 23, 33, 255, // (1,1)
              14, 24, 34, 255, // (0,2)
              15, 25, 35, 255, // (1,2)
          ];
          Frame::new(2, 3, data).unwrap()
      }

      #[test]
      fn new_accepts_matching_length() {
          let frame = sample_frame();
          assert_eq!(frame.width(), 2);
          assert_eq!(frame.height(), 3);
          assert_eq!(frame.data().len(), 24);
      }

      #[test]
      fn new_rejects_length_mismatch() {
          let result = Frame::new(2, 3, vec![0; 10]);
          assert_eq!(
              result,
              Err(FrameError::DataLengthMismatch { width: 2, height: 3, expected: 24, actual: 10 })
          );
      }

      #[test]
      fn new_rejects_zero_dimension() {
          let result = Frame::new(0, 3, vec![]);
          assert_eq!(result, Err(FrameError::ZeroDimension { width: 0, height: 3 }));
      }
  }
  ```
- [ ] Run `cargo test -p webcam-sharedtexture-core --lib frame::tests`.
  Expect **compile failure**: `Frame` and `FrameError` don't exist yet, e.g.
  `error[E0433]: failed to resolve: use of undeclared type `Frame`` (repeated
  for each use site; exact wording depends on rustc version, but it must be a
  compile error, not a test-assertion failure).
- [ ] Minimal implementation. Prepend to `crates/core/src/frame.rs` (above the test module):
  ```rust
  //! Frame type: BGRA8 tightly-packed pixel buffer.
  //!
  //! Layout: `data` is `width * height * 4` bytes, row-major, BGRA8 per pixel
  //! (no padding — stride is always `width * 4`). Pixel (x, y) starts at byte
  //! offset `(y * width + x) * 4`.

  #[derive(Debug, Clone, PartialEq, Eq)]
  pub struct Frame {
      width: u32,
      height: u32,
      data: Vec<u8>,
  }

  #[derive(Debug, PartialEq, Eq, thiserror::Error)]
  pub enum FrameError {
      #[error("frame data length {actual} does not match {width}x{height}x4 = {expected}")]
      DataLengthMismatch { width: u32, height: u32, expected: usize, actual: usize },
      #[error("frame dimensions must be non-zero (got {width}x{height})")]
      ZeroDimension { width: u32, height: u32 },
  }

  impl Frame {
      pub fn new(width: u32, height: u32, data: Vec<u8>) -> Result<Self, FrameError> {
          if width == 0 || height == 0 {
              return Err(FrameError::ZeroDimension { width, height });
          }
          let width_len = usize::try_from(width).unwrap_or(usize::MAX);
          let height_len = usize::try_from(height).unwrap_or(usize::MAX);
          let expected = width_len
              .checked_mul(height_len)
              .and_then(|pixels| pixels.checked_mul(4))
              .unwrap_or(usize::MAX);
          if data.len() != expected {
              return Err(FrameError::DataLengthMismatch {
                  width,
                  height,
                  expected,
                  actual: data.len(),
              });
          }
          Ok(Self { width, height, data })
      }

      pub fn width(&self) -> u32 {
          self.width
      }

      pub fn height(&self) -> u32 {
          self.height
      }

      pub fn data(&self) -> &[u8] {
          &self.data
      }
  }
  ```
- [ ] Run `cargo test -p webcam-sharedtexture-core --lib frame::tests`. Expect
  `test result: ok. 3 passed; 0 failed;`.
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`.
- [ ] `git add crates/core/src/frame.rs`, commit:
  ```
  feat(core): add Frame with BGRA8 length validation
  ```

#### Cycle 2 — `Frame::pixel`

- [ ] Write the failing test. Add to `frame.rs`'s `mod tests`:
  ```rust
      #[test]
      fn pixel_returns_bgra_bytes_at_position() {
          let frame = sample_frame();
          assert_eq!(frame.pixel(0, 0), Some([10, 20, 30, 255]));
          assert_eq!(frame.pixel(1, 0), Some([11, 21, 31, 255]));
          assert_eq!(frame.pixel(0, 1), Some([12, 22, 32, 255]));
          assert_eq!(frame.pixel(1, 1), Some([13, 23, 33, 255]));
          assert_eq!(frame.pixel(0, 2), Some([14, 24, 34, 255]));
          assert_eq!(frame.pixel(1, 2), Some([15, 25, 35, 255]));
      }

      #[test]
      fn pixel_returns_none_out_of_bounds() {
          let frame = sample_frame();
          assert_eq!(frame.pixel(2, 0), None);
          assert_eq!(frame.pixel(0, 3), None);
      }
  ```
- [ ] Run `cargo test -p webcam-sharedtexture-core --lib frame::tests`. Expect
  **compile failure**: `error[E0599]: no method named `pixel` found for struct `Frame` in the current scope`.
- [ ] Minimal implementation. Add to `impl Frame` in `frame.rs`:
  ```rust
      pub fn pixel(&self, x: u32, y: u32) -> Option<[u8; 4]> {
          if x >= self.width || y >= self.height {
              return None;
          }
          let width_len = usize::try_from(self.width).unwrap_or(usize::MAX);
          let x_len = usize::try_from(x).unwrap_or(usize::MAX);
          let y_len = usize::try_from(y).unwrap_or(usize::MAX);
          let idx = y_len.checked_mul(width_len)?.checked_add(x_len)?.checked_mul(4)?;
          let end = idx.checked_add(4)?;
          let bytes = self.data.get(idx..end)?;
          Some([bytes[0], bytes[1], bytes[2], bytes[3]])
      }
  ```
- [ ] Run `cargo test -p webcam-sharedtexture-core --lib frame::tests`. Expect
  `test result: ok. 5 passed; 0 failed;`.
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`.
- [ ] `git add crates/core/src/frame.rs`, commit:
  ```
  feat(core): add Frame::pixel BGRA8 accessor
  ```

#### Cycle 3 — `Frame::from_validated` (internal constructor for transforms)

- [ ] Write the failing test. Add to `frame.rs`'s `mod tests`:
  ```rust
      #[test]
      fn from_validated_skips_length_check() {
          let frame = Frame::from_validated(2, 1, vec![1, 2, 3, 4, 5, 6, 7, 8]);
          assert_eq!(frame.width(), 2);
          assert_eq!(frame.height(), 1);
          assert_eq!(frame.pixel(1, 0), Some([5, 6, 7, 8]));
      }
  ```
- [ ] Run `cargo test -p webcam-sharedtexture-core --lib frame::tests`. Expect
  **compile failure**: `error[E0599]: no function or associated item named `from_validated` found for struct `Frame``.
- [ ] Minimal implementation. Add to `impl Frame` in `frame.rs`:
  ```rust
      /// Builds a `Frame` without re-validating length/non-zero invariants.
      /// Callers (transform functions) already derive `width`/`height` from a
      /// source `Frame` and push exactly `width * height * 4` bytes, so the
      /// `new` round-trip would only re-check what the caller already proved.
      pub(crate) fn from_validated(width: u32, height: u32, data: Vec<u8>) -> Self {
          Self { width, height, data }
      }
  ```
- [ ] Run `cargo test -p webcam-sharedtexture-core --lib frame::tests`. Expect
  `test result: ok. 6 passed; 0 failed;`.
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`.
- [ ] `git add crates/core/src/frame.rs`, commit:
  ```
  feat(core): add Frame::from_validated for transform-internal construction
  ```

---

### Task 2: crop

**Files:**
- Create: `crates/core/src/transform/config.rs`
- Create: `crates/core/src/transform/mod.rs`
- Create: `crates/core/src/transform/crop.rs`
- Modify: `crates/core/src/lib.rs` — add `pub mod transform;`
- Test: `crates/core/src/transform/config.rs`, `crates/core/src/transform/crop.rs`

**Interfaces:**
- Consumes: `Frame::pixel`, `Frame::width`, `Frame::height`, `Frame::from_validated` (Task 1)
- Produces:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
  pub enum Rotation { #[default] R0, R90, R180, R270 }

  #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
  pub enum Flip { #[default] Keep, Horizontal, Vertical, Both }

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub struct CropRect { pub width: u32, pub height: u32, pub x: u32, pub y: u32 }

  #[derive(Debug, Clone, Copy, PartialEq)]
  pub enum ScaleSpec { Exact { width: u32, height: u32 }, Factor(f64) }

  #[derive(Debug, Clone, PartialEq, Default)]
  pub struct TransformConfig {
      pub crop: Option<CropRect>,
      pub rotation: Rotation,
      pub flip: Flip,
      pub scale: Option<ScaleSpec>,
  }

  #[derive(Debug, PartialEq, thiserror::Error)] // Eq dropped: see contract-correction note above
  pub enum TransformError {
      #[error("crop rect {width}x{height}+{x}+{y} exceeds frame bounds {frame_width}x{frame_height}")]
      CropOutOfBounds { width: u32, height: u32, x: u32, y: u32, frame_width: u32, frame_height: u32 },
      #[error("crop dimensions must be non-zero")]
      CropZeroSize,
      #[error("scale result must be non-zero (got {width}x{height})")]
      ScaleToZero { width: u32, height: u32 },
      #[error("scale factor must be finite and positive (got {factor})")]
      ScaleFactorInvalid { factor: f64 },
  }

  pub fn crop(frame: &Frame, rect: CropRect) -> Result<Frame, TransformError>;
  ```

#### Cycle 1 — config types + defaults

- [ ] Write the failing test. Create `crates/core/src/transform/config.rs`:
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;

      #[test]
      fn rotation_defaults_to_r0() {
          assert_eq!(Rotation::default(), Rotation::R0);
      }

      #[test]
      fn flip_defaults_to_keep() {
          assert_eq!(Flip::default(), Flip::Keep);
      }

      #[test]
      fn transform_config_defaults_to_no_op() {
          let config = TransformConfig::default();
          assert_eq!(config.crop, None);
          assert_eq!(config.rotation, Rotation::R0);
          assert_eq!(config.flip, Flip::Keep);
          assert_eq!(config.scale, None);
      }
  }
  ```
- [ ] Create `crates/core/src/transform/mod.rs` with just enough wiring to compile the test file as part of the crate:
  ```rust
  pub mod config;

  pub use config::{CropRect, Flip, Rotation, ScaleSpec, TransformConfig, TransformError};
  ```
- [ ] Add `pub mod transform;` to `crates/core/src/lib.rs` (now: `pub mod frame;` then `pub mod transform;`).
- [ ] Run `cargo test -p webcam-sharedtexture-core --lib transform::config::tests`. Expect
  **compile failure**: `error[E0412]: cannot find type `Rotation` in this scope` (and similarly for `Flip`, `TransformConfig`).
- [ ] Minimal implementation. Prepend to `crates/core/src/transform/config.rs`:
  ```rust
  #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
  pub enum Rotation {
      #[default]
      R0,
      R90,
      R180,
      R270,
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
  pub enum Flip {
      #[default]
      Keep,
      Horizontal,
      Vertical,
      Both,
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub struct CropRect {
      pub width: u32,
      pub height: u32,
      pub x: u32,
      pub y: u32,
  }

  #[derive(Debug, Clone, Copy, PartialEq)]
  pub enum ScaleSpec {
      Exact { width: u32, height: u32 },
      Factor(f64),
  }

  #[derive(Debug, Clone, PartialEq, Default)]
  pub struct TransformConfig {
      pub crop: Option<CropRect>,
      pub rotation: Rotation,
      pub flip: Flip,
      pub scale: Option<ScaleSpec>,
  }

  // NOTE: `Eq` intentionally dropped vs. the interface contract's literal
  // derive list — `ScaleFactorInvalid.factor: f64` cannot implement `Eq`
  // (NaN != NaN), so `#[derive(Eq)]` here would not compile.
  #[derive(Debug, PartialEq, thiserror::Error)]
  pub enum TransformError {
      #[error("crop rect {width}x{height}+{x}+{y} exceeds frame bounds {frame_width}x{frame_height}")]
      CropOutOfBounds { width: u32, height: u32, x: u32, y: u32, frame_width: u32, frame_height: u32 },
      #[error("crop dimensions must be non-zero")]
      CropZeroSize,
      #[error("scale result must be non-zero (got {width}x{height})")]
      ScaleToZero { width: u32, height: u32 },
      #[error("scale factor must be finite and positive (got {factor})")]
      ScaleFactorInvalid { factor: f64 },
  }
  ```
  (Note: `cargo fmt` collapses `CropOutOfBounds`'s field list onto the `#[error(...)]` line's
  neighbor since it fits within `max_width = 100` — the version above is already
  in canonical `cargo fmt` output, verified against this repo's `rustfmt.toml`.)
- [ ] Run `cargo test -p webcam-sharedtexture-core --lib transform::config::tests`. Expect
  `test result: ok. 3 passed; 0 failed;`.
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`.
- [ ] `git add crates/core/src/lib.rs crates/core/src/transform/mod.rs crates/core/src/transform/config.rs`, commit:
  ```
  feat(core): add transform config types (Rotation, Flip, CropRect, ScaleSpec, TransformConfig, TransformError)
  ```

#### Cycle 2 — `crop` happy path

- [ ] Write the failing test. Create `crates/core/src/transform/crop.rs`:
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;
      use crate::frame::Frame;

      fn sample_frame() -> Frame {
          let data = vec![
              10, 20, 30, 255, // (0,0)
              11, 21, 31, 255, // (1,0)
              12, 22, 32, 255, // (0,1)
              13, 23, 33, 255, // (1,1)
              14, 24, 34, 255, // (0,2)
              15, 25, 35, 255, // (1,2)
          ];
          Frame::new(2, 3, data).unwrap()
      }

      #[test]
      fn crops_the_requested_rect() {
          let frame = sample_frame();
          let rect = CropRect { width: 1, height: 2, x: 1, y: 1 };
          let result = crop(&frame, rect).unwrap();
          let expected = Frame::new(
              1,
              2,
              vec![
                  13, 23, 33, 255, // (0,0) <- input (1,1)
                  15, 25, 35, 255, // (0,1) <- input (1,2)
              ],
          )
          .unwrap();
          assert_eq!(result, expected);
      }
  }
  ```
- [ ] Add `pub mod crop;` to `crates/core/src/transform/mod.rs` (now: `pub mod config; pub mod crop;` + the existing `pub use`).
- [ ] Run `cargo test -p webcam-sharedtexture-core --lib transform::crop::tests`. Expect
  **compile failure**: `error[E0425]: cannot find function `crop` in this scope`.
- [ ] Minimal implementation (no bounds validation yet — that's Cycle 3). Prepend to `crates/core/src/transform/crop.rs`:
  ```rust
  use crate::frame::Frame;
  use crate::transform::config::{CropRect, TransformError};

  pub fn crop(frame: &Frame, rect: CropRect) -> Result<Frame, TransformError> {
      let mut data = Vec::new();
      for y in 0..rect.height {
          for x in 0..rect.width {
              let pixel = frame.pixel(rect.x + x, rect.y + y).unwrap_or([0, 0, 0, 0]);
              data.extend_from_slice(&pixel);
          }
      }
      Ok(Frame::from_validated(rect.width, rect.height, data))
  }
  ```
- [ ] Run `cargo test -p webcam-sharedtexture-core --lib transform::crop::tests`. Expect
  `test result: ok. 1 passed; 0 failed;`.
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`.
- [ ] `git add crates/core/src/transform/mod.rs crates/core/src/transform/crop.rs`, commit:
  ```
  feat(core): add crop happy path
  ```

#### Cycle 3 — `crop` bounds validation

- [ ] Write the failing test. Add to `crop.rs`'s `mod tests`:
  ```rust
      #[test]
      fn rejects_zero_size() {
          let frame = sample_frame();
          let rect = CropRect { width: 0, height: 1, x: 0, y: 0 };
          assert_eq!(crop(&frame, rect), Err(TransformError::CropZeroSize));
      }

      #[test]
      fn rejects_rect_exceeding_bounds() {
          let frame = sample_frame();
          let rect = CropRect { width: 2, height: 1, x: 1, y: 2 };
          assert_eq!(
              crop(&frame, rect),
              Err(TransformError::CropOutOfBounds {
                  width: 2,
                  height: 1,
                  x: 1,
                  y: 2,
                  frame_width: 2,
                  frame_height: 3,
              })
          );
      }
  ```
- [ ] Run `cargo test -p webcam-sharedtexture-core --lib transform::crop::tests`. Expect
  **assertion failures**: e.g. `rejects_zero_size` fails because Cycle 2's `crop` has no
  validation — a `width: 0` rect just produces an empty-but-`Ok` `Frame`, so
  `left: Ok(Frame { width: 0, height: 1, data: [] })`, `right: Err(CropZeroSize)`. Similarly
  `rejects_rect_exceeding_bounds` currently returns `Ok(..)` with wrapped-around/garbage pixels
  instead of `Err(CropOutOfBounds { .. })`.
- [ ] Minimal implementation. Replace the body of `crop` in `crop.rs`:
  ```rust
  pub fn crop(frame: &Frame, rect: CropRect) -> Result<Frame, TransformError> {
      if rect.width == 0 || rect.height == 0 {
          return Err(TransformError::CropZeroSize);
      }
      let right = rect.x.checked_add(rect.width);
      let bottom = rect.y.checked_add(rect.height);
      let fits = matches!(
          (right, bottom),
          (Some(r), Some(b)) if r <= frame.width() && b <= frame.height()
      );
      if !fits {
          return Err(TransformError::CropOutOfBounds {
              width: rect.width,
              height: rect.height,
              x: rect.x,
              y: rect.y,
              frame_width: frame.width(),
              frame_height: frame.height(),
          });
      }
      let mut data = Vec::new();
      for y in 0..rect.height {
          for x in 0..rect.width {
              let pixel = frame.pixel(rect.x + x, rect.y + y).unwrap_or([0, 0, 0, 0]);
              data.extend_from_slice(&pixel);
          }
      }
      Ok(Frame::from_validated(rect.width, rect.height, data))
  }
  ```
- [ ] Run `cargo test -p webcam-sharedtexture-core --lib transform::crop::tests`. Expect
  `test result: ok. 3 passed; 0 failed;`.
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`.
- [ ] `git add crates/core/src/transform/crop.rs`, commit:
  ```
  feat(core): validate crop bounds (CropZeroSize, CropOutOfBounds)
  ```

---

### Task 3: rotate

**Files:**
- Create: `crates/core/src/transform/rotate.rs`
- Modify: `crates/core/src/transform/mod.rs` — add `pub mod rotate;`
- Test: `crates/core/src/transform/rotate.rs`

**Interfaces:**
- Consumes: `Frame::pixel`, `Frame::width`, `Frame::height`, `Frame::from_validated` (Task 1); `Rotation` (Task 2)
- Produces: `pub fn rotate(frame: &Frame, rotation: Rotation) -> Frame;` (infallible)

**Rotation semantics (clockwise), derived and hand-verified against a 2×3 fixture before writing assertions:**
For a source frame of size `width_in x height_in`:
- `R0`: `output(x, y) = input(x, y)` — identity.
- `R90`: `output(x, y) = input(y, height_in - 1 - x)`; output size is `height_in x width_in` (dims swap).
- `R180`: `output(x, y) = input(width_in - 1 - x, height_in - 1 - y)`; dims unchanged.
- `R270`: `output(x, y) = input(width_in - 1 - y, x)`; output size is `height_in x width_in` (dims swap).

Because all four `Rotation` variants must be handled in one exhaustive `match`
(no `_` arm allowed), this task is a single TDD cycle covering all four cases
at once rather than four artificial increments — a partial `match` wouldn't
compile, and stub arms would be exactly the kind of placeholder this plan
must not contain.

#### Cycle 1 — all four rotations

- [ ] Write the failing test. Create `crates/core/src/transform/rotate.rs`:
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;
      use crate::frame::Frame;

      fn sample_frame() -> Frame {
          let data = vec![
              10, 20, 30, 255, // (0,0)
              11, 21, 31, 255, // (1,0)
              12, 22, 32, 255, // (0,1)
              13, 23, 33, 255, // (1,1)
              14, 24, 34, 255, // (0,2)
              15, 25, 35, 255, // (1,2)
          ];
          Frame::new(2, 3, data).unwrap()
      }

      #[test]
      fn r0_is_identity() {
          let frame = sample_frame();
          assert_eq!(rotate(&frame, Rotation::R0), frame);
      }

      #[test]
      fn r90_rotates_clockwise_and_swaps_dimensions() {
          let frame = sample_frame();
          let rotated = rotate(&frame, Rotation::R90);
          let expected = Frame::new(
              3,
              2,
              vec![
                  14, 24, 34, 255, // (0,0) <- input (0,2)
                  12, 22, 32, 255, // (1,0) <- input (0,1)
                  10, 20, 30, 255, // (2,0) <- input (0,0)
                  15, 25, 35, 255, // (0,1) <- input (1,2)
                  13, 23, 33, 255, // (1,1) <- input (1,1)
                  11, 21, 31, 255, // (2,1) <- input (1,0)
              ],
          )
          .unwrap();
          assert_eq!(rotated, expected);
          assert_eq!(rotated.width(), frame.height());
          assert_eq!(rotated.height(), frame.width());
      }

      #[test]
      fn r180_reverses_both_axes() {
          let frame = sample_frame();
          let rotated = rotate(&frame, Rotation::R180);
          let expected = Frame::new(
              2,
              3,
              vec![
                  15, 25, 35, 255, // (0,0) <- input (1,2)
                  14, 24, 34, 255, // (1,0) <- input (0,2)
                  13, 23, 33, 255, // (0,1) <- input (1,1)
                  12, 22, 32, 255, // (1,1) <- input (0,1)
                  11, 21, 31, 255, // (0,2) <- input (1,0)
                  10, 20, 30, 255, // (1,2) <- input (0,0)
              ],
          )
          .unwrap();
          assert_eq!(rotated, expected);
      }

      #[test]
      fn r270_rotates_counterclockwise_and_swaps_dimensions() {
          let frame = sample_frame();
          let rotated = rotate(&frame, Rotation::R270);
          let expected = Frame::new(
              3,
              2,
              vec![
                  11, 21, 31, 255, // (0,0) <- input (1,0)
                  13, 23, 33, 255, // (1,0) <- input (1,1)
                  15, 25, 35, 255, // (2,0) <- input (1,2)
                  10, 20, 30, 255, // (0,1) <- input (0,0)
                  12, 22, 32, 255, // (1,1) <- input (0,1)
                  14, 24, 34, 255, // (2,1) <- input (0,2)
              ],
          )
          .unwrap();
          assert_eq!(rotated, expected);
          assert_eq!(rotated.width(), frame.height());
          assert_eq!(rotated.height(), frame.width());
      }
  }
  ```
- [ ] Add `pub mod rotate;` to `crates/core/src/transform/mod.rs`.
- [ ] Run `cargo test -p webcam-sharedtexture-core --lib transform::rotate::tests`. Expect
  **compile failure**: `error[E0425]: cannot find function `rotate` in this scope`.
- [ ] Minimal implementation. Prepend to `crates/core/src/transform/rotate.rs`:
  ```rust
  //! Clockwise rotation. R90/R270 swap width and height; R180 keeps them.
  //!
  //! For a source frame of size `width_in x height_in`:
  //! - R90:  output(x, y) = input(y, height_in - 1 - x)
  //! - R180: output(x, y) = input(width_in - 1 - x, height_in - 1 - y)
  //! - R270: output(x, y) = input(width_in - 1 - y, x)

  use crate::frame::Frame;
  use crate::transform::config::Rotation;

  pub fn rotate(frame: &Frame, rotation: Rotation) -> Frame {
      match rotation {
          Rotation::R0 => frame.clone(),
          Rotation::R90 => rotate_r90(frame),
          Rotation::R180 => rotate_r180(frame),
          Rotation::R270 => rotate_r270(frame),
      }
  }

  fn rotate_r90(frame: &Frame) -> Frame {
      let width_in = frame.width();
      let height_in = frame.height();
      let width_out = height_in;
      let height_out = width_in;
      let mut data = Vec::new();
      for y_out in 0..height_out {
          for x_out in 0..width_out {
              let pixel = frame.pixel(y_out, height_in - 1 - x_out).unwrap_or([0, 0, 0, 0]);
              data.extend_from_slice(&pixel);
          }
      }
      Frame::from_validated(width_out, height_out, data)
  }

  fn rotate_r180(frame: &Frame) -> Frame {
      let width = frame.width();
      let height = frame.height();
      let mut data = Vec::new();
      for y in 0..height {
          for x in 0..width {
              let pixel = frame.pixel(width - 1 - x, height - 1 - y).unwrap_or([0, 0, 0, 0]);
              data.extend_from_slice(&pixel);
          }
      }
      Frame::from_validated(width, height, data)
  }

  fn rotate_r270(frame: &Frame) -> Frame {
      let width_in = frame.width();
      let height_in = frame.height();
      let width_out = height_in;
      let height_out = width_in;
      let mut data = Vec::new();
      for y_out in 0..height_out {
          for x_out in 0..width_out {
              let pixel = frame.pixel(width_in - 1 - y_out, x_out).unwrap_or([0, 0, 0, 0]);
              data.extend_from_slice(&pixel);
          }
      }
      Frame::from_validated(width_out, height_out, data)
  }
  ```
- [ ] Run `cargo test -p webcam-sharedtexture-core --lib transform::rotate::tests`. Expect
  `test result: ok. 4 passed; 0 failed;`.
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`.
- [ ] `git add crates/core/src/transform/mod.rs crates/core/src/transform/rotate.rs`, commit:
  ```
  feat(core): add clockwise rotate (R0/R90/R180/R270)
  ```

---

### Task 4: flip

**Files:**
- Create: `crates/core/src/transform/flip.rs`
- Modify: `crates/core/src/transform/mod.rs` — add `pub mod flip;`
- Test: `crates/core/src/transform/flip.rs`

**Interfaces:**
- Consumes: `Frame::pixel`, `Frame::width`, `Frame::height`, `Frame::from_validated` (Task 1); `Flip` (Task 2)
- Produces: `pub fn flip(frame: &Frame, direction: Flip) -> Frame;` (infallible)

**Flip semantics (dimensions never change):**
- `Keep`: identity.
- `Horizontal`: `output(x, y) = input(width - 1 - x, y)`.
- `Vertical`: `output(x, y) = input(x, height - 1 - y)`.
- `Both`: `output(x, y) = input(width - 1 - x, height - 1 - y)`.

Same exhaustive-`match`-over-4-variants reasoning as Task 3 applies: one cycle covering all four.

#### Cycle 1 — all four flip directions

- [ ] Write the failing test. Create `crates/core/src/transform/flip.rs`:
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;
      use crate::frame::Frame;

      fn sample_frame() -> Frame {
          let data = vec![
              10, 20, 30, 255, // (0,0)
              11, 21, 31, 255, // (1,0)
              12, 22, 32, 255, // (0,1)
              13, 23, 33, 255, // (1,1)
              14, 24, 34, 255, // (0,2)
              15, 25, 35, 255, // (1,2)
          ];
          Frame::new(2, 3, data).unwrap()
      }

      #[test]
      fn keep_is_identity() {
          let frame = sample_frame();
          assert_eq!(flip(&frame, Flip::Keep), frame);
      }

      #[test]
      fn horizontal_mirrors_columns() {
          let frame = sample_frame();
          let flipped = flip(&frame, Flip::Horizontal);
          let expected = Frame::new(
              2,
              3,
              vec![
                  11, 21, 31, 255, // (0,0) <- input (1,0)
                  10, 20, 30, 255, // (1,0) <- input (0,0)
                  13, 23, 33, 255, // (0,1) <- input (1,1)
                  12, 22, 32, 255, // (1,1) <- input (0,1)
                  15, 25, 35, 255, // (0,2) <- input (1,2)
                  14, 24, 34, 255, // (1,2) <- input (0,2)
              ],
          )
          .unwrap();
          assert_eq!(flipped, expected);
      }

      #[test]
      fn vertical_mirrors_rows() {
          let frame = sample_frame();
          let flipped = flip(&frame, Flip::Vertical);
          let expected = Frame::new(
              2,
              3,
              vec![
                  14, 24, 34, 255, // (0,0) <- input (0,2)
                  15, 25, 35, 255, // (1,0) <- input (1,2)
                  12, 22, 32, 255, // (0,1) <- input (0,1)
                  13, 23, 33, 255, // (1,1) <- input (1,1)
                  10, 20, 30, 255, // (0,2) <- input (0,0)
                  11, 21, 31, 255, // (1,2) <- input (1,0)
              ],
          )
          .unwrap();
          assert_eq!(flipped, expected);
      }

      #[test]
      fn both_mirrors_rows_and_columns() {
          let frame = sample_frame();
          let flipped = flip(&frame, Flip::Both);
          let expected = Frame::new(
              2,
              3,
              vec![
                  15, 25, 35, 255, // (0,0) <- input (1,2)
                  14, 24, 34, 255, // (1,0) <- input (0,2)
                  13, 23, 33, 255, // (0,1) <- input (1,1)
                  12, 22, 32, 255, // (1,1) <- input (0,1)
                  11, 21, 31, 255, // (0,2) <- input (1,0)
                  10, 20, 30, 255, // (1,2) <- input (0,0)
              ],
          )
          .unwrap();
          assert_eq!(flipped, expected);
      }
  }
  ```
- [ ] Add `pub mod flip;` to `crates/core/src/transform/mod.rs`.
- [ ] Run `cargo test -p webcam-sharedtexture-core --lib transform::flip::tests`. Expect
  **compile failure**: `error[E0425]: cannot find function `flip` in this scope`.
- [ ] Minimal implementation. Prepend to `crates/core/src/transform/flip.rs`:
  ```rust
  //! Mirror flips. Dimensions never change.
  //!
  //! - Horizontal: output(x, y) = input(width - 1 - x, y)
  //! - Vertical:   output(x, y) = input(x, height - 1 - y)
  //! - Both:       output(x, y) = input(width - 1 - x, height - 1 - y)

  use crate::frame::Frame;
  use crate::transform::config::Flip;

  pub fn flip(frame: &Frame, direction: Flip) -> Frame {
      match direction {
          Flip::Keep => frame.clone(),
          Flip::Horizontal => flip_horizontal(frame),
          Flip::Vertical => flip_vertical(frame),
          Flip::Both => flip_both(frame),
      }
  }

  fn flip_horizontal(frame: &Frame) -> Frame {
      let width = frame.width();
      let height = frame.height();
      let mut data = Vec::new();
      for y in 0..height {
          for x in 0..width {
              let pixel = frame.pixel(width - 1 - x, y).unwrap_or([0, 0, 0, 0]);
              data.extend_from_slice(&pixel);
          }
      }
      Frame::from_validated(width, height, data)
  }

  fn flip_vertical(frame: &Frame) -> Frame {
      let width = frame.width();
      let height = frame.height();
      let mut data = Vec::new();
      for y in 0..height {
          for x in 0..width {
              let pixel = frame.pixel(x, height - 1 - y).unwrap_or([0, 0, 0, 0]);
              data.extend_from_slice(&pixel);
          }
      }
      Frame::from_validated(width, height, data)
  }

  fn flip_both(frame: &Frame) -> Frame {
      let width = frame.width();
      let height = frame.height();
      let mut data = Vec::new();
      for y in 0..height {
          for x in 0..width {
              let pixel = frame.pixel(width - 1 - x, height - 1 - y).unwrap_or([0, 0, 0, 0]);
              data.extend_from_slice(&pixel);
          }
      }
      Frame::from_validated(width, height, data)
  }
  ```
- [ ] Run `cargo test -p webcam-sharedtexture-core --lib transform::flip::tests`. Expect
  `test result: ok. 4 passed; 0 failed;`.
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`.
- [ ] `git add crates/core/src/transform/mod.rs crates/core/src/transform/flip.rs`, commit:
  ```
  feat(core): add mirror flip (Keep/Horizontal/Vertical/Both)
  ```

---

### Task 5: scale

**Files:**
- Create: `crates/core/src/transform/scale.rs`
- Modify: `crates/core/src/transform/mod.rs` — add `pub mod scale;`
- Test: `crates/core/src/transform/scale.rs`

**Interfaces:**
- Consumes: `Frame::pixel`, `Frame::width`, `Frame::height`, `Frame::from_validated` (Task 1); `ScaleSpec`, `TransformError` (Task 2)
- Produces:
  ```rust
  pub fn scale(frame: &Frame, spec: ScaleSpec) -> Result<Frame, TransformError>;
  #[allow(clippy::as_conversions)] fn scale_dimension(dim: u32, factor: f64) -> u32; // the only `as` casts in core
  ```

Nearest-neighbor mapping: for output length `dst_len` and source length
`src_len`, `output[i]` samples `source[floor(i * src_len / dst_len)]`,
computed in `u64` and converted back with `u32::try_from(..).unwrap_or(u32::MAX)`
(never `as`) — the `#[allow(clippy::as_conversions)]` on `scale_dimension` is
the *only* place in `core` that casts, per the contract.

#### Cycle 1 — `scale` happy path (Exact + Factor)

- [ ] Write the failing test. Create `crates/core/src/transform/scale.rs`:
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;
      use crate::frame::Frame;

      fn sample_frame() -> Frame {
          let data = vec![
              10, 20, 30, 255, // (0,0) A
              11, 21, 31, 255, // (1,0) B
              12, 22, 32, 255, // (0,1) C
              13, 23, 33, 255, // (1,1) D
              14, 24, 34, 255, // (0,2) E
              15, 25, 35, 255, // (1,2) F
          ];
          Frame::new(2, 3, data).unwrap()
      }

      #[test]
      fn exact_upscale_duplicates_pixels() {
          let frame = sample_frame();
          let result = scale(&frame, ScaleSpec::Exact { width: 4, height: 6 }).unwrap();
          let a = [10, 20, 30, 255];
          let b = [11, 21, 31, 255];
          let c = [12, 22, 32, 255];
          let d = [13, 23, 33, 255];
          let e = [14, 24, 34, 255];
          let f = [15, 25, 35, 255];
          let expected_data = [
              a, a, b, b, // row0
              a, a, b, b, // row1
              c, c, d, d, // row2
              c, c, d, d, // row3
              e, e, f, f, // row4
              e, e, f, f, // row5
          ]
          .concat();
          let expected = Frame::new(4, 6, expected_data).unwrap();
          assert_eq!(result, expected);
      }

      #[test]
      fn factor_downscale_picks_nearest_source_pixel() {
          let frame = sample_frame();
          let result = scale(&frame, ScaleSpec::Factor(0.5)).unwrap();
          let expected = Frame::new(
              1,
              2,
              vec![
                  10, 20, 30, 255, // (0,0) <- input (0,0) A
                  12, 22, 32, 255, // (0,1) <- input (0,1) C
              ],
          )
          .unwrap();
          assert_eq!(result, expected);
      }
  }
  ```
- [ ] Add `pub mod scale;` to `crates/core/src/transform/mod.rs`.
- [ ] Run `cargo test -p webcam-sharedtexture-core --lib transform::scale::tests`. Expect
  **compile failure**: `error[E0425]: cannot find function `scale` in this scope`.
- [ ] Minimal implementation (no zero/invalid-factor guards yet — that's Cycle 2). Prepend to `crates/core/src/transform/scale.rs`:
  ```rust
  //! Nearest-neighbor scaling. `ScaleSpec::Exact` picks the target size
  //! directly; `ScaleSpec::Factor` multiplies both dimensions.

  use crate::frame::Frame;
  use crate::transform::config::{ScaleSpec, TransformError};

  pub fn scale(frame: &Frame, spec: ScaleSpec) -> Result<Frame, TransformError> {
      let (width_out, height_out) = target_dims(frame, spec)?;
      let mut data = Vec::new();
      for y_out in 0..height_out {
          let y_src = nearest_index(y_out, height_out, frame.height());
          for x_out in 0..width_out {
              let x_src = nearest_index(x_out, width_out, frame.width());
              let pixel = frame.pixel(x_src, y_src).unwrap_or([0, 0, 0, 0]);
              data.extend_from_slice(&pixel);
          }
      }
      Ok(Frame::from_validated(width_out, height_out, data))
  }

  fn target_dims(frame: &Frame, spec: ScaleSpec) -> Result<(u32, u32), TransformError> {
      match spec {
          ScaleSpec::Exact { width, height } => Ok((width, height)),
          ScaleSpec::Factor(factor) => {
              let width = scale_dimension(frame.width(), factor);
              let height = scale_dimension(frame.height(), factor);
              Ok((width, height))
          }
      }
  }

  fn nearest_index(dst: u32, dst_len: u32, src_len: u32) -> u32 {
      let dst = u64::from(dst);
      let dst_len = u64::from(dst_len).max(1); // defensive: scale() only ever passes a validated non-zero dst_len
      let src_len = u64::from(src_len);
      let src = dst * src_len / dst_len;
      u32::try_from(src).unwrap_or(u32::MAX)
  }

  #[allow(clippy::as_conversions)]
  fn scale_dimension(dim: u32, factor: f64) -> u32 {
      // u32 -> f64 is lossless; f64 -> u32 has no infallible std conversion
      // (TryFrom<f64> does not exist), so this is core's one deliberate `as`
      // cast, applied only after round() + clamp() bound the value to u32.
      let scaled = (dim as f64) * factor;
      scaled.round().clamp(1.0, u32::MAX as f64) as u32
  }
  ```
- [ ] Run `cargo test -p webcam-sharedtexture-core --lib transform::scale::tests`. Expect
  `test result: ok. 2 passed; 0 failed;`.
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`.
- [ ] `git add crates/core/src/transform/mod.rs crates/core/src/transform/scale.rs`, commit:
  ```
  feat(core): add nearest-neighbor scale (Exact/Factor)
  ```

#### Cycle 2 — `scale` validation (`ScaleToZero`, `ScaleFactorInvalid`)

- [ ] Write the failing test. Add to `scale.rs`'s `mod tests`:
  ```rust
      #[test]
      fn exact_zero_size_is_rejected() {
          let frame = sample_frame();
          let result = scale(&frame, ScaleSpec::Exact { width: 0, height: 5 });
          assert_eq!(result, Err(TransformError::ScaleToZero { width: 0, height: 5 }));
      }

      #[test]
      fn negative_factor_is_rejected() {
          let frame = sample_frame();
          let result = scale(&frame, ScaleSpec::Factor(-1.0));
          assert_eq!(result, Err(TransformError::ScaleFactorInvalid { factor: -1.0 }));
      }

      #[test]
      fn non_finite_factor_is_rejected() {
          let frame = sample_frame();
          let result = scale(&frame, ScaleSpec::Factor(f64::NAN));
          match result {
              Err(TransformError::ScaleFactorInvalid { factor }) => assert!(factor.is_nan()),
              other => panic!("expected ScaleFactorInvalid, got {other:?}"),
          }
      }
  ```
  (`non_finite_factor_is_rejected` can't use `assert_eq!` against a literal
  `ScaleFactorInvalid { factor: f64::NAN }` — `NaN != NaN` under `PartialEq`,
  so it would spuriously fail even with a correct implementation. Matching
  the variant and asserting `is_nan()` separately is the correct check.)
- [ ] Run `cargo test -p webcam-sharedtexture-core --lib transform::scale::tests`. Expect
  **assertion failures**: `exact_zero_size_is_rejected` fails because Cycle 1's `target_dims`
  doesn't check for zero (`left: Ok(Frame { width: 0, height: 5, data: [] })`, `right: Err(ScaleToZero { .. })`).
  `negative_factor_is_rejected` fails because `scale_dimension` silently clamps a negative
  factor's result up to `1` (`left: Ok(Frame { width: 1, height: 1, .. })`, `right: Err(ScaleFactorInvalid { factor: -1.0 })`).
  `non_finite_factor_is_rejected` fails with `expected ScaleFactorInvalid, got Ok(Frame { width: 0, height: 0, data: [] })`
  (NaN propagates through `scale_dimension` and casts to `0`).
- [ ] Minimal implementation. Replace `target_dims` in `scale.rs`:
  ```rust
  fn target_dims(frame: &Frame, spec: ScaleSpec) -> Result<(u32, u32), TransformError> {
      match spec {
          ScaleSpec::Exact { width, height } => {
              if width == 0 || height == 0 {
                  return Err(TransformError::ScaleToZero { width, height });
              }
              Ok((width, height))
          }
          ScaleSpec::Factor(factor) => {
              if !factor.is_finite() || factor <= 0.0 {
                  return Err(TransformError::ScaleFactorInvalid { factor });
              }
              let width = scale_dimension(frame.width(), factor);
              let height = scale_dimension(frame.height(), factor);
              Ok((width, height))
          }
      }
  }
  ```
- [ ] Run `cargo test -p webcam-sharedtexture-core --lib transform::scale::tests`. Expect
  `test result: ok. 5 passed; 0 failed;`.
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`.
- [ ] `git add crates/core/src/transform/scale.rs`, commit:
  ```
  feat(core): validate scale target (ScaleToZero, ScaleFactorInvalid)
  ```

---

### Task 6: `TransformConfig::apply` composition

**Files:**
- Modify: `crates/core/src/transform/mod.rs` — add `pub fn apply` + tests
- Test: `crates/core/src/transform/mod.rs`

**Interfaces:**
- Consumes: `crop::crop`, `rotate::rotate`, `flip::flip`, `scale::scale`, `TransformConfig`, `TransformError` (Tasks 2–5)
- Produces: `pub fn apply(frame: &Frame, config: &TransformConfig) -> Result<Frame, TransformError>;`
  — fixed order **crop → rotate → flip → scale**; each stage is skipped
  (treated as identity) when its `Option` field is `None` (`crop`, `scale`)
  or its enum is the no-op default (`rotation: Rotation::R0`, `flip: Flip::Keep`
  — those are already identity inside `rotate`/`flip` themselves, so `apply`
  always calls them unconditionally).

#### Cycle 1 — composition + fixed order

- [ ] Write the failing test. Add to `crates/core/src/transform/mod.rs` a new `#[cfg(test)] mod tests` block:
  ```rust
  #[cfg(test)]
  mod tests {
      use super::*;
      use crate::frame::Frame;

      fn sample_frame() -> Frame {
          let data = vec![
              10, 20, 30, 255, // (0,0) A
              11, 21, 31, 255, // (1,0) B
              12, 22, 32, 255, // (0,1) C
              13, 23, 33, 255, // (1,1) D
              14, 24, 34, 255, // (0,2) E
              15, 25, 35, 255, // (1,2) F
          ];
          Frame::new(2, 3, data).unwrap()
      }

      #[test]
      fn apply_runs_crop_then_rotate_then_flip_then_scale() {
          let frame = sample_frame();
          let config = TransformConfig {
              crop: Some(CropRect { width: 2, height: 2, x: 0, y: 0 }),
              rotation: Rotation::R90,
              flip: Flip::Horizontal,
              scale: Some(ScaleSpec::Exact { width: 4, height: 4 }),
          };
          let result = apply(&frame, &config).unwrap();

          let a = [10, 20, 30, 255];
          let b = [11, 21, 31, 255];
          let c = [12, 22, 32, 255];
          let d = [13, 23, 33, 255];
          let expected_data = [
              a, a, c, c, // row0
              a, a, c, c, // row1
              b, b, d, d, // row2
              b, b, d, d, // row3
          ]
          .concat();
          let expected = Frame::new(4, 4, expected_data).unwrap();
          assert_eq!(result, expected);
      }

      #[test]
      fn apply_with_default_config_is_identity() {
          let frame = sample_frame();
          let result = apply(&frame, &TransformConfig::default()).unwrap();
          assert_eq!(result, frame);
      }
  }
  ```
  (Trace for the first test: cropping `{0,0,2,2}` from the 2×3 fixture keeps
  the top two rows `A B / C D`. Rotating that 2×2 block 90° clockwise via
  `output(x,y) = input(y, 1-x)` gives `C A / D B`. Flipping that horizontally
  via `output(x,y) = input(1-x,y)` gives `A C / B D`. Scaling that 2×2 to 4×4
  nearest-neighbor duplicates each pixel into a 2×2 block, giving the
  `expected_data` above.)
- [ ] Run `cargo test -p webcam-sharedtexture-core --lib transform::tests`. Expect
  **compile failure**: `error[E0425]: cannot find function `apply` in this scope`.
- [ ] Minimal implementation. Add to `crates/core/src/transform/mod.rs` (above the test module):
  ```rust
  use crate::frame::Frame;

  pub fn apply(frame: &Frame, config: &TransformConfig) -> Result<Frame, TransformError> {
      let cropped = match config.crop {
          Some(rect) => crop::crop(frame, rect)?,
          None => frame.clone(),
      };
      let rotated = rotate::rotate(&cropped, config.rotation);
      let flipped = flip::flip(&rotated, config.flip);
      let scaled = match config.scale {
          Some(spec) => scale::scale(&flipped, spec)?,
          None => flipped,
      };
      Ok(scaled)
  }
  ```
- [ ] Run `cargo test -p webcam-sharedtexture-core --lib transform::tests`. Expect
  `test result: ok. 2 passed; 0 failed;`.
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`.
- [ ] `git add crates/core/src/transform/mod.rs`, commit:
  ```
  feat(core): add TransformConfig::apply composition (crop -> rotate -> flip -> scale)
  ```

#### Cycle 2 — property tests

- [ ] Write the failing test. Add to `transform/mod.rs`'s `mod tests`:
  ```rust
      #[test]
      fn rotate_four_times_returns_original() {
          let frame = sample_frame();
          let mut current = frame.clone();
          for _ in 0..4 {
              current = rotate::rotate(&current, Rotation::R90);
          }
          assert_eq!(current, frame);
      }

      #[test]
      fn flip_twice_returns_original() {
          let frame = sample_frame();
          let once = flip::flip(&frame, Flip::Horizontal);
          let twice = flip::flip(&once, Flip::Horizontal);
          assert_eq!(twice, frame);
      }

      #[test]
      fn rotate_ninety_swaps_dimensions() {
          let frame = sample_frame();
          let rotated = rotate::rotate(&frame, Rotation::R90);
          assert_eq!(rotated.width(), frame.height());
          assert_eq!(rotated.height(), frame.width());
      }
  ```
  These are not new behavior (Tasks 3–4 already implement `rotate`/`flip`
  correctly), so this cycle is "red" only in the trivial sense that the tests
  don't exist yet — running them immediately after adding should pass. To
  keep the red/green discipline meaningful: temporarily change `rotate_r90`'s
  formula to `input(x, y)` (a deliberate bug) before running, confirm the
  three tests fail (`rotate_four_times_returns_original` and
  `rotate_ninety_swaps_dimensions` both fail — the swapped-dims assertion
  fails outright since a broken identity-like rotate wouldn't swap
  dimensions), then revert to the correct formula from Task 3 and re-run.
- [ ] Run `cargo test -p webcam-sharedtexture-core --lib transform::tests` (with the deliberate bug in place).
  Expect **assertion failures**, e.g. `rotate_ninety_swaps_dimensions` panics with
  `assertion `left == right` failed` because a bugged `rotate_r90` no longer swaps `width`/`height`.
- [ ] Revert `rotate_r90` to the Task 3 implementation (no other code changes needed — `rotate`, `flip` are already correct).
- [ ] Run `cargo test -p webcam-sharedtexture-core --lib transform::tests`. Expect
  `test result: ok. 5 passed; 0 failed;`.
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`.
- [ ] `git add crates/core/src/transform/mod.rs`, commit:
  ```
  test(core): add rotate/flip round-trip and dimension-swap property tests
  ```
## Section B — Tasks 7–9 (crates/core: traits, pipeline, nokhwa capture)

Global reminders (see INTERFACE-CONTRACT.md for the full text, copied here for this section's scope):

- t-wada TDD: failing test first → run (expect FAIL) → minimal impl → run (expect PASS) → lint gate → commit.
- Lint gate (run after every cycle, exact command):
  `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`
- Test command for this crate: `cargo test -p webcam-sharedtexture-core <filter>`
- `unwrap()`/`expect()` are denied by clippy everywhere except `#[cfg(test)]` modules (clippy.toml exempts tests, created in Task 1).
- No `as` casts anywhere in these three tasks (the only sanctioned `as` sites are `transform/scale.rs` and the syphon FFI boundary, both out of scope here).
- Exhaustive `match` without `_` on owned/borrowed enums we define or that come from a small fixed dependency enum.
- Function names ≤ 3 words.
- Commits: conventional commit messages, do NOT push.

---

### Task 7: publish + capture traits

**Files:**
- Create: `crates/core/src/capture.rs`
- Create: `crates/core/src/publish.rs`
- Modify: `crates/core/src/lib.rs`
- Test: inline `#[cfg(test)] mod tests` blocks inside `capture.rs` and `publish.rs`

**Interfaces:**
- Consumes: `Frame` from `crates/core/src/frame.rs` (Task 1) — `Frame::new(width: u32, height: u32, data: Vec<u8>) -> Result<Self, FrameError>`, `Frame` derives `Debug, Clone, PartialEq, Eq`.
- Produces (for Task 8 `pipeline.rs` and Task 9 `NokhwaSource`, and later `crates/syphon`, `crates/cli`):
  - `pub trait CaptureSource { fn next_frame(&mut self) -> Result<Frame, CaptureError>; }`
  - `pub struct DeviceInfo { pub index: u32, pub name: String }`
  - `pub enum CaptureError { NoDevices, DeviceNotFound { index, available }, OpenFailed { index, reason }, FrameRead { reason }, FormatUnsupported { format } }`
  - `pub trait TexturePublisher { fn publish(&mut self, frame: &Frame) -> Result<(), PublishError>; }`
  - `pub enum PublishError { ServerCreate { name, reason }, Publish { reason } }`

These two traits have no behavior of their own — the TDD cycle proves (a) the types compile as specified in the contract, and (b) both traits are object-safe (`&mut dyn CaptureSource`, `&mut dyn TexturePublisher` are constructible and callable), using a trivial recording test double per trait.

#### Cycle 7.1 — `CaptureSource` / `CaptureError` / `DeviceInfo`

- [ ] Modify `crates/core/src/lib.rs`: insert `pub mod capture;` immediately after the existing `pub mod transform;` line.
- [ ] Create `crates/core/src/capture.rs` containing **only** the test module below (the trait/enum/struct it references do not exist yet — this is the RED step):

  ```rust
  #[cfg(test)]
  mod tests {
      use super::{CaptureError, CaptureSource, DeviceInfo};
      use crate::frame::Frame;

      struct RecordingSource {
          frames: Vec<Frame>,
          calls: u32,
      }

      impl CaptureSource for RecordingSource {
          fn next_frame(&mut self) -> Result<Frame, CaptureError> {
              self.calls += 1;
              self.frames.pop().ok_or_else(|| CaptureError::FrameRead {
                  reason: "exhausted".to_string(),
              })
          }
      }

      #[test]
      fn dyn_capture_source_returns_recorded_frame() {
          let frame = Frame::new(1, 1, vec![10, 20, 30, 255]).expect("valid frame");
          let mut recording = RecordingSource {
              frames: vec![frame.clone()],
              calls: 0,
          };
          // This binding is the object-safety proof: CaptureSource has no generic
          // methods and takes `&mut self`, so it coerces to a trait object.
          let source: &mut dyn CaptureSource = &mut recording;

          let result = source.next_frame().expect("frame available");

          assert_eq!(result, frame);
          assert_eq!(recording.calls, 1);
      }

      #[test]
      fn device_info_holds_index_and_name() {
          let info = DeviceInfo { index: 2, name: "Logi C920".to_string() };

          assert_eq!(info.index, 2);
          assert_eq!(info.name, "Logi C920");
      }
  }
  ```

- [ ] Run: `cargo test -p webcam-sharedtexture-core --lib capture`
  Expected FAIL (compile error, not a test failure):
  ```
  error[E0432]: unresolved import `super::CaptureError`
   --> crates/core/src/capture.rs:2:16
    |
  2 |     use super::{CaptureError, CaptureSource, DeviceInfo};
    |                 ^^^^^^^^^^^^ no `CaptureError` in `capture`
  ```
- [ ] Add the minimal implementation above the `#[cfg(test)]` module in `capture.rs`:

  ```rust
  use thiserror::Error;

  use crate::frame::Frame;

  pub trait CaptureSource {
      fn next_frame(&mut self) -> Result<Frame, CaptureError>;
  }

  #[derive(Debug, Clone, PartialEq, Eq)]
  pub struct DeviceInfo {
      pub index: u32,
      pub name: String,
  }

  #[derive(Debug, Error)]
  pub enum CaptureError {
      #[error("no capture devices found")]
      NoDevices,
      #[error("device index {index} not found ({available} devices available)")]
      DeviceNotFound { index: u32, available: usize },
      #[error("failed to open device {index}: {reason}")]
      OpenFailed { index: u32, reason: String },
      #[error("failed to read frame: {reason}")]
      FrameRead { reason: String },
      #[error("unsupported camera pixel format: {format}")]
      FormatUnsupported { format: String },
  }
  ```

- [ ] Run: `cargo test -p webcam-sharedtexture-core --lib capture`
  Expected PASS:
  ```
  running 2 tests
  test capture::tests::device_info_holds_index_and_name ... ok
  test capture::tests::dyn_capture_source_returns_recorded_frame ... ok

  test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
  ```
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`
- [ ] Commit:
  ```
  git add crates/core/src/capture.rs crates/core/src/lib.rs
  git commit -m "feat(core): define CaptureSource trait, CaptureError, DeviceInfo"
  ```

#### Cycle 7.2 — `TexturePublisher` / `PublishError`

- [ ] Modify `crates/core/src/lib.rs`: insert `pub mod publish;` immediately after the `pub mod capture;` line just added.
- [ ] Create `crates/core/src/publish.rs` containing **only** the test module below (RED step):

  ```rust
  #[cfg(test)]
  mod tests {
      use super::{PublishError, TexturePublisher};
      use crate::frame::Frame;

      struct RecordingPublisher {
          received: Vec<Frame>,
      }

      impl TexturePublisher for RecordingPublisher {
          fn publish(&mut self, frame: &Frame) -> Result<(), PublishError> {
              self.received.push(frame.clone());

              Ok(())
          }
      }

      #[test]
      fn dyn_texture_publisher_records_published_frame() {
          let frame = Frame::new(1, 1, vec![1, 2, 3, 255]).expect("valid frame");
          let mut recording = RecordingPublisher { received: Vec::new() };
          // Object-safety proof, mirroring CaptureSource in capture.rs.
          let publisher: &mut dyn TexturePublisher = &mut recording;

          publisher.publish(&frame).expect("publish succeeds");

          assert_eq!(recording.received, vec![frame]);
      }
  }
  ```

- [ ] Run: `cargo test -p webcam-sharedtexture-core --lib publish`
  Expected FAIL (compile error):
  ```
  error[E0432]: unresolved import `super::PublishError`
   --> crates/core/src/publish.rs:2:16
    |
  2 |     use super::{PublishError, TexturePublisher};
    |                 ^^^^^^^^^^^^ no `PublishError` in `publish`
  ```
- [ ] Add the minimal implementation above the `#[cfg(test)]` module in `publish.rs`:

  ```rust
  use thiserror::Error;

  use crate::frame::Frame;

  pub trait TexturePublisher {
      fn publish(&mut self, frame: &Frame) -> Result<(), PublishError>;
  }

  #[derive(Debug, Error)]
  pub enum PublishError {
      #[error("failed to create texture server \"{name}\": {reason}")]
      ServerCreate { name: String, reason: String },
      #[error("failed to publish frame: {reason}")]
      Publish { reason: String },
  }
  ```

- [ ] Run: `cargo test -p webcam-sharedtexture-core --lib publish`
  Expected PASS:
  ```
  running 1 test
  test publish::tests::dyn_texture_publisher_records_published_frame ... ok

  test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
  ```
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`
- [ ] Commit:
  ```
  git add crates/core/src/publish.rs crates/core/src/lib.rs
  git commit -m "feat(core): define TexturePublisher trait and PublishError"
  ```

---

### Task 8: pipeline

**Files:**
- Create: `crates/core/src/pipeline.rs`
- Modify: `crates/core/src/lib.rs`
- Test: inline `#[cfg(test)] mod tests` block inside `pipeline.rs`

**Interfaces:**
- Consumes:
  - `Frame` (Task 1): `width()`, `height()`, `pixel(x, y)`, derives `PartialEq, Eq, Clone`.
  - `crate::transform::{apply, TransformConfig, TransformError, Rotation}` (Tasks 3–6): `pub fn apply(frame: &Frame, config: &TransformConfig) -> Result<Frame, TransformError>`.
  - `crate::capture::{CaptureSource, CaptureError}` (Task 7).
  - `crate::publish::{TexturePublisher, PublishError}` (Task 7).
- Produces (for `crates/cli/src/run.rs` in a later task, and `crates/gui`):
  - `pub enum PipelineError { Capture(#[from] CaptureError), Transform(#[from] TransformError), Publish(#[from] PublishError) }`
  - `pub fn run_pipeline(source: &mut dyn CaptureSource, config: &TransformConfig, publisher: &mut dyn TexturePublisher, stop: &AtomicBool) -> Result<(), PipelineError>`

This task is a single function with one clear contract ("loop next_frame → apply → publish until `stop`, propagate any error"), so its four required behaviors are captured as one RED/GREEN cycle: all four tests are written together against the not-yet-existing `run_pipeline`/`PipelineError`, then one minimal implementation makes all four pass.

#### Cycle 8.1 — `run_pipeline` + `PipelineError`

- [ ] Modify `crates/core/src/lib.rs`: insert `pub mod pipeline;` immediately after the `pub mod publish;` line (matches contract order: frame, transform, capture, publish, pipeline).
- [ ] Create `crates/core/src/pipeline.rs` containing **only** the test module below (RED step — `run_pipeline` and `PipelineError` do not exist yet):

  ```rust
  #[cfg(test)]
  mod tests {
      use std::collections::VecDeque;
      use std::sync::atomic::{AtomicBool, Ordering};
      use std::sync::Arc;

      use super::{run_pipeline, PipelineError};
      use crate::capture::{CaptureError, CaptureSource};
      use crate::frame::Frame;
      use crate::publish::{PublishError, TexturePublisher};
      use crate::transform::{self, Rotation, TransformConfig};

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
              self.frames.pop_front().ok_or_else(|| CaptureError::FrameRead {
                  reason: "exhausted".to_string(),
              })
          }
      }

      struct CollectingPublisher {
          published: Vec<Frame>,
          stop_after: Option<usize>,
          stop: Arc<AtomicBool>,
      }

      impl CollectingPublisher {
          fn new(stop_after: Option<usize>, stop: Arc<AtomicBool>) -> Self {
              Self { published: Vec::new(), stop_after, stop }
          }
      }

      impl TexturePublisher for CollectingPublisher {
          fn publish(&mut self, frame: &Frame) -> Result<(), PublishError> {
              self.published.push(frame.clone());
              if self.stop_after == Some(self.published.len()) {
                  self.stop.store(true, Ordering::SeqCst);
              }

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
          // 2 wide x 3 tall, every pixel a unique BGRA value, row-major.
          let data = vec![
              10, 20, 30, 255, 40, 50, 60, 255, // row 0
              70, 80, 90, 255, 100, 110, 120, 255, // row 1
              130, 140, 150, 255, 160, 170, 180, 255, // row 2
          ];

          Frame::new(2, 3, data).expect("valid frame")
      }

      #[test]
      fn applies_transform_before_publish() {
          let frame = asymmetric_frame();
          let config = TransformConfig { rotation: Rotation::R90, ..TransformConfig::default() };
          let expected = transform::apply(&frame, &config).expect("apply succeeds");
          let mut source = FakeSource::new(vec![frame]);
          let stop = Arc::new(AtomicBool::new(false));
          let mut publisher = CollectingPublisher::new(Some(1), Arc::clone(&stop));

          let result = run_pipeline(&mut source, &config, &mut publisher, &stop);

          assert!(result.is_ok());
          assert_eq!(publisher.published.len(), 1);
          assert_eq!(publisher.published[0], expected);
          assert_eq!(publisher.published[0].width(), 3);
          assert_eq!(publisher.published[0].height(), 2);
      }

      #[test]
      fn stop_flag_ends_loop_with_ok() {
          let pixel = Frame::new(1, 1, vec![0, 0, 0, 255]).expect("valid frame");
          let frames = vec![pixel.clone(), pixel.clone(), pixel];
          let mut source = FakeSource::new(frames);
          let config = TransformConfig::default();
          let stop = Arc::new(AtomicBool::new(false));
          let mut publisher = CollectingPublisher::new(Some(2), Arc::clone(&stop));

          // 3 frames are available but stop_after=2 must end the loop before the
          // 3rd next_frame() call — if run_pipeline ignored `stop` this would
          // instead exhaust FakeSource and return an Err.
          let result = run_pipeline(&mut source, &config, &mut publisher, &stop);

          assert!(result.is_ok());
          assert_eq!(publisher.published.len(), 2);
      }

      #[test]
      fn capture_error_propagates() {
          let mut source = FakeSource::new(vec![]);
          let config = TransformConfig::default();
          let stop = Arc::new(AtomicBool::new(false));
          let mut publisher = CollectingPublisher::new(None, Arc::clone(&stop));

          let result = run_pipeline(&mut source, &config, &mut publisher, &stop);

          assert!(matches!(
              result,
              Err(PipelineError::Capture(CaptureError::FrameRead { .. }))
          ));
      }

      #[test]
      fn publish_error_propagates() {
          let pixel = Frame::new(1, 1, vec![0, 0, 0, 255]).expect("valid frame");
          let mut source = FakeSource::new(vec![pixel]);
          let config = TransformConfig::default();
          let stop = Arc::new(AtomicBool::new(false));
          let mut publisher = FailingPublisher;

          let result = run_pipeline(&mut source, &config, &mut publisher, &stop);

          assert!(matches!(
              result,
              Err(PipelineError::Publish(PublishError::Publish { .. }))
          ));
      }
  }
  ```

- [ ] Run: `cargo test -p webcam-sharedtexture-core --lib pipeline`
  Expected FAIL (compile error):
  ```
  error[E0432]: unresolved import `super::run_pipeline`
   --> crates/core/src/pipeline.rs:6:16
    |
  6 |     use super::{run_pipeline, PipelineError};
    |                 ^^^^^^^^^^^^ no `run_pipeline` in the root
  ```
- [ ] Add the minimal implementation above the `#[cfg(test)]` module in `pipeline.rs`:

  ```rust
  use std::sync::atomic::{AtomicBool, Ordering};

  use thiserror::Error;

  use crate::capture::{CaptureError, CaptureSource};
  use crate::publish::{PublishError, TexturePublisher};
  use crate::transform::{self, TransformConfig, TransformError};

  #[derive(Debug, Error)]
  pub enum PipelineError {
      #[error(transparent)]
      Capture(#[from] CaptureError),
      #[error(transparent)]
      Transform(#[from] TransformError),
      #[error(transparent)]
      Publish(#[from] PublishError),
  }

  /// Loops: next_frame → apply(config) → publish, until `stop` is true
  /// (checked at the top of every iteration). Any step's error aborts the
  /// loop and propagates.
  pub fn run_pipeline(
      source: &mut dyn CaptureSource,
      config: &TransformConfig,
      publisher: &mut dyn TexturePublisher,
      stop: &AtomicBool,
  ) -> Result<(), PipelineError> {
      while !stop.load(Ordering::SeqCst) {
          let frame = source.next_frame()?;
          let transformed = transform::apply(&frame, config)?;
          publisher.publish(&transformed)?;
      }

      Ok(())
  }
  ```

- [ ] Run: `cargo test -p webcam-sharedtexture-core --lib pipeline`
  Expected PASS:
  ```
  running 4 tests
  test pipeline::tests::applies_transform_before_publish ... ok
  test pipeline::tests::capture_error_propagates ... ok
  test pipeline::tests::publish_error_propagates ... ok
  test pipeline::tests::stop_flag_ends_loop_with_ok ... ok

  test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
  ```
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`
- [ ] Commit:
  ```
  git add crates/core/src/pipeline.rs crates/core/src/lib.rs
  git commit -m "feat(core): add run_pipeline orchestrating capture, transform, publish"
  ```

---

### Task 9: nokhwa capture

**Files:**
- Modify: `crates/core/src/capture.rs` (append `list_devices`, `NokhwaSource`, `impl CaptureSource for NokhwaSource`, and private helpers)
- Modify: `crates/core/Cargo.toml` (add `nokhwa` dependency, feature `input-native`)
- Test: inline `#[cfg(test)] mod tests` additions in `capture.rs`; new integration test file `crates/core/tests/camera_smoke.rs`

**Interfaces:**
- Consumes: `CaptureSource`, `CaptureError`, `DeviceInfo` (Task 7, same file), `Frame::new` (Task 1).
- Produces (for `crates/cli` device-selection and pipeline wiring in later tasks, and `crates/gui`):
  - `pub fn list_devices() -> Result<Vec<DeviceInfo>, CaptureError>`
  - `pub struct NokhwaSource { /* private */ }` with `pub fn open(index: u32, requested_fps: Option<u32>) -> Result<Self, CaptureError>`
  - `impl CaptureSource for NokhwaSource`

**Verified nokhwa API (checked against docs.rs, current published version 0.10.11 at plan-writing time — re-run the dry-run below before pinning, since the version may have moved on by implementation time):**

```rust
// crate root
pub fn query(api: ApiBackend) -> Result<Vec<CameraInfo>, NokhwaError>;

// nokhwa::utils
pub enum CameraIndex { Index(u32), String(String) }
impl CameraIndex { pub fn as_index(&self) -> Result<u32, NokhwaError>; }

pub struct CameraInfo { /* ... */ }
impl CameraInfo {
    pub fn new(human_name: &str, description: &str, misc: &str, index: CameraIndex) -> CameraInfo;
    pub fn human_name(&self) -> String;
    pub fn index(&self) -> &CameraIndex;
}

pub enum RequestedFormatType {
    AbsoluteHighestResolution, AbsoluteHighestFrameRate,
    HighestResolution(Resolution), HighestFrameRate(u32),
    Exact(CameraFormat), Closest(CameraFormat), None,
}
pub struct RequestedFormat<'a> { /* ... */ }
impl<'a> RequestedFormat<'a> {
    pub fn new<F: FormatDecoder>(format_type: RequestedFormatType) -> Self;
}

// nokhwa (root)
pub struct Camera { /* ... */ }
impl Camera {
    pub fn new(index: CameraIndex, format: RequestedFormat<'_>) -> Result<Self, NokhwaError>;
    pub fn open_stream(&mut self) -> Result<(), NokhwaError>;
    pub fn frame(&mut self) -> Result<Buffer, NokhwaError>;
}

// nokhwa::buffer
pub struct Buffer { /* ... */ }
impl Buffer {
    pub fn decode_image<F: FormatDecoder>(&self) -> Result<ImageBuffer<F::Output, Vec<u8>>, NokhwaError>;
}
// decode_image::<RgbFormat> yields image::ImageBuffer<Rgb<u8>, Vec<u8>>, which has
// .as_raw() -> &Vec<u8> (tightly packed RGB8) and .width()/.height() -> u32.

// nokhwa::pixel_format
pub struct RgbFormat;

// nokhwa::error
pub enum NokhwaError {
    UnitializedError,
    InitializeError { backend: ApiBackend, error: String },
    ShutdownError { backend: ApiBackend, error: String },
    GeneralError(String),
    StructureError { structure: String, error: String },
    OpenDeviceError(String, String),
    GetPropertyError { property: String, error: String },
    SetPropertyError { property: String, value: String, error: String },
    OpenStreamError(String),
    ReadFrameError(String),
    ProcessFrameError { src: FrameFormat, destination: String, error: String },
    StreamShutdownError(String),
    UnsupportedOperationError(ApiBackend),
    NotImplementedError(String),
}
```

If any of these exact shapes have drifted (check via `cargo doc -p nokhwa --open` after adding the dependency), adjust the pure helper functions below accordingly before wiring `list_devices`/`NokhwaSource` — the unit tests for those helpers will catch a mismatch immediately as compile errors.

The design splits this task into pure, hardware-free helper functions (each gets a real TDD cycle with a real assertion) and thin hardware-touching wrappers around `nokhwa::query` / `Camera` (which cannot run in CI without a camera, so they get an `#[ignore]`d integration test plus a manual verification step instead of a red/green cycle).

#### Step 9.0 — add the dependency

- [ ] Run: `cargo add --dry-run nokhwa --features input-native -p webcam-sharedtexture-core`
  Confirm the resolved version line (expect `nokhwa v0.10.x`); if a newer 0.10.x or a breaking 0.x bump is reported, re-check the API shapes above against `https://docs.rs/nokhwa/<resolved-version>/nokhwa/` before continuing.
- [ ] Run for real: `cargo add nokhwa --features input-native -p webcam-sharedtexture-core`
- [ ] Commit:
  ```
  git add crates/core/Cargo.toml Cargo.lock
  git commit -m "chore(core): add nokhwa dependency for camera capture"
  ```

#### Cycle 9.1 — `rgb_to_bgra` (pure, exact pixel assertions)

- [ ] Add to the existing `#[cfg(test)] mod tests` block in `capture.rs` (RED step — `rgb_to_bgra` does not exist yet):

  ```rust
  #[test]
  fn rgb_to_bgra_swizzles_channels_and_adds_opaque_alpha() {
      let rgb = vec![
          10, 20, 30, // pixel (0,0): R=10 G=20 B=30
          40, 50, 60, // pixel (1,0): R=40 G=50 B=60
      ];

      let bgra = rgb_to_bgra(&rgb, 2, 1);

      assert_eq!(bgra, vec![30, 20, 10, 255, 60, 50, 40, 255]);
  }
  ```

  (Add `use super::rgb_to_bgra;` to the test module's existing `use super::{...}` line.)

- [ ] Run: `cargo test -p webcam-sharedtexture-core --lib capture::tests::rgb_to_bgra`
  Expected FAIL:
  ```
  error[E0425]: cannot find function `rgb_to_bgra` in this scope
  ```
- [ ] Add the minimal implementation to `capture.rs` (outside the test module):

  ```rust
  /// Converts tightly-packed RGB8 to tightly-packed BGRA8 with opaque alpha.
  fn rgb_to_bgra(rgb: &[u8], width: u32, height: u32) -> Vec<u8> {
      let pixel_count = usize::try_from(width)
          .unwrap_or(0)
          .saturating_mul(usize::try_from(height).unwrap_or(0));
      let mut bgra = Vec::with_capacity(pixel_count.saturating_mul(4));

      for pixel in rgb.chunks_exact(3) {
          bgra.extend_from_slice(&[pixel[2], pixel[1], pixel[0], 255]);
      }

      bgra
  }
  ```

- [ ] Run: `cargo test -p webcam-sharedtexture-core --lib capture::tests::rgb_to_bgra`
  Expected PASS:
  ```
  running 1 test
  test capture::tests::rgb_to_bgra_swizzles_channels_and_adds_opaque_alpha ... ok
  ```
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`
- [ ] Commit:
  ```
  git add crates/core/src/capture.rs
  git commit -m "feat(core): add rgb_to_bgra pixel swizzle helper"
  ```

#### Cycle 9.2 — pure device-listing helpers (`index_number`, `to_device_info`, `devices_from`)

These build `DeviceInfo` values from `nokhwa::utils::CameraInfo`/`CameraIndex`, both of which are plain constructible structs/enums — no hardware access needed to unit test them.

- [ ] Add to `capture.rs`'s test module (RED step):

  ```rust
  #[test]
  fn index_number_reads_numeric_index() {
      let index = nokhwa::utils::CameraIndex::Index(3);

      assert_eq!(index_number(&index), 3);
  }

  #[test]
  fn index_number_falls_back_to_zero_for_non_numeric_index() {
      let index = nokhwa::utils::CameraIndex::String("ipcam-1".to_string());

      assert_eq!(index_number(&index), 0);
  }

  #[test]
  fn to_device_info_copies_index_and_name() {
      let info = nokhwa::utils::CameraInfo::new(
          "Logi C920",
          "USB Video Class",
          "",
          nokhwa::utils::CameraIndex::Index(1),
      );

      let device = to_device_info(&info);

      assert_eq!(device, DeviceInfo { index: 1, name: "Logi C920".to_string() });
  }

  #[test]
  fn devices_from_empty_list_is_no_devices() {
      let result = devices_from(vec![]);

      assert!(matches!(result, Err(CaptureError::NoDevices)));
  }

  #[test]
  fn devices_from_maps_every_entry() {
      let infos = vec![
          nokhwa::utils::CameraInfo::new("Cam A", "", "", nokhwa::utils::CameraIndex::Index(0)),
          nokhwa::utils::CameraInfo::new("Cam B", "", "", nokhwa::utils::CameraIndex::Index(1)),
      ];

      let devices = devices_from(infos).expect("non-empty list maps");

      assert_eq!(
          devices,
          vec![
              DeviceInfo { index: 0, name: "Cam A".to_string() },
              DeviceInfo { index: 1, name: "Cam B".to_string() },
          ]
      );
  }
  ```

- [ ] Run: `cargo test -p webcam-sharedtexture-core --lib capture`
  Expected FAIL:
  ```
  error[E0425]: cannot find function `index_number` in this scope
  error[E0425]: cannot find function `to_device_info` in this scope
  error[E0425]: cannot find function `devices_from` in this scope
  ```
- [ ] Add the minimal implementation to `capture.rs`:

  ```rust
  use nokhwa::utils::{CameraIndex, CameraInfo};

  fn index_number(index: &CameraIndex) -> u32 {
      index.as_index().unwrap_or(0)
  }

  fn to_device_info(info: &CameraInfo) -> DeviceInfo {
      DeviceInfo { index: index_number(info.index()), name: info.human_name() }
  }

  fn devices_from(infos: Vec<CameraInfo>) -> Result<Vec<DeviceInfo>, CaptureError> {
      if infos.is_empty() {
          return Err(CaptureError::NoDevices);
      }

      Ok(infos.iter().map(to_device_info).collect())
  }
  ```

- [ ] Run: `cargo test -p webcam-sharedtexture-core --lib capture`
  Expected PASS: all previously-passing tests plus the 5 new ones report `ok`.
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`
- [ ] Commit:
  ```
  git add crates/core/src/capture.rs
  git commit -m "feat(core): add pure device-listing helpers for nokhwa CameraInfo"
  ```

#### Cycle 9.3 — pure error-mapping and format-request helpers

- [ ] Add to `capture.rs`'s test module (RED step):

  ```rust
  #[test]
  fn open_failed_wraps_nokhwa_error_text() {
      let error = nokhwa::NokhwaError::OpenDeviceError(
          "0".to_string(),
          "device busy".to_string(),
      );

      let mapped = open_failed(0, error);

      assert!(matches!(mapped, CaptureError::OpenFailed { index: 0, .. }));
  }

  #[test]
  fn frame_read_failed_wraps_nokhwa_error_text() {
      let error = nokhwa::NokhwaError::ReadFrameError("timeout".to_string());

      let mapped = frame_read_failed(error);

      assert!(matches!(mapped, CaptureError::FrameRead { .. }));
  }

  #[test]
  fn requested_format_type_uses_absolute_highest_frame_rate_by_default() {
      let format_type = requested_format_type(None);

      assert!(matches!(
          format_type,
          nokhwa::utils::RequestedFormatType::AbsoluteHighestFrameRate
      ));
  }

  #[test]
  fn requested_format_type_honors_requested_fps() {
      let format_type = requested_format_type(Some(30));

      assert!(matches!(
          format_type,
          nokhwa::utils::RequestedFormatType::HighestFrameRate(30)
      ));
  }
  ```

- [ ] Run: `cargo test -p webcam-sharedtexture-core --lib capture`
  Expected FAIL:
  ```
  error[E0425]: cannot find function `open_failed` in this scope
  error[E0425]: cannot find function `frame_read_failed` in this scope
  error[E0425]: cannot find function `requested_format_type` in this scope
  ```
- [ ] Add the minimal implementation to `capture.rs`:

  ```rust
  use nokhwa::utils::RequestedFormatType;
  use nokhwa::NokhwaError;

  fn open_failed(index: u32, error: NokhwaError) -> CaptureError {
      CaptureError::OpenFailed { index, reason: error.to_string() }
  }

  fn frame_read_failed(error: NokhwaError) -> CaptureError {
      CaptureError::FrameRead { reason: error.to_string() }
  }

  fn requested_format_type(requested_fps: Option<u32>) -> RequestedFormatType {
      match requested_fps {
          Some(fps) => RequestedFormatType::HighestFrameRate(fps),
          None => RequestedFormatType::AbsoluteHighestFrameRate,
      }
  }
  ```

- [ ] Run: `cargo test -p webcam-sharedtexture-core --lib capture`
  Expected PASS: all tests report `ok`, including the 4 new ones.
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`
- [ ] Commit:
  ```
  git add crates/core/src/capture.rs
  git commit -m "feat(core): add nokhwa error-mapping and format-request helpers"
  ```

#### Cycle 9.4 — wire `list_devices` and `NokhwaSource` (no automated red/green; ignored integration test + manual verification)

`nokhwa::query` and `Camera` require real OS camera APIs, so this step cannot be driven by a failing unit test in CI. Instead: write the implementation using only the already-tested pure helpers, confirm it compiles and the whole workspace still passes its existing (non-hardware) tests, and add an `#[ignore]`d integration test that a developer runs by hand on a machine with a camera.

- [ ] Create `crates/core/tests/camera_smoke.rs`:

  ```rust
  //! Manual verification only — exercises real camera hardware and is
  //! excluded from the default `cargo test` run. Run explicitly with:
  //!   cargo test -p webcam-sharedtexture-core --test camera_smoke -- --ignored
  use webcam_sharedtexture_core::capture::{list_devices, CaptureSource, NokhwaSource};

  #[test]
  #[ignore = "requires physical camera hardware"]
  fn lists_devices_and_grabs_one_frame() {
      let devices = list_devices().expect("at least one camera connected");
      assert!(!devices.is_empty());

      let mut source = NokhwaSource::open(devices[0].index, None).expect("camera opens");
      let frame = source.next_frame().expect("frame captured");

      assert!(frame.width() > 0);
      assert!(frame.height() > 0);
  }
  ```

- [ ] Add the implementation to `capture.rs`:

  ```rust
  use nokhwa::pixel_format::RgbFormat;
  use nokhwa::utils::{ApiBackend, RequestedFormat};
  use nokhwa::Camera;

  pub fn list_devices() -> Result<Vec<DeviceInfo>, CaptureError> {
      let infos = nokhwa::query(ApiBackend::Auto).map_err(|_error| CaptureError::NoDevices)?;

      devices_from(infos)
  }

  pub struct NokhwaSource {
      camera: Camera,
  }

  impl NokhwaSource {
      pub fn open(index: u32, requested_fps: Option<u32>) -> Result<Self, CaptureError> {
          let format_type = requested_format_type(requested_fps);
          let requested = RequestedFormat::new::<RgbFormat>(format_type);
          let mut camera = Camera::new(CameraIndex::Index(index), requested)
              .map_err(|error| open_failed(index, error))?;
          camera.open_stream().map_err(|error| open_failed(index, error))?;

          Ok(Self { camera })
      }
  }

  impl CaptureSource for NokhwaSource {
      fn next_frame(&mut self) -> Result<Frame, CaptureError> {
          let buffer = self.camera.frame().map_err(frame_read_failed)?;
          let decoded = buffer.decode_image::<RgbFormat>().map_err(frame_read_failed)?;
          let width = decoded.width();
          let height = decoded.height();
          let bgra = rgb_to_bgra(decoded.as_raw(), width, height);

          Frame::new(width, height, bgra)
              .map_err(|error| CaptureError::FrameRead { reason: error.to_string() })
      }
  }
  ```

  (Add `use crate::frame::Frame;` if not already imported at the top of `capture.rs` from Task 7.)

- [ ] Run: `cargo build -p webcam-sharedtexture-core --tests`
  Expected: clean build, 0 errors (this confirms the ignored test and the new code compile; it does not run `camera_smoke`).
- [ ] Run: `cargo test -p webcam-sharedtexture-core` (default set, hardware test excluded by `#[ignore]`)
  Expected PASS: every test from Cycles 7.1–9.3 still reports `ok`; `camera_smoke::lists_devices_and_grabs_one_frame` reports `ignored`.
- [ ] Lint gate: `cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check`
- [ ] Commit:
  ```
  git add crates/core/src/capture.rs crates/core/tests/camera_smoke.rs
  git commit -m "feat(core): implement list_devices and NokhwaSource via nokhwa"
  ```
- [ ] **Manual verification (not part of the automated gate, run on a machine with a physical camera attached):**
  ```
  cargo test -p webcam-sharedtexture-core --test camera_smoke -- --ignored --nocapture
  ```
  Confirm the test passes and note the reported frame width/height in the PR description or task log; if it fails, capture the exact `NokhwaError` text before changing any mapping logic (the pure helpers in Cycle 9.3 are the only translation layer and should not need to change unless the nokhwa API itself has drifted from the shapes verified above).
### Task 10: `crates/syphon` (Syphon Metal publisher, macOS)

New workspace member `crates/syphon` (package `webcam-sharedtexture-syphon`), implementing
`SyphonPublisher` per `INTERFACE-CONTRACT.md`. This is a sender-only trim of
`naporin0624/electron-texture-bridge`'s `packages/native` Syphon Metal bridge: only
`create` / `send_rgba` / `destroy` survive the port — receiver, discovery, IOSurface-handle
sends (`syphon_bridge_send` / `syphon_bridge_send_surface`), and all napi glue are dropped.

#### Deviations from the reference implementation (read before implementing)

The reference's C ABI does **not** literally match `INTERFACE-CONTRACT.md`'s Bridge C ABI —
these are deliberate corrections made while porting, not errors in this plan:

1. **Handle type.** Reference: `typedef void* SyphonBridgeHandle;` (a bare opaque `void*`,
   cast internally with `static_cast<SyphonBridge*>`). Contract (and this plan): a real
   opaque struct — `typedef struct SyphonBridgeHandle SyphonBridgeHandle;` declared in the
   header, defined only in the `.mm`. This is strictly safer (the Rust side gets a distinct
   pointer type instead of an untyped `*mut c_void`, so the compiler catches
   handle/IOSurface-pointer mixups) and costs nothing.
2. **`send_rgba` return type.** Reference returns `int` (`0` success / `-1` error). Contract
   specifies `bool`. This plan's `.mm` returns `bool` directly — C `_Bool`/C++ `bool` and
   Rust `bool` are ABI-compatible for `extern "C"`, so no translation layer is needed.
3. **No `__bridge_transfer` in `destroy`.** The task brief anticipated needing
   `__bridge_transfer` to release the handle under ARC. That pattern applies when a C `void*`
   is a *disguised* ARC object pointer (`CFBridgingRetain`/`CFBridgingRelease` pairs). Here
   the opaque handle is a plain heap-allocated C++ struct (`new SyphonBridgeHandle{...}`,
   freed with `delete`) whose fields happen to be ARC-qualified `id<...>` types. Under
   `-fobjc-arc`, the compiler synthesizes retain/release calls for those fields automatically
   as part of the struct's (non-trivial) constructor/destructor — exactly the pattern the
   reference's own `SyphonBridge`/`syphon_bridge_destroy` already uses. No `__bridge` cast is
   involved anywhere in the lifecycle; only `(__bridge CFDictionaryRef)` for the one-line
   `NSDictionary*` → `CFDictionaryRef` cast needed by `IOSurfaceCreate`, ported unchanged from
   the reference.
4. **No `#[allow(clippy::as_conversions)]` needed.** The contract flags this as
   conditionally necessary "if any cast is needed" for pointer casts in FFI. It isn't needed
   here: every value crossing the boundary already has matching width on both sides
   (`u32`↔`u32`, `*const u8`↔`*const uint8_t`, `bool`↔`bool`), and `NonNull::new` /
   `.as_ptr()` require no `as`. This plan's `ffi.rs` and `lib.rs` contain zero `as` casts —
   confirmed step by step below.
5. **Edition 2024 `unsafe extern` blocks.** The reference (older edition) declares
   `extern "C" { ... }` directly. Workspace edition is 2024 (per contract), where FFI
   declaration blocks must be written `unsafe extern "C" { ... }` (RFC 3484). `ffi.rs` below
   uses the 2024 syntax; get this wrong and the crate fails to compile with an edition
   migration error, not a subtle bug — worth calling out explicitly since the reference
   predates this requirement and copying it verbatim would not compile.

Everything else (IOSurface-create → lock → row-wise `memcpy` → `newTextureWithDescriptor:
iosurface:plane:0` → `publishFrameTexture:onCommandBuffer:imageRegion:flipped:YES` → async
`commit`; Metal device/queue created once in `create`; BGRA8 throughout) is ported faithfully.

---

#### 10.1 Vendor Syphon.framework

- [ ] Add the Syphon Framework source as a git submodule at the workspace root:

  ```bash
  git submodule add https://github.com/naporin0624/Syphon-Framework vendor/syphon-src
  ```

  Expected: `.gitmodules` created/updated at the repo root with:

  ```
  [submodule "vendor/syphon-src"]
  	path = vendor/syphon-src
  	url = https://github.com/naporin0624/Syphon-Framework
  ```

- [ ] Build the framework (exact command, copied from the reference repo's README
  "macOS: Build Syphon Framework" section, paths adjusted — this project vendors at the repo
  root, not under `packages/native/`):

  ```bash
  cd vendor/syphon-src
  xcodebuild -project Syphon.xcodeproj \
    -scheme Syphon \
    -configuration Release \
    -derivedDataPath build \
    ONLY_ACTIVE_ARCH=NO \
    BUILD_LIBRARY_FOR_DISTRIBUTION=YES
  cp -R build/Build/Products/Release/Syphon.framework ../Syphon.framework
  cd ../..
  ```

  Expected: `xcodebuild` ends with `** BUILD SUCCEEDED **`; afterward
  `vendor/Syphon.framework/Versions/Current/Syphon` exists (verify with
  `test -f vendor/Syphon.framework/Versions/Current/Syphon && echo OK`).

- [ ] If macOS Gatekeeper quarantines the copied framework (seen when `vendor/syphon-src` was
  itself downloaded rather than freshly built), clear it:

  ```bash
  xattr -dr com.apple.quarantine vendor/Syphon.framework
  ```

- [ ] Add to the repo-root `.gitignore` (the built framework and Xcode's derived-data output
  are build artifacts, not source — only the submodule *pointer* is tracked, via
  `.gitmodules`):

  ```gitignore
  # Syphon.framework — built locally from the vendor/syphon-src submodule (see README)
  /vendor/Syphon.framework/
  /vendor/syphon-src/build/
  ```

- [ ] Create `THIRD-PARTY-NOTICES` at the repo root (or append to it if a prior task already
  created one). Copy the Syphon Framework BSD license block **verbatim** from the reference
  repo — do not retype it by hand:

  ```bash
  gh api repos/naporin0624/electron-texture-bridge/contents/packages/native/THIRD-PARTY-NOTICES \
    -q .content | base64 -d
  ```

  Take only the "Syphon Framework" section (the reference file also carries a Spout2 license
  block this project does not need yet, since Spout is trait-only per the design doc). The
  section to copy into this repo's `THIRD-PARTY-NOTICES`:

  ```
  THIRD-PARTY SOFTWARE NOTICES AND INFORMATION

  This package incorporates components from the projects listed below.

  ================================================================================

  Syphon Framework
  https://github.com/Syphon/Syphon-Framework

  Copyright 2010 bangnoise (Tom Butterworth) & vade (Anton Marini).
  All rights reserved.

  Redistribution and use in source and binary forms, with or without
  modification, are permitted provided that the following conditions are met:

  * Redistributions of source code must retain the above copyright
  notice, this list of conditions and the following disclaimer.

  * Redistributions in binary form must reproduce the above copyright
  notice, this list of conditions and the following disclaimer in the
  documentation and/or other materials provided with the distribution.

  * Neither the name of the Syphon Project nor the names of its contributors
  may be used to endorse or promote products derived from this software
  without specific prior written permission.

  THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND
  ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED
  WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
  DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDERS BE LIABLE FOR ANY
  DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
  (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;
  LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND
  ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
  (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
  SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
  ```

- [ ] Commit (see 10.7) — this checkpoint has no Rust code yet, so no test cycle applies.

#### 10.2 Crate scaffold

- [ ] Create `crates/syphon/Cargo.toml`:

  ```toml
  [package]
  name = "webcam-sharedtexture-syphon"
  version = "0.1.0"
  edition = "2024"
  publish = false

  [lints]
  workspace = true

  [dependencies]
  webcam-sharedtexture-core = { path = "../core" }

  [build-dependencies]
  cc = "1"
  ```

  Before pinning, verify the current `cc` major version is still `1.x`:

  ```bash
  cargo add --dry-run cc -p webcam-sharedtexture-syphon --build
  ```

- [ ] Add `"crates/syphon"` to the workspace root `Cargo.toml` `members` list (alongside
  `"crates/core"`, `"crates/cli"`, `"crates/gui"` from earlier tasks):

  ```toml
  [workspace]
  members = ["crates/core", "crates/cli", "crates/gui", "crates/syphon"]
  resolver = "2"
  ```

- [ ] Create empty placeholders so the crate is buildable before real code lands:
  `crates/syphon/src/lib.rs` containing only `#![cfg(target_os = "macos")]` (see 10.5 for why
  — this makes the crate compile to nothing on non-macOS targets rather than failing to link),
  `crates/syphon/build.rs` containing only `fn main() {}`, and empty
  `crates/syphon/cpp/syphon_bridge.{h,mm}` files.

- [ ] Run:

  ```bash
  cargo build -p webcam-sharedtexture-syphon
  ```

  Expected: succeeds with no warnings (empty crate, empty no-op build script).

- [ ] Commit (10.7).

#### 10.3 The C header + ObjC++ bridge (`cpp/syphon_bridge.h`, `cpp/syphon_bridge.mm`)

This is the sender-only trim of the reference's `syphon_bridge.{h,mm}`: the struct now holds
only `device` / `commandQueue` / `server` (no receiver, no discovery, no pixel-format-mapping
helpers used only by the receiver path).

`crates/syphon/cpp/syphon_bridge.h` — full contents:

```c
#pragma once

#include <stdbool.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

// Opaque handle. The real struct (device + command queue + Syphon server) is
// defined only in syphon_bridge.mm — Rust and any other C caller only ever
// see a pointer to an incomplete type.
typedef struct SyphonBridgeHandle SyphonBridgeHandle;

// Creates a Metal-backed Syphon server advertised under `server_name`.
// Returns NULL on failure (no default Metal device, or SyphonMetalServer
// construction failed) — check Console.app for the corresponding NSLog.
SyphonBridgeHandle* syphon_bridge_create(const char* server_name);

// Publishes one BGRA8 frame. `pixels` is row-major, tightly packed or not —
// `bytes_per_row` tells the bridge the real stride so callers may pass
// exactly `width * 4` (tightly packed, as this project's core always does)
// without the bridge assuming it.
//
// Returns true on success, false on failure (invalid arguments, IOSurface
// allocation failure, or Metal texture creation failure).
bool syphon_bridge_send_rgba(SyphonBridgeHandle* handle,
                              const uint8_t* pixels,
                              uint32_t width,
                              uint32_t height,
                              uint32_t bytes_per_row);

// Stops the Syphon server and releases all resources. `handle` must not be
// used after this call. Safe to call with NULL (no-op).
void syphon_bridge_destroy(SyphonBridgeHandle* handle);

#ifdef __cplusplus
}
#endif
```

`crates/syphon/cpp/syphon_bridge.mm` — full contents:

```objc
#import "syphon_bridge.h"
#import <Metal/Metal.h>
#import <IOSurface/IOSurface.h>
#import <Syphon/Syphon.h>
#import <Foundation/Foundation.h>

// Real definition of the handle declared opaque in the header. ARC manages
// the three id<...>-typed fields: `-fobjc-arc` gives this C++ struct a
// non-trivial constructor/destructor pair that retains on construction and
// releases on destruction, the same way the reference bridge's `SyphonBridge`
// struct does. No manual retain/release or __bridge cast is needed for the
// handle itself — only IOSurfaceCreate's NSDictionary argument needs one
// (Core Foundation's CFDictionaryRef vs. Foundation's NSDictionary*).
struct SyphonBridgeHandle {
    id<MTLDevice>       device;
    id<MTLCommandQueue> commandQueue;
    SyphonMetalServer*  server;
};

extern "C" {

SyphonBridgeHandle* syphon_bridge_create(const char* server_name) {
    if (!server_name) {
        return nullptr;
    }
    @autoreleasepool {
        id<MTLDevice> device = MTLCreateSystemDefaultDevice();
        if (!device) {
            NSLog(@"[SyphonBridge] create: no default Metal device");
            return nullptr;
        }

        id<MTLCommandQueue> queue = [device newCommandQueue];
        if (!queue) {
            NSLog(@"[SyphonBridge] create: failed to create a command queue");
            return nullptr;
        }

        NSString* name = [NSString stringWithUTF8String:server_name];
        SyphonMetalServer* server = [[SyphonMetalServer alloc] initWithName:name
                                                                      device:device
                                                                     options:nil];
        if (!server) {
            NSLog(@"[SyphonBridge] create: failed to create SyphonMetalServer \"%@\"", name);
            return nullptr;
        }

        return new SyphonBridgeHandle{device, queue, server};
    }
}

bool syphon_bridge_send_rgba(SyphonBridgeHandle* handle,
                              const uint8_t* pixels,
                              uint32_t width,
                              uint32_t height,
                              uint32_t bytes_per_row) {
    if (!handle || !pixels || width == 0 || height == 0) {
        return false;
    }
    // Defend against a caller-declared stride narrower than one packed row —
    // the row-wise copy below would read past the end of `pixels`.
    if ((uint64_t)bytes_per_row < (uint64_t)width * 4) {
        return false;
    }

    @autoreleasepool {
        NSDictionary* surfaceProps = @{
            (NSString*)kIOSurfaceWidth: @(width),
            (NSString*)kIOSurfaceHeight: @(height),
            (NSString*)kIOSurfaceBytesPerElement: @4,
            (NSString*)kIOSurfaceBytesPerRow: @(bytes_per_row),
            (NSString*)kIOSurfacePixelFormat: @(kCVPixelFormatType_32BGRA),
            (NSString*)kIOSurfaceAllocSize: @((size_t)bytes_per_row * (size_t)height),
        };

        IOSurfaceRef surface = IOSurfaceCreate((__bridge CFDictionaryRef)surfaceProps);
        if (!surface) {
            NSLog(@"[SyphonBridge] send_rgba: IOSurfaceCreate failed (%ux%u)", width, height);
            return false;
        }

        IOSurfaceLock(surface, 0, nullptr);
        uint8_t* dstBase = static_cast<uint8_t*>(IOSurfaceGetBaseAddress(surface));
        size_t dstBytesPerRow = IOSurfaceGetBytesPerRow(surface);
        size_t copyWidth = (size_t)width * 4;

        // Row-wise copy: the IOSurface's actual stride (page/tile aligned by
        // the system) can differ from the caller's `bytes_per_row`, so a
        // single memcpy over the whole buffer would misalign every row past
        // the first.
        const uint8_t* srcRow = pixels;
        uint8_t* dstRow = dstBase;
        for (uint32_t row = 0; row < height; row++) {
            memcpy(dstRow, srcRow, copyWidth);
            srcRow += bytes_per_row;
            dstRow += dstBytesPerRow;
        }
        IOSurfaceUnlock(surface, 0, nullptr);

        MTLTextureDescriptor* desc =
            [MTLTextureDescriptor texture2DDescriptorWithPixelFormat:MTLPixelFormatBGRA8Unorm
                                                                width:width
                                                               height:height
                                                            mipmapped:NO];
        desc.usage = MTLTextureUsageShaderRead;
        desc.storageMode = MTLStorageModeShared;

        id<MTLTexture> texture = [handle->device newTextureWithDescriptor:desc
                                                                 iosurface:surface
                                                                     plane:0];
        CFRelease(surface);

        if (!texture) {
            NSLog(@"[SyphonBridge] send_rgba: failed to wrap IOSurface as MTLTexture");
            return false;
        }

        // Publish is fire-and-forget: we commit and return immediately.
        // `addCompletedHandler` only logs a GPU-side error asynchronously —
        // callers see success as soon as the command buffer is queued, same
        // as the reference bridge.
        id<MTLCommandBuffer> cmdBuf = [handle->commandQueue commandBuffer];
        [handle->server publishFrameTexture:texture
                            onCommandBuffer:cmdBuf
                                imageRegion:NSMakeRect(0, 0, width, height)
                                    flipped:YES];
        [cmdBuf addCompletedHandler:^(id<MTLCommandBuffer> completed) {
            if (completed.error) {
                NSLog(@"[SyphonBridge] send_rgba: publish command buffer error: %@", completed.error);
            }
        }];
        [cmdBuf commit];

        return true;
    }
}

void syphon_bridge_destroy(SyphonBridgeHandle* handle) {
    if (!handle) {
        return;
    }
    @autoreleasepool {
        [handle->server stop];
        delete handle; // runs the ARC-synthesized destructor for device/commandQueue/server
    }
}

} // extern "C"
```

- [ ] Commit (10.7) — no automated test targets ObjC++ directly; correctness here is checked
  by the Rust tests in 10.6 once the build script (10.4) compiles this file.

#### 10.4 `build.rs`

Macro-guarded to a genuine no-op on non-macOS (matches `lib.rs`'s `#![cfg(target_os =
"macos")]`, so `cargo build --workspace` never fails off-macOS even though this crate is only
meaningful there). Uses `Result`-returning `run()` + early-return guards rather than
`.unwrap()`/`.expect()`, consistent with the workspace's `unwrap_used`/`expect_used` clippy
denies (which also apply to the build-script compilation unit).

`crates/syphon/build.rs` — full contents:

```rust
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(reason) => {
            eprintln!("crates/syphon build.rs failed: {reason}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS")
        .map_err(|err| format!("CARGO_CFG_TARGET_OS is not set: {err}"))?;

    // Non-macOS builds compile the `#![cfg(target_os = "macos")]`-gated empty
    // crate (see src/lib.rs) — there is no native bridge to build.
    if target_os != "macos" {
        return Ok(());
    }

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map_err(|err| format!("CARGO_MANIFEST_DIR is not set: {err}"))?;
    let crate_dir = Path::new(&manifest_dir);
    let workspace_root = crate_dir
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| format!("{manifest_dir} has no workspace root two levels up"))?;
    let vendor_dir = workspace_root.join("vendor");
    let vendor_str = vendor_dir
        .to_str()
        .ok_or_else(|| format!("vendor path {} is not valid UTF-8", vendor_dir.display()))?;

    println!("cargo:rerun-if-changed=cpp/syphon_bridge.mm");
    println!("cargo:rerun-if-changed=cpp/syphon_bridge.h");
    println!("cargo:rerun-if-changed={vendor_str}/Syphon.framework");

    cc::Build::new()
        .file("cpp/syphon_bridge.mm")
        .include("cpp")
        .flag("-ObjC++")
        .flag("-std=c++17")
        .flag("-fobjc-arc")
        .flag("-F")
        .flag(vendor_str)
        .try_compile("syphon_bridge")
        .map_err(|err| format!("failed to compile cpp/syphon_bridge.mm: {err}"))?;

    // C++ runtime, required because syphon_bridge.mm is compiled as C++17.
    println!("cargo:rustc-link-lib=c++");

    println!("cargo:rustc-link-lib=framework=Syphon");
    println!("cargo:rustc-link-lib=framework=Metal");
    println!("cargo:rustc-link-lib=framework=IOSurface");
    println!("cargo:rustc-link-lib=framework=Cocoa");
    println!("cargo:rustc-link-lib=framework=QuartzCore");
    println!("cargo:rustc-link-search=framework={vendor_str}");

    // rpath so built binaries find vendor/Syphon.framework without a
    // system-wide install. `cargo build`/`test` binaries land at
    // target/debug/, target/debug/deps/, or — with an explicit --target
    // triple — one directory deeper; cover each depth back to the workspace
    // root so every binary kind resolves the same relative vendor/ path.
    for rel in [
        "@loader_path/../../vendor",
        "@loader_path/../../../vendor",
        "@loader_path/../../../../vendor",
    ] {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{rel}");
    }

    Ok(())
}
```

- [ ] Replace the placeholder `fn main() {}` from 10.2 with the above.
- [ ] Run:

  ```bash
  cargo build -p webcam-sharedtexture-syphon
  ```

  Expected (once 10.1's `vendor/Syphon.framework` exists and 10.3's `.mm` has real content):
  clean build, ending in the usual `Compiling webcam-sharedtexture-syphon v0.1.0 (...)` /
  `Finished` lines. If `vendor/Syphon.framework` is missing, expect a clear
  `ld: framework 'Syphon' not found` linker error — confirms the link flags are wired
  correctly even before the framework exists.
- [ ] Commit (10.7).

#### 10.5 `src/ffi.rs` + `src/lib.rs`

The only `unsafe` in the whole workspace lives here: the three FFI calls, each with a
`// SAFETY:` comment. No `as` cast appears anywhere in either file (see deviation #4 above).

`crates/syphon/src/ffi.rs` — full contents:

```rust
use std::os::raw::c_char;

/// Opaque handle to the native bridge (defined only in `cpp/syphon_bridge.mm`).
/// The zero-sized field is the standard pattern for an FFI opaque type: Rust
/// never constructs or reads through this struct, only holds pointers to it.
#[repr(C)]
pub struct SyphonBridgeHandle {
    _private: [u8; 0],
}

// Edition 2024 requires FFI declaration blocks to be written as `unsafe
// extern "C"` (RFC 3484) — every item inside remains individually unsafe to
// call, same as a pre-2024 bare `extern "C"` block.
unsafe extern "C" {
    pub fn syphon_bridge_create(server_name: *const c_char) -> *mut SyphonBridgeHandle;

    /// `pixels` must point to at least `bytes_per_row * height` readable,
    /// initialized bytes. The bridge copies them into its own `IOSurface`
    /// before returning and retains no pointer afterward.
    pub fn syphon_bridge_send_rgba(
        handle: *mut SyphonBridgeHandle,
        pixels: *const u8,
        width: u32,
        height: u32,
        bytes_per_row: u32,
    ) -> bool;

    pub fn syphon_bridge_destroy(handle: *mut SyphonBridgeHandle);
}
```

`crates/syphon/src/lib.rs` — full contents:

```rust
#![cfg(target_os = "macos")]

mod ffi;

use std::ffi::CString;
use std::ptr::NonNull;

use webcam_sharedtexture_core::frame::Frame;
use webcam_sharedtexture_core::publish::{PublishError, TexturePublisher};

/// Sender-only Syphon Metal publisher. Wraps the opaque bridge handle
/// returned by `syphon_bridge_create`.
pub struct SyphonPublisher {
    handle: NonNull<ffi::SyphonBridgeHandle>,
}

// SAFETY: `SyphonBridgeHandle` owns a `SyphonMetalServer`, an `MTLDevice`,
// and an `MTLCommandQueue`. `SyphonPublisher` is not `Clone` and exposes no
// way to obtain a second handle to the same native object, so moving one to
// another thread (e.g. handing it to a dedicated capture/publish thread)
// never creates concurrent access from two threads at once. This mirrors the
// reference bridge's own `unsafe impl Send for Sender`.
unsafe impl Send for SyphonPublisher {}

impl SyphonPublisher {
    /// Creates a new Syphon Metal server advertised under `server_name`.
    pub fn new(server_name: &str) -> Result<Self, PublishError> {
        let c_name = CString::new(server_name).map_err(|err| PublishError::ServerCreate {
            name: server_name.to_string(),
            reason: err.to_string(),
        })?;

        // SAFETY: `c_name` is a valid, NUL-terminated C string that stays
        // alive for the duration of this call (it is not dropped until this
        // statement completes). `syphon_bridge_create` copies the bytes into
        // an `NSString` before returning, so it never retains the pointer.
        let raw = unsafe { ffi::syphon_bridge_create(c_name.as_ptr()) };

        let handle = NonNull::new(raw).ok_or_else(|| PublishError::ServerCreate {
            name: server_name.to_string(),
            reason: "syphon_bridge_create returned a null handle".to_string(),
        })?;

        Ok(Self { handle })
    }
}

impl TexturePublisher for SyphonPublisher {
    fn publish(&mut self, frame: &Frame) -> Result<(), PublishError> {
        let bytes_per_row = frame
            .width()
            .checked_mul(4)
            .ok_or_else(|| PublishError::Publish {
                reason: format!(
                    "frame width {} overflows bytes-per-row (width * 4)",
                    frame.width()
                ),
            })?;

        // SAFETY: `self.handle` was created by `syphon_bridge_create` in
        // `new` and is not destroyed until `Drop::drop` runs (which takes
        // `&mut self`, so it cannot race with this `&mut self` call).
        // `frame.data()` is a valid `&[u8]` of exactly `width * height * 4`
        // bytes (a `Frame` invariant enforced by `Frame::new`), so
        // `bytes_per_row * height` never reads past its end. The bridge only
        // reads the buffer for the duration of this call — it copies pixels
        // into its own `IOSurface` before returning — so no aliasing or
        // use-after-free is possible once this call returns.
        let ok = unsafe {
            ffi::syphon_bridge_send_rgba(
                self.handle.as_ptr(),
                frame.data().as_ptr(),
                frame.width(),
                frame.height(),
                bytes_per_row,
            )
        };

        if ok {
            Ok(())
        } else {
            Err(PublishError::Publish {
                reason: "syphon_bridge_send_rgba returned false".to_string(),
            })
        }
    }
}

impl Drop for SyphonPublisher {
    fn drop(&mut self) {
        // SAFETY: `self.handle` is the handle created in `new` and has not
        // been destroyed yet — `drop` runs at most once per `SyphonPublisher`
        // and `SyphonPublisher` is not `Clone`, so no other reference to it
        // can be live concurrently.
        unsafe { ffi::syphon_bridge_destroy(self.handle.as_ptr()) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_rejects_interior_nul() {
        let result = SyphonPublisher::new("bad\0name");

        assert!(matches!(result, Err(PublishError::ServerCreate { .. })));
    }
}
```

TDD cycle for this checkpoint (the one genuinely re-runnable red/green cycle in this task —
everything above it is native code with no Rust test harness until the crate links):

- [ ] **Red.** Before writing `SyphonPublisher` at all, add just the test
  `new_rejects_interior_nul` (shown above) against a `lib.rs` that only has
  `#![cfg(target_os = "macos")]` and `mod ffi;` (ffi.rs from this step, already complete).
  Run:

  ```bash
  cargo test -p webcam-sharedtexture-syphon new_rejects_interior_nul
  ```

  Expected: **compile error** — `error[E0433]: failed to resolve: use of undeclared type
  \`SyphonPublisher\`` (or `cannot find type` / `cannot find function \`new\`` depending on
  exact staging). This is the "red" state.

- [ ] **Green (minimal).** Add just the `SyphonPublisher` struct, the `unsafe impl Send`, and
  `impl SyphonPublisher { pub fn new(...) }` (exactly as shown above — the `CString`
  validation is already the full, real implementation, not a stub). Do not yet add
  `TexturePublisher` or `Drop`. Run the same command again.

  Expected: **PASS** —

  ```
  running 1 test
  test tests::new_rejects_interior_nul ... ok
  ```

  (Leaking the handle on the success path at this intermediate stage is fine — no test
  exercises the success path without `Drop`, and `Drop` is added in the same commit before
  this checkpoint closes.)

- [ ] **Extend.** Add `impl TexturePublisher for SyphonPublisher` and `impl Drop for
  SyphonPublisher` (full code above). Run:

  ```bash
  cargo test -p webcam-sharedtexture-syphon
  ```

  Expected: still green (`new_rejects_interior_nul` unaffected; no new tests yet — the smoke
  test is added in 10.6).

- [ ] Run the lint gate now that real logic exists:

  ```bash
  cargo clippy -p webcam-sharedtexture-syphon --all-targets -- -D warnings
  cargo fmt -p webcam-sharedtexture-syphon -- --check
  ```

  Expected: no warnings. In particular, confirm clippy raises nothing about `as_conversions`
  (there are none to allow — see deviation #4) and nothing about `unwrap_used`/`expect_used`
  (none present).

- [ ] Commit (10.7).

#### 10.6 Smoke test + manual verification

A real `SyphonMetalServer` needs a GPU-backed macOS session (Metal device + a running
WindowServer) — not something to run unattended in a headless CI job, so it's `#[ignore]`d.
Local development still gets full coverage: run it manually whenever `cpp/syphon_bridge.mm`
or `build.rs` changes.

Add to `crates/syphon/src/lib.rs`'s `#[cfg(test)] mod tests`:

```rust
    #[test]
    #[ignore = "requires a real macOS GPU session; run manually with --ignored"]
    fn publish_one_solid_color_frame() {
        use webcam_sharedtexture_core::frame::Frame;

        let width = 64_u32;
        let height = 64_u32;
        let pixel = [0_u8, 0, 255, 255]; // solid red, BGRA
        let data = pixel
            .iter()
            .copied()
            .cycle()
            .take((width * height * 4) as usize)
            .collect();
        let frame = Frame::new(width, height, data).expect("valid frame");

        let mut publisher =
            SyphonPublisher::new("webcam-sharedtexture-smoke-test").expect("server create");

        publisher.publish(&frame).expect("publish");

        // Give a receiving app a moment to observe the frame before the
        // server is torn down by `publisher`'s Drop at the end of this test.
        std::thread::sleep(std::time::Duration::from_secs(5));
    }
```

(`.expect(...)` is allowed here — `clippy.toml`'s `allow-expect-in-tests = true` from Task 1
exempts test code from the workspace-wide `expect_used` deny; `(width * height * 4) as usize`
is likewise fine since it's test-only code, not the FFI module the contract's `as`-cast note
targets.)

- [ ] Add the test above.
- [ ] Run:

  ```bash
  cargo test -p webcam-sharedtexture-syphon -- --ignored --nocapture
  ```

  Expected: test runs for ~5s and reports `test tests::publish_one_solid_color_frame ... ok`.

- [ ] Manual verification checklist (do this once, by hand, to confirm the bridge actually
  reaches a Syphon client — not just that the Rust/ObjC++ call chain returns success):

  1. Install a Syphon client app if not already present — e.g. **Syphon Recorder** or
     **Simple Client** from the official example apps at
     <https://github.com/Syphon/Simple>, or any VJ app that lists Syphon servers
     (Resolume Arena, VDMX, TouchDesigner).
  2. Launch the client app and leave its server list visible.
  3. In a terminal, run:

     ```bash
     cargo test -p webcam-sharedtexture-syphon -- --ignored --nocapture publish_one_solid_color_frame
     ```

  4. Within the 5-second window, confirm a server named
     `webcam-sharedtexture-smoke-test` appears in the client's server list and, once
     selected, displays a solid red 64×64 frame.
  5. Confirm the server disappears from the list after the test process exits (proves
     `Drop`/`syphon_bridge_destroy` actually calls `[server stop]`).

- [ ] Commit (10.7).

#### 10.7 Commits + README documentation

Conventional commits, one per checkpoint above, none pushed:

```bash
git add .gitmodules vendor/syphon-src .gitignore THIRD-PARTY-NOTICES
git commit -m "chore(syphon): vendor Syphon Framework submodule and license notice"

git add Cargo.toml crates/syphon/Cargo.toml crates/syphon/src crates/syphon/build.rs crates/syphon/cpp
git commit -m "feat(syphon): scaffold webcam-sharedtexture-syphon crate"

git add crates/syphon/cpp/syphon_bridge.h crates/syphon/cpp/syphon_bridge.mm
git commit -m "feat(syphon): port sender-only Syphon Metal bridge from electron-texture-bridge"

git add crates/syphon/build.rs
git commit -m "feat(syphon): compile syphon_bridge.mm via cc and link Syphon/Metal/IOSurface"

git add crates/syphon/src/ffi.rs crates/syphon/src/lib.rs
git commit -m "test(syphon): add SyphonPublisher::new interior-NUL rejection test"

git add crates/syphon/src/lib.rs
git commit -m "feat(syphon): implement SyphonPublisher publish and drop over the FFI bridge"

git add crates/syphon/src/lib.rs
git commit -m "test(syphon): add ignored smoke test publishing a solid-color frame"

git add README.md
git commit -m "docs: document Syphon.framework build steps and vendor layout"
```

Do not push (per workspace constraints — commits stay local for the user to review).

- [ ] Add to the repo's `README.md` (create a "Building the Syphon bridge (macOS)" section if
  none exists yet from an earlier task):

  ```markdown
  ## Building the Syphon bridge (macOS)

  `crates/syphon` links against Apple's Syphon.framework, which is not vendored/prebuilt —
  it's built locally from a git submodule the first time you set up the repo.

  ```bash
  # 1. fetch the Syphon Framework source (once, or after a fresh clone without --recursive)
  git submodule update --init --recursive

  # 2. build Syphon.framework
  cd vendor/syphon-src
  xcodebuild -project Syphon.xcodeproj \
    -scheme Syphon \
    -configuration Release \
    -derivedDataPath build \
    ONLY_ACTIVE_ARCH=NO \
    BUILD_LIBRARY_FOR_DISTRIBUTION=YES
  cp -R build/Build/Products/Release/Syphon.framework ../Syphon.framework
  cd ../..

  # 3. build the workspace
  cargo build --workspace
  ```

  `vendor/syphon-src` (the submodule source) and `vendor/Syphon.framework` (the build output)
  are both gitignored except for the submodule pointer in `.gitmodules` — every clone rebuilds
  the framework locally rather than committing a binary.

  If Gatekeeper quarantines the copied framework and Syphon servers silently fail to appear to
  clients, clear it: `xattr -dr com.apple.quarantine vendor/Syphon.framework`.
  ```

- [ ] Final full-workspace check before moving to the next task:

  ```bash
  cargo test -p webcam-sharedtexture-syphon
  cargo clippy --workspace --all-targets -- -D warnings
  cargo fmt --all -- --check
  ```

  Expected: all green. `cargo test -p webcam-sharedtexture-syphon` runs only
  `new_rejects_interior_nul` by default (the smoke test stays `#[ignore]`d).
## Section D — Tasks 11–14 (CLI: args, selection, wiring, verification)

Scope: `crates/cli/`. Depends on Tasks 1–10 already landed (`webcam-sharedtexture-core` fully
implemented per the interface contract, `webcam-sharedtexture-syphon` added to the workspace).
All types/signatures below are exactly as fixed in `INTERFACE-CONTRACT.md`. Two additions to the
contract are introduced in Task 13 and are called out explicitly where they happen — see
"Contract additions" boxes.

Conventions used throughout this section:
- Every code change is TDD'd: write the failing test → run it (expect FAIL) → write minimal
  implementation → run it (expect PASS) → lint gate → commit.
- Lint gate (run after every task, before its final commit):
  ```
  cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check
  ```
- No `unwrap`/`expect` outside `#[cfg(test)]` code, no `_` wildcard arms on owned enums, no `as`
  casts, guard-style early returns (`let-else` + early `return`), function names ≤ 3 words.
- Commits are created but never pushed.

---

### Task 11: CLI argument parsing (`crates/cli/src/args.rs`)

### 11.0 Verify dependency versions

```
$ cargo add --dry-run clap --features derive -p webcam-sharedtexture-cli
    Updating crates.io index
      Adding clap v4.6.1 to dependencies
             Features:
             + color
             + derive
             + error-context
             + help
             + std
             + suggestions
             + usage
warning: aborting add due to dry run
```

Confirmed: **clap 4.6.x** with the `derive` feature. Pin `4.6` (caret range) in
`[workspace.dependencies]`.

- [ ] Run the `cargo add --dry-run` command above and confirm the resolved version before editing
      any `Cargo.toml`.

### 11.1 Wire up the dependency

- [ ] Edit root `Cargo.toml`: add `clap` to `[workspace.dependencies]` (create the table if Task 1
      has not already; `thiserror` should already be there from Task 1 — do not duplicate it):

  ```toml
  [workspace.dependencies]
  thiserror = "2"
  clap = { version = "4.6", features = ["derive"] }
  ```

- [ ] Edit `crates/cli/Cargo.toml`, add `clap` under `[dependencies]`:

  ```toml
  [dependencies]
  webcam-sharedtexture-core = { path = "../core" }
  clap = { workspace = true }
  ```

- [ ] Sanity build (no code yet, just confirms the dependency resolves):
  ```
  $ cargo build -p webcam-sharedtexture-cli
     Compiling clap_lex v0.7.x
     Compiling clap_builder v4.6.x
     Compiling clap_derive v4.6.x
     Compiling clap v4.6.x
     Compiling webcam-sharedtexture-cli v0.1.0 (.../crates/cli)
      Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.4s
  ```
- [ ] Commit:
  ```
  git add Cargo.toml crates/cli/Cargo.toml Cargo.lock
  git commit -m "build(cli): add clap with derive feature"
  ```

### 11.2 Create `crates/cli/src/args.rs` — RED

- [ ] Create `crates/cli/src/args.rs` with only the struct, imports, and empty parser stubs so
      the test module compiles against real signatures (test-first, but the parser bodies are the
      part under TDD — start each parser at `unimplemented!()` is disallowed by the no-`unwrap`/
      `expect`/panic-in-prod rule, so instead go straight to writing the failing **test module**
      against the *not-yet-existing* functions; the crate simply fails to compile, which is the
      RED state for this step):

  ```rust
  //! CLI argument definition and value parsing.
  //!
  //! Value parsers here reject anything the core `transform` module cannot represent
  //! (e.g. non-90-degree rotations) before a `TransformConfig` is ever built, so `core`
  //! never has to handle CLI-shaped invalid input.

  use clap::Parser;
  use webcam_sharedtexture_core::transform::{CropRect, Flip, Rotation, ScaleSpec, TransformConfig};

  #[derive(Debug, Parser)]
  #[command(name = "webcam-sharedtexture")]
  pub struct Args {
      /// Camera device index; omit to select interactively (TTY only)
      pub device: Option<u32>,

      #[arg(long)]
      pub list_devices: bool,

      #[arg(long, default_value = "0", value_parser = parse_rotation)]
      pub rotate: Rotation,

      #[arg(long, value_parser = parse_flip)]
      pub flip: Option<Flip>,

      #[arg(long, value_parser = parse_crop)]
      pub crop: Option<CropRect>,

      #[arg(long, value_parser = parse_scale)]
      pub scale: Option<ScaleSpec>,

      #[arg(long, default_value = "webcam-sharedtexture")]
      pub server_name: String,

      #[arg(long)]
      pub fps: Option<u32>,
  }

  impl Args {
      pub fn transform_config(&self) -> TransformConfig {
          TransformConfig {
              crop: self.crop,
              rotation: self.rotate,
              flip: self.flip.unwrap_or_default(),
              scale: self.scale,
          }
      }
  }
  ```

  (No `parse_*` function bodies yet — leave them undeclared so the test module below fails to
  compile. That absence of a function is the RED signal for this sub-step.)

- [ ] Append the **failing** test module (functions referenced do not exist yet):

  ```rust
  #[cfg(test)]
  mod parse_rotation_tests {
      use super::*;

      #[test]
      fn accepts_valid_values() {
          let cases = [
              ("0", Rotation::R0),
              ("90", Rotation::R90),
              ("180", Rotation::R180),
              ("270", Rotation::R270),
          ];

          for (input, expected) in cases {
              assert_eq!(parse_rotation(input), Ok(expected), "input: {input}");
          }
      }

      #[test]
      fn rejects_invalid_value_with_helpful_message() {
          let Err(message) = parse_rotation("45") else {
              panic!("expected \"45\" to be rejected");
          };
          assert!(message.contains("0"), "message: {message}");
          assert!(message.contains("90"), "message: {message}");
          assert!(message.contains("180"), "message: {message}");
          assert!(message.contains("270"), "message: {message}");
      }
  }

  #[cfg(test)]
  mod parse_flip_tests {
      use super::*;

      #[test]
      fn accepts_valid_values() {
          let cases = [("h", Flip::Horizontal), ("v", Flip::Vertical), ("hv", Flip::Both)];

          for (input, expected) in cases {
              assert_eq!(parse_flip(input), Ok(expected), "input: {input}");
          }
      }

      #[test]
      fn rejects_invalid_value_with_helpful_message() {
          let Err(message) = parse_flip("x") else {
              panic!("expected \"x\" to be rejected");
          };
          assert!(message.contains("h"), "message: {message}");
          assert!(message.contains("v"), "message: {message}");
          assert!(message.contains("hv"), "message: {message}");
      }
  }

  #[cfg(test)]
  mod parse_crop_tests {
      use super::*;

      #[test]
      fn accepts_valid_crop_specs() {
          let cases = [
              ("1280x720+320+180", CropRect { width: 1280, height: 720, x: 320, y: 180 }),
              ("100x200+0+0", CropRect { width: 100, height: 200, x: 0, y: 0 }),
          ];

          for (input, expected) in cases {
              assert_eq!(parse_crop(input), Ok(expected), "input: {input}");
          }
      }

      #[test]
      fn rejects_zero_width_or_height() {
          let Err(message) = parse_crop("0x10+0+0") else {
              panic!("expected \"0x10+0+0\" to be rejected");
          };
          assert!(message.contains("non-zero"), "message: {message}");
      }

      #[test]
      fn rejects_malformed_spec() {
          for input in ["1280x720", "1280+320+180", "abcx720+0+0", ""] {
              assert!(parse_crop(input).is_err(), "expected \"{input}\" to be rejected");
          }
      }
  }

  #[cfg(test)]
  mod parse_scale_tests {
      use super::*;

      #[test]
      fn accepts_valid_scale_specs() {
          assert_eq!(parse_scale("960x540"), Ok(ScaleSpec::Exact { width: 960, height: 540 }));
          assert_eq!(parse_scale("0.5"), Ok(ScaleSpec::Factor(0.5)));
          assert_eq!(parse_scale("2"), Ok(ScaleSpec::Factor(2.0)));
      }

      #[test]
      fn rejects_non_positive_factor() {
          let Err(message) = parse_scale("-0.5") else {
              panic!("expected \"-0.5\" to be rejected");
          };
          assert!(message.contains("positive"), "message: {message}");
      }

      #[test]
      fn rejects_garbage_input() {
          let Err(message) = parse_scale("abc") else {
              panic!("expected \"abc\" to be rejected");
          };
          assert!(message.contains("abc"), "message: {message}");
      }
  }

  #[cfg(test)]
  mod args_parse_tests {
      use super::*;

      #[test]
      fn parses_device_and_options() {
          let Ok(args) = Args::try_parse_from(["prog", "0", "--rotate", "90", "--flip", "h"])
          else {
              panic!("expected successful parse");
          };
          assert_eq!(args.device, Some(0));
          assert_eq!(args.rotate, Rotation::R90);
          assert_eq!(args.flip, Some(Flip::Horizontal));
      }

      #[test]
      fn rejects_invalid_rotate_with_usage_error() {
          let Err(error) = Args::try_parse_from(["prog", "--rotate", "45"]) else {
              panic!("expected parse failure for --rotate 45");
          };
          assert_eq!(error.exit_code(), 2);
      }
  }

  #[cfg(test)]
  mod transform_config_tests {
      use super::*;

      fn base_args() -> Args {
          Args {
              device: None,
              list_devices: false,
              rotate: Rotation::R0,
              flip: None,
              crop: None,
              scale: None,
              server_name: "webcam-sharedtexture".to_string(),
              fps: None,
          }
      }

      #[test]
      fn defaults_flip_to_keep_when_absent() {
          let config = base_args().transform_config();
          assert_eq!(config.flip, Flip::Keep);
      }

      #[test]
      fn carries_rotation_crop_and_scale_through() {
          let mut args = base_args();
          args.rotate = Rotation::R90;
          args.crop = Some(CropRect { width: 100, height: 100, x: 0, y: 0 });
          args.scale = Some(ScaleSpec::Factor(0.5));

          let config = args.transform_config();

          assert_eq!(config.rotation, Rotation::R90);
          assert_eq!(config.crop, Some(CropRect { width: 100, height: 100, x: 0, y: 0 }));
          assert_eq!(config.scale, Some(ScaleSpec::Factor(0.5)));
      }
  }
  ```

- [ ] Run and confirm **compile failure** (RED — `parse_rotation`, `parse_flip`, `parse_crop`,
      `parse_scale` are not defined, and `#[arg(..., value_parser = parse_rotation)]` also fails to
      resolve):
  ```
  $ cargo test -p webcam-sharedtexture-cli
  error[E0425]: cannot find function `parse_rotation` in this scope
  error[E0425]: cannot find function `parse_flip` in this scope
  error[E0425]: cannot find function `parse_crop` in this scope
  error[E0425]: cannot find function `parse_scale` in this scope
  error: could not compile `webcam-sharedtexture-cli` (lib test target) due to 4 previous errors
  ```

### 11.3 Implement the four value parsers — GREEN

- [ ] Insert the parser functions above the test module in `args.rs`:

  ```rust
  fn parse_rotation(input: &str) -> Result<Rotation, String> {
      match input {
          "0" => Ok(Rotation::R0),
          "90" => Ok(Rotation::R90),
          "180" => Ok(Rotation::R180),
          "270" => Ok(Rotation::R270),
          other => {
              Err(format!("invalid rotation \"{other}\" (expected one of: 0, 90, 180, 270)"))
          }
      }
  }

  fn parse_flip(input: &str) -> Result<Flip, String> {
      match input {
          "h" => Ok(Flip::Horizontal),
          "v" => Ok(Flip::Vertical),
          "hv" => Ok(Flip::Both),
          other => Err(format!("invalid flip \"{other}\" (expected one of: h, v, hv)")),
      }
  }

  fn crop_format_error(input: &str) -> String {
      format!("invalid crop \"{input}\" (expected format WxH+X+Y, e.g. 1280x720+320+180)")
  }

  fn parse_crop(input: &str) -> Result<CropRect, String> {
      let Some((size, coords)) = input.split_once('+') else {
          return Err(crop_format_error(input));
      };

      let Some((width, height)) = size.split_once('x') else {
          return Err(crop_format_error(input));
      };

      let Ok(width) = width.parse::<u32>() else {
          return Err(crop_format_error(input));
      };
      let Ok(height) = height.parse::<u32>() else {
          return Err(crop_format_error(input));
      };

      let Some((x, y)) = coords.split_once('+') else {
          return Err(crop_format_error(input));
      };

      let Ok(x) = x.parse::<u32>() else {
          return Err(crop_format_error(input));
      };
      let Ok(y) = y.parse::<u32>() else {
          return Err(crop_format_error(input));
      };

      if width == 0 || height == 0 {
          return Err(format!("crop dimensions must be non-zero (got {width}x{height})"));
      }

      Ok(CropRect { width, height, x, y })
  }

  fn scale_format_error(input: &str) -> String {
      format!("invalid scale \"{input}\" (expected WxH or a positive factor, e.g. 960x540 or 0.5)")
  }

  fn parse_scale(input: &str) -> Result<ScaleSpec, String> {
      if let Some((width, height)) = input.split_once('x') {
          let Ok(width) = width.parse::<u32>() else {
              return Err(scale_format_error(input));
          };
          let Ok(height) = height.parse::<u32>() else {
              return Err(scale_format_error(input));
          };
          if width == 0 || height == 0 {
              return Err(format!("scale dimensions must be non-zero (got {width}x{height})"));
          }
          return Ok(ScaleSpec::Exact { width, height });
      }

      let Ok(factor) = input.parse::<f64>() else {
          return Err(scale_format_error(input));
      };
      if !factor.is_finite() || factor <= 0.0 {
          return Err(format!("scale factor must be finite and positive (got {factor})"));
      }

      Ok(ScaleSpec::Factor(factor))
  }
  ```

- [ ] Run — expect **all pass**:
  ```
  $ cargo test -p webcam-sharedtexture-cli
  running 14 tests
  test args::args_parse_tests::parses_device_and_options ... ok
  test args::args_parse_tests::rejects_invalid_rotate_with_usage_error ... ok
  test args::parse_crop_tests::accepts_valid_crop_specs ... ok
  test args::parse_crop_tests::rejects_malformed_spec ... ok
  test args::parse_crop_tests::rejects_zero_width_or_height ... ok
  test args::parse_flip_tests::accepts_valid_values ... ok
  test args::parse_flip_tests::rejects_invalid_value_with_helpful_message ... ok
  test args::parse_rotation_tests::accepts_valid_values ... ok
  test args::parse_rotation_tests::rejects_invalid_value_with_helpful_message ... ok
  test args::parse_scale_tests::accepts_valid_scale_specs ... ok
  test args::parse_scale_tests::rejects_garbage_input ... ok
  test args::parse_scale_tests::rejects_non_positive_factor ... ok
  test args::transform_config_tests::carries_rotation_crop_and_scale_through ... ok
  test args::transform_config_tests::defaults_flip_to_keep_when_absent ... ok

  test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
  ```

### 11.4 Wire `mod args;` into the binary

- [ ] Edit `crates/cli/src/main.rs` — add the module declaration only (implementation of `main`
      itself happens in Task 13; for now keep the existing placeholder body so the binary still
      links):

  ```rust
  //! CLI entry point placeholder for the webcam -> Spout/Syphon sharing tool.

  mod args;

  fn main() {
      println!(
          "webcam-sharedtexture-cli: not yet implemented (core = {})",
          webcam_sharedtexture_core::crate_name()
      );
  }
  ```

- [ ] Run the lint gate:
  ```
  $ cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check
      Checking webcam-sharedtexture-cli v0.1.0 (.../crates/cli)
      Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.8s
  ```
- [ ] Commit:
  ```
  git add crates/cli/src/args.rs crates/cli/src/main.rs
  git commit -m "test(cli): add Args with rotate/flip/crop/scale value parsers"
  ```

---

### Task 12: Device listing & interactive selection (`crates/cli/src/select.rs`)

### 12.1 RED — `format_devices` and `parse_selection`

- [ ] Create `crates/cli/src/select.rs`:

  ```rust
  //! Device listing output and interactive device selection.
  //!
  //! `parse_selection` and `format_devices` are pure and fully unit-tested; `choose_device`
  //! is a thin stdin/stdout loop around `parse_selection` and is exercised manually (see
  //! Task 13's ignored smoke test and Task 14's E2E checklist) — piping fake stdin into a
  //! blocking `read_line` loop under `cargo test` is not worth the added indirection here.

  use std::io::Write;

  use webcam_sharedtexture_core::capture::DeviceInfo;

  use crate::run::CliError;

  #[cfg(test)]
  mod format_devices_tests {
      use super::*;

      #[test]
      fn formats_empty_list() {
          assert_eq!(format_devices(&[]), "");
      }

      #[test]
      fn formats_single_device() {
          let devices = [DeviceInfo { index: 0, name: "FaceTime HD Camera".to_string() }];
          assert_eq!(format_devices(&devices), "0: FaceTime HD Camera");
      }

      #[test]
      fn formats_multiple_devices_joined_by_newline() {
          let devices = [
              DeviceInfo { index: 0, name: "FaceTime HD Camera".to_string() },
              DeviceInfo { index: 1, name: "USB Webcam".to_string() },
          ];
          assert_eq!(format_devices(&devices), "0: FaceTime HD Camera\n1: USB Webcam");
      }
  }

  #[cfg(test)]
  mod parse_selection_tests {
      use super::*;

      #[test]
      fn accepts_valid_index() {
          let cases = [("0", 0), ("2", 2)];
          for (input, expected) in cases {
              assert_eq!(parse_selection(input, 3), Ok(expected), "input: {input}");
          }
      }

      #[test]
      fn trims_surrounding_whitespace_and_newline() {
          assert_eq!(parse_selection(" 1 \n", 3), Ok(1));
      }

      #[test]
      fn rejects_out_of_range_index() {
          assert!(parse_selection("3", 3).is_err());
      }

      #[test]
      fn rejects_non_numeric_input() {
          for input in ["abc", "-1", ""] {
              assert!(parse_selection(input, 3).is_err(), "expected \"{input}\" to be rejected");
          }
      }
  }
  ```

- [ ] Run — expect **compile failure** (`format_devices` / `parse_selection` undefined):
  ```
  $ cargo test -p webcam-sharedtexture-cli
  error[E0425]: cannot find function `format_devices` in this scope
  error[E0425]: cannot find function `parse_selection` in this scope
  error[E0433]: failed to resolve: use of undeclared crate or module `run`
  error: could not compile `webcam-sharedtexture-cli` (lib test target) due to 3 previous errors
  ```

### 12.2 GREEN — implement `format_devices`, `parse_selection`, `choose_device`

`choose_device` references `CliError`, which does not exist until Task 13. Declare a minimal
`CliError` stub now (Task 13 replaces it with the full contract-defined enum plus the additions
noted there — this is an intra-plan sequencing detail, not a contract change).

- [ ] Create `crates/cli/src/run.rs` with just enough to unblock `select.rs` compilation:

  ```rust
  //! Full implementation lands in Task 13. This stub exists only so `select.rs`
  //! (Task 12) has a `CliError` to compile against.

  #[derive(Debug, thiserror::Error)]
  pub enum CliError {
      #[error("device selection cancelled")]
      SelectionCancelled,
  }
  ```

  This requires `thiserror` in `crates/cli/Cargo.toml` a task early — pull it forward here since
  Task 13 needs it anyway:
  ```toml
  [dependencies]
  webcam-sharedtexture-core = { path = "../core" }
  clap = { workspace = true }
  thiserror = { workspace = true }
  ```

- [ ] Add `mod run; mod select;` to `main.rs` (keep the placeholder `main` body):

  ```rust
  mod args;
  mod run;
  mod select;

  fn main() {
      println!(
          "webcam-sharedtexture-cli: not yet implemented (core = {})",
          webcam_sharedtexture_core::crate_name()
      );
  }
  ```

- [ ] Implement `format_devices` and `parse_selection` above the test modules in `select.rs`:

  ```rust
  pub fn format_devices(devices: &[DeviceInfo]) -> String {
      devices
          .iter()
          .map(|device| format!("{}: {}", device.index, device.name))
          .collect::<Vec<_>>()
          .join("\n")
  }

  pub fn parse_selection(input: &str, device_count: usize) -> Result<u32, String> {
      let trimmed = input.trim();

      let Ok(index) = trimmed.parse::<u32>() else {
          return Err(format!("\"{trimmed}\" is not a valid device number"));
      };

      let Ok(count) = u32::try_from(device_count) else {
          return Err("no devices available".to_string());
      };

      if index >= count {
          return Err(format!("{index} is out of range (0..{count})"));
      }

      Ok(index)
  }
  ```

- [ ] Run — expect `format_devices`/`parse_selection` tests to **pass**, but the crate still fails
      to compile until `choose_device` exists (it is referenced nowhere yet, so it's not required
      for these tests alone — run with the test filter to confirm the pure functions are green in
      isolation):
  ```
  $ cargo test -p webcam-sharedtexture-cli select::
  running 7 tests
  test select::format_devices_tests::formats_empty_list ... ok
  test select::format_devices_tests::formats_multiple_devices_joined_by_newline ... ok
  test select::format_devices_tests::formats_single_device ... ok
  test select::parse_selection_tests::accepts_valid_index ... ok
  test select::parse_selection_tests::rejects_non_numeric_input ... ok
  test select::parse_selection_tests::rejects_out_of_range_index ... ok
  test select::parse_selection_tests::trims_surrounding_whitespace_and_newline ... ok

  test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
  ```

- [ ] Add `choose_device` (the thin IO shell; not unit-tested, exercised manually per the module
      doc comment above):

  ```rust
  pub fn choose_device(devices: &[DeviceInfo]) -> Result<u32, CliError> {
      println!("{}", format_devices(devices));

      loop {
          print!("select device index: ");
          std::io::stdout().flush().map_err(|_| CliError::SelectionCancelled)?;

          let mut line = String::new();
          let bytes_read =
              std::io::stdin().read_line(&mut line).map_err(|_| CliError::SelectionCancelled)?;
          if bytes_read == 0 {
              return Err(CliError::SelectionCancelled);
          }

          match parse_selection(&line, devices.len()) {
              Ok(index) => return Ok(index),
              Err(message) => println!("{message}, try again"),
          }
      }
  }
  ```

- [ ] Full workspace build + test:
  ```
  $ cargo test -p webcam-sharedtexture-cli
  running 21 tests
  ...
  test result: ok. 21 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
  ```
- [ ] Lint gate:
  ```
  $ cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check
      Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.9s
  ```
- [ ] Commit:
  ```
  git add crates/cli/src/select.rs crates/cli/src/run.rs crates/cli/src/main.rs crates/cli/Cargo.toml
  git commit -m "test(cli): add device listing format and interactive selection"
  ```

---

### Task 13: `run()` wiring + `main()`

### Contract additions (flagged explicitly)

The fixed contract's `CliError` has five variants (`Capture`, `Publish`, `Pipeline`,
`NonInteractive`, `SelectionCancelled`). Wiring `run()` end-to-end surfaces two more error paths
that have no home in that set. Both are additive (no existing variant changes meaning or
signature):

1. **`UnsupportedPlatform`** — explicitly pre-authorized by this task's brief. `SyphonPublisher`
   only exists under `#[cfg(target_os = "macos")]`; on any other OS there is nothing to publish
   to, and that is a real, expected-at-the-boundary failure, not a bug — it must be a `Result`,
   not a `compile_error!` or panic, so non-mac contributors can still `cargo build --workspace`
   and `cargo test --workspace` (Task 14's gate) on their machine.
2. **`CtrlcSetup`** — `ctrlc::set_handler` returns `Result<(), ctrlc::Error>`. The workspace lints
   deny `unwrap_used`/`expect_used`, so this fallible call needs a variant to propagate through
   just like the other three `#[from]` wrappers already in the enum.

### 13.0 Verify `ctrlc` version

```
$ cargo add --dry-run ctrlc -p webcam-sharedtexture-cli
    Updating crates.io index
      Adding ctrlc v3.5.2 to dependencies
             Features:
             - termination
warning: aborting add due to dry run
```

Confirmed: **ctrlc 3.5.x**. Pin `3.5`.

- [ ] Run the command above and confirm before editing `Cargo.toml`.

### 13.1 Dependency wiring

- [ ] Edit root `Cargo.toml`:
  ```toml
  [workspace.dependencies]
  thiserror = "2"
  clap = { version = "4.6", features = ["derive"] }
  ctrlc = "3.5"
  ```
- [ ] Edit `crates/cli/Cargo.toml` — add `ctrlc` unconditionally, and `webcam-sharedtexture-syphon`
      as a **macOS-only** target dependency (so `cargo build --workspace` on non-mac CI/dev
      machines does not try to compile the ObjC++ bridge):
  ```toml
  [dependencies]
  webcam-sharedtexture-core = { path = "../core" }
  clap = { workspace = true }
  ctrlc = { workspace = true }
  thiserror = { workspace = true }

  [target.'cfg(target_os = "macos")'.dependencies]
  webcam-sharedtexture-syphon = { path = "../syphon" }
  ```
- [ ] Build check:
  ```
  $ cargo build -p webcam-sharedtexture-cli
      Compiling ctrlc v3.5.2
      Compiling webcam-sharedtexture-syphon v0.1.0 (.../crates/syphon)
      Compiling webcam-sharedtexture-cli v0.1.0 (.../crates/cli)
      Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.1s
  ```
- [ ] Commit:
  ```
  git add Cargo.toml crates/cli/Cargo.toml Cargo.lock
  git commit -m "build(cli): add ctrlc and macOS-only syphon dependency"
  ```

### 13.2 RED — `resolve_device` and full `CliError`

- [ ] Replace the stub in `crates/cli/src/run.rs` with the full enum plus test module (no
      implementation of `resolve_device`/`run` yet, so the test module fails to compile — RED):

  ```rust
  //! Wires argument parsing, device resolution, capture, transform, and publish into one run.

  use std::io::IsTerminal;
  use std::sync::Arc;
  use std::sync::atomic::{AtomicBool, Ordering};

  use webcam_sharedtexture_core::capture::{CaptureError, DeviceInfo, NokhwaSource, list_devices};
  use webcam_sharedtexture_core::pipeline::{PipelineError, run_pipeline};
  use webcam_sharedtexture_core::publish::{PublishError, TexturePublisher};

  use crate::args::Args;
  use crate::select::{choose_device, format_devices};

  #[derive(Debug, thiserror::Error)]
  pub enum CliError {
      #[error(transparent)]
      Capture(#[from] CaptureError),
      #[error(transparent)]
      Publish(#[from] PublishError),
      #[error(transparent)]
      Pipeline(#[from] PipelineError),
      #[error("no device specified and stdin is not a TTY")]
      NonInteractive,
      #[error("device selection cancelled")]
      SelectionCancelled,
      /// Contract addition (Task 13): SyphonPublisher only exists on macOS.
      #[error("Syphon/Spout publishing is not supported on this platform")]
      UnsupportedPlatform,
      /// Contract addition (Task 13): surfaces a failed Ctrl+C handler install
      /// instead of `unwrap`/`expect`ing it away.
      #[error(transparent)]
      CtrlcSetup(#[from] ctrlc::Error),
  }

  #[derive(Debug, Clone, Copy, PartialEq, Eq)]
  pub enum DeviceResolution {
      Index(u32),
      NeedsPrompt,
  }

  #[cfg(test)]
  mod resolve_device_tests {
      use super::*;

      fn devices() -> Vec<DeviceInfo> {
          vec![
              DeviceInfo { index: 0, name: "FaceTime HD Camera".to_string() },
              DeviceInfo { index: 1, name: "USB Webcam".to_string() },
          ]
      }

      #[test]
      fn resolves_requested_index_when_available() {
          let Ok(DeviceResolution::Index(index)) = resolve_device(Some(1), &devices(), true)
          else {
              panic!("expected DeviceResolution::Index(1)");
          };
          assert_eq!(index, 1);
      }

      #[test]
      fn rejects_requested_index_not_in_device_list() {
          let Err(error) = resolve_device(Some(5), &devices(), true) else {
              panic!("expected error for out-of-range index");
          };
          assert!(matches!(
              error,
              CliError::Capture(CaptureError::DeviceNotFound { index: 5, available: 2 })
          ));
      }

      #[test]
      fn needs_prompt_when_no_index_and_interactive() {
          assert!(matches!(
              resolve_device(None, &devices(), true),
              Ok(DeviceResolution::NeedsPrompt)
          ));
      }

      #[test]
      fn errors_when_no_index_and_not_interactive() {
          assert!(matches!(resolve_device(None, &devices(), false), Err(CliError::NonInteractive)));
      }
  }

  #[cfg(test)]
  mod cli_error_display_tests {
      use super::*;

      #[test]
      fn non_interactive_message() {
          assert_eq!(
              CliError::NonInteractive.to_string(),
              "no device specified and stdin is not a TTY"
          );
      }

      #[test]
      fn selection_cancelled_message() {
          assert_eq!(CliError::SelectionCancelled.to_string(), "device selection cancelled");
      }

      #[test]
      fn unsupported_platform_message() {
          assert_eq!(
              CliError::UnsupportedPlatform.to_string(),
              "Syphon/Spout publishing is not supported on this platform"
          );
      }
  }
  ```

- [ ] Run — expect **compile failure** (`resolve_device` undefined):
  ```
  $ cargo test -p webcam-sharedtexture-cli
  error[E0425]: cannot find function `resolve_device` in this scope
  error: could not compile `webcam-sharedtexture-cli` (lib test target) due to previous error
  ```

### 13.3 GREEN — `resolve_device`, `run`, `main`

- [ ] Implement `resolve_device` above the test modules:

  ```rust
  pub fn resolve_device(
      requested: Option<u32>,
      available: &[DeviceInfo],
      interactive: bool,
  ) -> Result<DeviceResolution, CliError> {
      let Some(index) = requested else {
          if interactive {
              return Ok(DeviceResolution::NeedsPrompt);
          }
          return Err(CliError::NonInteractive);
      };

      let is_available = available.iter().any(|device| device.index == index);
      if !is_available {
          return Err(CliError::Capture(CaptureError::DeviceNotFound {
              index,
              available: available.len(),
          }));
      }

      Ok(DeviceResolution::Index(index))
  }
  ```

- [ ] Run — expect **all pass**:
  ```
  $ cargo test -p webcam-sharedtexture-cli run::
  running 7 tests
  test run::cli_error_display_tests::non_interactive_message ... ok
  test run::cli_error_display_tests::selection_cancelled_message ... ok
  test run::cli_error_display_tests::unsupported_platform_message ... ok
  test run::resolve_device_tests::errors_when_no_index_and_not_interactive ... ok
  test run::resolve_device_tests::needs_prompt_when_no_index_and_interactive ... ok
  test run::resolve_device_tests::rejects_requested_index_not_in_device_list ... ok
  test run::resolve_device_tests::resolves_requested_index_when_available ... ok

  test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
  ```

- [ ] Add the platform-gated publisher constructor and `run()` (impure IO shell — no new unit
      tests beyond the ignored smoke test below; `resolve_device` above already covers the
      decision logic `run()` delegates to):

  ```rust
  #[cfg(target_os = "macos")]
  fn create_publisher(server_name: &str) -> Result<Box<dyn TexturePublisher>, CliError> {
      let publisher = webcam_sharedtexture_syphon::SyphonPublisher::new(server_name)?;
      Ok(Box::new(publisher))
  }

  #[cfg(not(target_os = "macos"))]
  fn create_publisher(_server_name: &str) -> Result<Box<dyn TexturePublisher>, CliError> {
      Err(CliError::UnsupportedPlatform)
  }

  pub fn run(args: Args) -> Result<(), CliError> {
      let devices = list_devices()?;

      if args.list_devices {
          println!("{}", format_devices(&devices));
          return Ok(());
      }

      let interactive = std::io::stdin().is_terminal();
      let index = match resolve_device(args.device, &devices, interactive)? {
          DeviceResolution::Index(index) => index,
          DeviceResolution::NeedsPrompt => choose_device(&devices)?,
      };

      let mut source = NokhwaSource::open(index, args.fps)?;
      let config = args.transform_config();
      let mut publisher = create_publisher(&args.server_name)?;

      let stop = Arc::new(AtomicBool::new(false));
      let handler_stop = Arc::clone(&stop);
      ctrlc::set_handler(move || handler_stop.store(true, Ordering::SeqCst))?;

      run_pipeline(&mut source, &config, publisher.as_mut(), &stop)?;

      Ok(())
  }
  ```

  Note: `run_pipeline` returns `Ok(())` once `stop` flips true between iterations (per its
  contract-defined loop semantics), so a Ctrl+C-triggered stop naturally surfaces as `Ok(())` here
  — no extra mapping needed.

- [ ] Add the ignored smoke test (documents the manual verification path; requires a real camera
      and, on macOS, a running Syphon client to be meaningful):

  ```rust
  #[cfg(test)]
  mod run_smoke_tests {
      use super::*;

      #[test]
      #[ignore = "requires a real camera; run manually with \
                  `cargo test -p webcam-sharedtexture-cli run_smoke_test -- --ignored`, \
                  then Ctrl+C after a few seconds and confirm it exits 0"]
      fn run_smoke_test() {
          let args = Args {
              device: Some(0),
              list_devices: false,
              rotate: webcam_sharedtexture_core::transform::Rotation::R0,
              flip: None,
              crop: None,
              scale: None,
              server_name: "webcam-sharedtexture-smoke-test".to_string(),
              fps: None,
          };

          let Ok(()) = run(args) else {
              panic!("expected run() to succeed against a real camera + publisher");
          };
      }
  }
  ```

- [ ] Write `crates/cli/src/main.rs` in full:

  ```rust
  //! CLI entry point for the webcam -> Spout/Syphon sharing tool.

  mod args;
  mod run;
  mod select;

  use std::process::ExitCode;

  use clap::Parser;

  use args::Args;
  use run::run;

  fn main() -> ExitCode {
      let args = Args::parse();

      match run(args) {
          Ok(()) => ExitCode::SUCCESS,
          Err(error) => {
              eprintln!("error: {error}");
              ExitCode::FAILURE
          }
      }
  }
  ```

- [ ] Full test run (normal, ignored smoke test excluded by default):
  ```
  $ cargo test -p webcam-sharedtexture-cli
  running 28 tests
  ...
  test run::run_smoke_tests::run_smoke_test ... ignored

  test result: ok. 27 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out
  ```
- [ ] Lint gate:
  ```
  $ cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check
      Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.3s
  ```
- [ ] Commit:
  ```
  git add crates/cli/src/run.rs crates/cli/src/main.rs crates/cli/Cargo.toml Cargo.toml Cargo.lock
  git commit -m "feat(cli): wire device resolution, capture, publish, and Ctrl+C into run()"
  ```

---

### Task 14: End-to-end verification + README

No new logic in this task — it closes the loop between "tests pass" and "the tool actually works
against a real camera and a real Syphon client."

### 14.1 Full workspace gate

- [ ] Build:
  ```
  $ cargo build --workspace
      Finished `dev` profile [unoptimized + debuginfo] target(s) in 6.8s
  ```
- [ ] Test:
  ```
  $ cargo test --workspace
  ...
  test result: ok. <core tests> passed; 0 failed; 0 ignored
  test result: ok. 27 passed; 0 failed; 1 ignored
  ```
- [ ] Clippy:
  ```
  $ cargo clippy --workspace --all-targets -- -D warnings
      Finished `dev` profile [unoptimized + debuginfo] target(s) in 3.1s
  ```
- [ ] Format check:
  ```
  $ cargo fmt --all -- --check
  ```
  (no output = clean)

### 14.2 Manual E2E checklist (macOS, real camera + Syphon client required)

- [ ] List devices:
  ```
  $ cargo run -p webcam-sharedtexture-cli -- --list-devices
  0: FaceTime HD Camera
  ```
  Confirm every attached camera appears, one per line, `"{index}: {name}"`.

- [ ] Open Syphon Recorder (or Simple Client) as the receiving app.

- [ ] Run with transforms:
  ```
  $ cargo run -p webcam-sharedtexture-cli -- 0 --rotate 90 --flip h --scale 0.5
  ```
  - [ ] Confirm a server named `webcam-sharedtexture` appears in the Syphon client's server list.
  - [ ] Confirm the received image is rotated 90° clockwise relative to the raw camera feed.
  - [ ] Confirm the received image is horizontally mirrored.
  - [ ] Confirm the received image's resolution is half the camera's native resolution (width and
        height both halved, within nearest-neighbor rounding).

- [ ] Press Ctrl+C in the terminal running the CLI:
  - [ ] Confirm the process exits promptly (no hang).
  - [ ] Confirm the exit code is `0`:
    ```
    $ echo $?
    0
    ```
  - [ ] Confirm the Syphon client shows the server disappearing (publisher dropped cleanly).

- [ ] Negative-path check — run with no device index from a non-TTY context:
  ```
  $ cargo run -p webcam-sharedtexture-cli -- < /dev/null
  error: no device specified and stdin is not a TTY
  $ echo $?
  1
  ```

- [ ] Negative-path check — invalid rotation value:
  ```
  $ cargo run -p webcam-sharedtexture-cli -- --rotate 45
  error: invalid value '45' for '--rotate <ROTATE>': invalid rotation "45" (expected one of: 0, 90, 180, 270)
  ...
  $ echo $?
  2
  ```

### 14.3 README.md

- [ ] Create `README.md` at the repo root with the following content:

  ```markdown
  # web-cam-sharedtexture

  A small CLI (and, later, GUI) tool that reads a webcam feed, applies rotate / flip / crop /
  scale transforms, and publishes the result as a Syphon shared texture on macOS (Spout on
  Windows is defined as a trait only, not yet implemented).

  ## Setup

  1. Install the toolchain via [mise](https://mise.jdx.dev/):
     ```
     mise install
     ```
     This pins the Rust, Node, and pnpm versions declared in `mise.toml`.

  2. Install dev tooling (husky pre-commit hook: `cargo fmt --check` + `cargo clippy -D warnings`
     + `cargo test`):
     ```
     pnpm install
     ```

  3. Fetch and build the vendored Syphon framework (macOS only — required to build
     `webcam-sharedtexture-syphon` / run the CLI's publish step):
     ```
     git submodule update --init vendor/syphon-src
     xcodebuild -project vendor/syphon-src/Syphon.xcodeproj \
       -scheme Syphon -configuration Release \
       SYMROOT=vendor/syphon-src/build
     cp -R vendor/syphon-src/build/Release/Syphon.framework vendor/Syphon.framework
     ```
     (`vendor/Syphon.framework` and `vendor/syphon-src/build` are gitignored — this step must be
     run once per clone/CI machine.)

  4. Build the workspace:
     ```
     cargo build --workspace
     ```

  ## CLI usage

  ```
  webcam-sharedtexture [DEVICE_INDEX] [OPTIONS]
  ```

  | Option | Values | Default | Description |
  |---|---|---|---|
  | `DEVICE_INDEX` | integer | interactive prompt (TTY) / error (non-TTY) | Camera device index; see `--list-devices` |
  | `--list-devices` | flag | — | List available cameras as `{index}: {name}` and exit |
  | `--rotate <N>` | `0`, `90`, `180`, `270` | `0` | Clockwise rotation, applied first (after crop) |
  | `--flip <F>` | `h`, `v`, `hv` | no flip | Mirror horizontally, vertically, or both |
  | `--crop <SPEC>` | `WxH+X+Y`, e.g. `1280x720+320+180` | no crop | Crop before any other transform |
  | `--scale <SPEC>` | `WxH` or a positive factor, e.g. `960x540` or `0.5` | no scale | Resize, applied last |
  | `--server-name <NAME>` | string | `webcam-sharedtexture` | Name of the published Syphon server |
  | `--fps <N>` | integer | camera default | Requested capture fps (best-effort) |

  Transform order is always **crop → rotate → flip → scale**, regardless of the order options are
  given on the command line.

  Examples:
  ```
  webcam-sharedtexture --list-devices
  webcam-sharedtexture 0 --rotate 90 --flip h --scale 0.5
  webcam-sharedtexture 0 --crop 1280x720+320+180 --server-name my-camera
  ```

  Exit codes: `0` clean shutdown (including Ctrl+C) · `1` runtime error (printed to stderr) ·
  `2` CLI usage error (invalid/missing argument).

  ## License

  MIT — see workspace `Cargo.toml` (`license = "MIT"`).

  This project vendors [Syphon-Framework](https://github.com/Syphon/Syphon-Framework) (via the
  `vendor/syphon-src` git submodule) as a build-time dependency of
  `webcam-sharedtexture-syphon`. Syphon-Framework is distributed under a BSD-style license; the
  full text ships with the submodule at `vendor/syphon-src/LICENSE`. No Syphon-Framework source
  is copied into this repository — it is fetched and built locally per the Setup steps above.
  ```

- [ ] Confirm rendering / no broken links by eye; no lint tool runs against README (not part of
      the `husky` pre-commit gate).

### 14.4 Final commit

- [ ] Stage and commit:
  ```
  git add README.md
  git commit -m "docs: add README with setup, CLI usage table, and license notice"
  ```
- [ ] Re-run the full gate one last time to confirm nothing regressed after the README-only
      change:
  ```
  cargo build --workspace && cargo test --workspace && \
    cargo clippy --workspace --all-targets -- -D warnings && cargo fmt --all -- --check
  ```
