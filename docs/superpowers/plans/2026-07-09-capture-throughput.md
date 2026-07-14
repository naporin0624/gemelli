# Capture Throughput Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Raise webcam→Spout/Syphon throughput from ~1–5fps to ≥30fps by making the camera negotiate MJPEG (fast decode) and by replacing the slow RGB→BGRA conversion — all inside `gemelli-core`, with no public API change.

**Architecture:** Two independent CPU-cost fixes in `crates/core/src/capture.rs`, proven by measurement: (1) a fast, preallocated `rgb_to_bgra` swizzle; (2) after opening the camera, enumerate its formats and `set_camera_format` to the best high-frame-rate MJPEG one (a pure `select_mjpeg_format` chooser drives the choice). `next_frame` still decodes via `RgbFormat`, so MJPEG frames go through nokhwa's fast mozjpeg path.

**Tech Stack:** Rust (edition 2024), nokhwa 0.10.11 (`Camera::compatible_camera_formats`, `set_camera_format`, `CameraFormat`, `FrameFormat::MJPEG`, `Resolution`).

## Global Constraints

- Rust edition `2024`; workspace toolchain pinned `1.96.1` (mise). `gemelli-core` builds on the default local toolchain too (no vergen dependency).
- Workspace clippy denies `unwrap_used`, `expect_used`, `as_conversions`. `clippy.toml` allows `unwrap`/`expect` in tests only. **No `as` casts anywhere** — use `u32::from`/`u64::from`/`try_from`. Do not discard a `Result` with `let _ = ...` (use `.is_err()`/explicit match).
- Public API is frozen: `NokhwaSource::open(index: u32, requested_fps: Option<u32>) -> Result<Self, CaptureError>`, `CaptureSource::next_frame`, and `Frame` keep their signatures. Only internals change. No changes to `pipeline.rs`, `transform/*`, CLI, GUI, spout, or syphon.
- `Frame` is BGRA8 tightly-packed (`data.len() == width*height*4`). `rgb_to_bgra` must return exactly `width*height*4` bytes for valid input, RGB→BGRA order, alpha `255`.
- nokhwa facts (verified in nokhwa-0.10.11 source): `Camera::compatible_camera_formats(&mut self) -> Result<Vec<CameraFormat>, NokhwaError>`; `Camera::set_camera_format(&mut self, CameraFormat) -> Result<(), NokhwaError>` (call BEFORE `open_stream`); `CameraFormat::new(Resolution, FrameFormat, frame_rate: u32)`, `.resolution()`, `.width()`, `.height()`, `.frame_rate() -> u32`, `.format() -> FrameFormat`; `CameraFormat` is `Copy`; `Resolution::new(x, y)`, `.width()`, `.height()`; `FrameFormat::{MJPEG, YUYV, NV12}`.
- Commit trailer: `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`.
- MJPEG resolution cap constants: `MAX_MJPEG_WIDTH = 1920`, `MAX_MJPEG_HEIGHT = 1080`. High-fps priority: prefer highest frame rate, tie-break on larger resolution.

---

### Task 1: Fast RGB→BGRA conversion

**Files:**
- Modify: `crates/core/src/capture.rs` (`rgb_to_bgra`, lines ~34-45; tests module).

**Interfaces:**
- Produces: `fn rgb_to_bgra(rgb: &[u8], width: u32, height: u32) -> Vec<u8>` — unchanged signature and observable output; faster implementation.

- [ ] **Step 1: Add a stronger characterization test**

In `capture.rs` `#[cfg(test)] mod tests`, add (alongside the existing `rgb_to_bgra_swizzles_channels_and_adds_opaque_alpha`):

```rust
#[test]
fn rgb_to_bgra_handles_multiple_rows_and_returns_exact_length() {
    // 2x2: four distinct RGB pixels, row-major.
    let rgb = vec![
        1, 2, 3, 4, 5, 6, // row 0: (R1 G2 B3) (R4 G5 B6)
        7, 8, 9, 10, 11, 12, // row 1: (R7 G8 B9) (R10 G11 B12)
    ];

    let bgra = rgb_to_bgra(&rgb, 2, 2);

    assert_eq!(bgra.len(), 2 * 2 * 4);
    assert_eq!(
        bgra,
        vec![
            3, 2, 1, 255, 6, 5, 4, 255, // row 0 → BGRA
            9, 8, 7, 255, 12, 11, 10, 255, // row 1 → BGRA
        ]
    );
}
```

- [ ] **Step 2: Run it against the current implementation (must pass — this pins behavior before the refactor)**

Run: `cargo test -p gemelli-core rgb_to_bgra`
Expected: both `rgb_to_bgra` tests PASS (the current loop already produces this output).

- [ ] **Step 3: Replace the implementation with the fast, preallocated swizzle**

Replace the body of `rgb_to_bgra`:

```rust
/// Converts tightly-packed RGB8 to tightly-packed BGRA8 with opaque alpha.
/// Preallocates the output and writes each pixel by index so the swizzle
/// vectorizes, instead of pushing a fresh 4-byte array per pixel.
fn rgb_to_bgra(rgb: &[u8], width: u32, height: u32) -> Vec<u8> {
    let pixel_count =
        usize::try_from(width).unwrap_or(0).saturating_mul(usize::try_from(height).unwrap_or(0));
    let mut bgra = vec![0_u8; pixel_count.saturating_mul(4)];

    for (src, dst) in rgb.chunks_exact(3).zip(bgra.chunks_exact_mut(4)) {
        dst[0] = src[2];
        dst[1] = src[1];
        dst[2] = src[0];
        dst[3] = 255;
    }

    bgra
}
```

- [ ] **Step 4: Run the tests to verify they still pass**

Run: `cargo test -p gemelli-core rgb_to_bgra`
Expected: both tests PASS (identical output, faster path).

- [ ] **Step 5: Add an ignored micro-benchmark to record the win**

Add to the tests module:

```rust
#[test]
#[ignore = "micro-benchmark; run manually with `cargo test -p gemelli-core \
            bench_rgb_to_bgra -- --ignored --nocapture`"]
fn bench_rgb_to_bgra() {
    let (w, h) = (1920_u32, 1080_u32);
    let len = usize::try_from(w).unwrap() * usize::try_from(h).unwrap() * 3;
    let rgb = vec![128_u8; len];

    let start = std::time::Instant::now();
    let iters = 100;
    for _ in 0..iters {
        let out = rgb_to_bgra(&rgb, w, h);
        std::hint::black_box(&out);
    }
    let per = start.elapsed().as_secs_f64() * 1000.0 / f64::from(iters);
    println!("rgb_to_bgra {w}x{h}: {per:.2} ms/frame");
}
```

- [ ] **Step 6: Run the benchmark and confirm it is fast**

Run: `cargo test -p gemelli-core bench_rgb_to_bgra -- --ignored --nocapture`
Expected: prints a single-digit (or low) ms/frame (baseline was ~157ms; target well under ~10ms in release; debug will be slower but far below 157ms). Record the number in the commit body.

- [ ] **Step 7: Lint + commit**

Run: `cargo clippy -p gemelli-core --all-targets -- -D warnings` (no warnings), `cargo fmt -p gemelli-core -- --check`.
```bash
git add crates/core/src/capture.rs
git commit -m "perf(core): vectorizable preallocated RGB->BGRA swizzle

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: `select_mjpeg_format` chooser (pure function)

**Files:**
- Modify: `crates/core/src/capture.rs` (new fn + `MAX_MJPEG_WIDTH`/`MAX_MJPEG_HEIGHT` consts; tests module + imports).

**Interfaces:**
- Produces: `fn select_mjpeg_format(formats: &[CameraFormat], requested_fps: Option<u32>, max_width: u32, max_height: u32) -> Option<CameraFormat>`. Consumed by Task 3.

- [ ] **Step 1: Add the failing tests**

At the top of `capture.rs`, ensure the import line includes `CameraFormat`, `FrameFormat`, `Resolution`:
```rust
use nokhwa::utils::{
    ApiBackend, CameraFormat, CameraIndex, CameraInfo, FrameFormat, RequestedFormat,
    RequestedFormatType, Resolution,
};
```
Add to `#[cfg(test)] mod tests` (and add `CameraFormat, FrameFormat, Resolution, select_mjpeg_format` to the `use super::{...}` list, plus `MAX_MJPEG_WIDTH, MAX_MJPEG_HEIGHT`):

```rust
fn fmt(w: u32, h: u32, format: FrameFormat, fps: u32) -> CameraFormat {
    CameraFormat::new(Resolution::new(w, h), format, fps)
}

#[test]
fn select_prefers_highest_frame_rate_mjpeg_when_no_fps_requested() {
    let formats = vec![
        fmt(1920, 1080, FrameFormat::MJPEG, 30),
        fmt(1280, 720, FrameFormat::MJPEG, 60),
        fmt(1920, 1080, FrameFormat::YUYV, 60),
    ];
    let chosen = select_mjpeg_format(&formats, None, MAX_MJPEG_WIDTH, MAX_MJPEG_HEIGHT)
        .expect("an MJPEG format is available");
    assert_eq!(chosen.frame_rate(), 60);
    assert_eq!(chosen.format(), FrameFormat::MJPEG);
    assert_eq!((chosen.width(), chosen.height()), (1280, 720));
}

#[test]
fn select_breaks_frame_rate_ties_on_larger_resolution() {
    let formats = vec![
        fmt(1280, 720, FrameFormat::MJPEG, 60),
        fmt(1920, 1080, FrameFormat::MJPEG, 60),
    ];
    let chosen = select_mjpeg_format(&formats, None, MAX_MJPEG_WIDTH, MAX_MJPEG_HEIGHT).unwrap();
    assert_eq!((chosen.width(), chosen.height()), (1920, 1080));
}

#[test]
fn select_excludes_mjpeg_above_the_resolution_cap() {
    let formats = vec![fmt(2304, 1296, FrameFormat::MJPEG, 30)];
    assert_eq!(select_mjpeg_format(&formats, None, MAX_MJPEG_WIDTH, MAX_MJPEG_HEIGHT), None);
}

#[test]
fn select_returns_none_when_no_mjpeg_present() {
    let formats = vec![fmt(1920, 1080, FrameFormat::YUYV, 60)];
    assert_eq!(select_mjpeg_format(&formats, None, MAX_MJPEG_WIDTH, MAX_MJPEG_HEIGHT), None);
}

#[test]
fn select_picks_closest_frame_rate_to_requested_fps() {
    let formats = vec![
        fmt(1920, 1080, FrameFormat::MJPEG, 60),
        fmt(1920, 1080, FrameFormat::MJPEG, 30),
        fmt(1280, 720, FrameFormat::MJPEG, 24),
    ];
    let chosen = select_mjpeg_format(&formats, Some(30), MAX_MJPEG_WIDTH, MAX_MJPEG_HEIGHT).unwrap();
    assert_eq!(chosen.frame_rate(), 30);
}
```

- [ ] **Step 2: Run to verify they fail to compile (function/consts not defined)**

Run: `cargo test -p gemelli-core select_`
Expected: FAIL — `cannot find function select_mjpeg_format` / `MAX_MJPEG_WIDTH` not found.

- [ ] **Step 3: Implement the consts and the chooser**

Add near the top of `capture.rs` (after imports):

```rust
/// Resolution cap for the auto-selected MJPEG format. High frame rate is
/// preferred over resolution, so we never pick a huge low-fps mode.
const MAX_MJPEG_WIDTH: u32 = 1920;
const MAX_MJPEG_HEIGHT: u32 = 1080;
```

Add the function (e.g. just below `format_candidates`):

```rust
/// Chooses the best MJPEG format for high throughput: MJPEG only, within the
/// resolution cap, preferring the highest frame rate (tie-break: larger area).
/// With `requested_fps`, prefers the frame rate closest to it instead. Returns
/// `None` when the camera exposes no MJPEG format within the cap.
fn select_mjpeg_format(
    formats: &[CameraFormat],
    requested_fps: Option<u32>,
    max_width: u32,
    max_height: u32,
) -> Option<CameraFormat> {
    let area = |f: &CameraFormat| u64::from(f.width()) * u64::from(f.height());

    formats
        .iter()
        .copied()
        .filter(|f| f.format() == FrameFormat::MJPEG)
        .filter(|f| f.width() <= max_width && f.height() <= max_height)
        .max_by(|a, b| match requested_fps {
            Some(fps) => {
                // Closest frame rate wins; larger area breaks ties.
                b.frame_rate().abs_diff(fps).cmp(&a.frame_rate().abs_diff(fps)).then(area(a).cmp(&area(b)))
            }
            None => a.frame_rate().cmp(&b.frame_rate()).then(area(a).cmp(&area(b))),
        })
}
```

Note: `max_by` returns the maximum per the comparator. For `None` fps, larger `frame_rate` then larger `area` is "greater" → chosen. For `Some(fps)`, we invert the distance (smaller distance must compare "greater"), hence `b.dist().cmp(&a.dist())`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p gemelli-core select_`
Expected: all five `select_*` tests PASS.

- [ ] **Step 5: Lint + commit**

Run: `cargo clippy -p gemelli-core --all-targets -- -D warnings`, `cargo fmt -p gemelli-core -- --check`.
```bash
git add crates/core/src/capture.rs
git commit -m "feat(core): add high-fps MJPEG format chooser

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: Negotiate MJPEG in `NokhwaSource::open`

**Files:**
- Modify: `crates/core/src/capture.rs` (`NokhwaSource::open`, lines ~106-130; tests module for an ignored smoke test).

**Interfaces:**
- Consumes: `select_mjpeg_format`, `MAX_MJPEG_WIDTH`, `MAX_MJPEG_HEIGHT` (Task 2); existing `format_candidates`, `open_failed`.
- Produces: `NokhwaSource::open` unchanged signature; after this task the camera negotiates MJPEG when available.

- [ ] **Step 1: Rewrite `open` to switch to MJPEG before streaming**

Replace the `open` method body:

```rust
    pub fn open(index: u32, requested_fps: Option<u32>) -> Result<Self, CaptureError> {
        let mut attempts = format_candidates(requested_fps).into_iter();
        let Some(mut format_type) = attempts.next() else {
            return Err(CaptureError::OpenFailed {
                index,
                reason: "no capture format candidates".to_string(),
            });
        };

        // Open the camera first (any working format) so we can enumerate its
        // real formats, then switch to a fast MJPEG one before streaming.
        let mut camera = loop {
            let requested = RequestedFormat::new::<RgbFormat>(format_type);
            match Camera::new(CameraIndex::Index(index), requested) {
                Ok(camera) => break camera,
                Err(error) => {
                    let Some(next_format) = attempts.next() else {
                        return Err(open_failed(index, error));
                    };
                    format_type = next_format;
                }
            }
        };

        // Prefer a high-frame-rate MJPEG format: nokhwa's uncompressed (YUYV)
        // decode path is ~15x slower than its MJPEG (mozjpeg) path. Best-effort
        // — if the camera cannot enumerate or refuses the switch, keep the
        // format it opened with rather than failing the whole open.
        if let Ok(formats) = camera.compatible_camera_formats() {
            if let Some(best) =
                select_mjpeg_format(&formats, requested_fps, MAX_MJPEG_WIDTH, MAX_MJPEG_HEIGHT)
            {
                if camera.set_camera_format(best).is_err() {
                    // Camera refused the MJPEG switch; the opened format stays.
                }
            }
        }

        camera.open_stream().map_err(|error| open_failed(index, error))?;
        Ok(Self { camera })
    }
```

- [ ] **Step 2: Add an ignored real-camera smoke test asserting MJPEG negotiation**

Add to the tests module:

```rust
#[test]
#[ignore = "requires a real camera; run manually with \
            `cargo test -p gemelli-core opens_camera_as_mjpeg -- --ignored --nocapture`"]
fn opens_camera_as_mjpeg() {
    let source = super::NokhwaSource::open(0, None).expect("camera opens");
    let format = source.camera.camera_format();
    println!("negotiated: {}x{} @ {}fps {:?}", format.width(), format.height(), format.frame_rate(), format.format());
    assert_eq!(format.format(), nokhwa::utils::FrameFormat::MJPEG);
}
```
(The test reads the private `camera` field — it lives in the same module, so this is allowed.)

- [ ] **Step 3: Build + run the non-ignored suite (open has no pure unit test; the chooser is covered in Task 2)**

Run: `cargo test -p gemelli-core`
Expected: all existing + Task 1/2 tests PASS; the two `#[ignore]`d tests are skipped.

- [ ] **Step 4: Lint + commit**

Run: `cargo clippy -p gemelli-core --all-targets -- -D warnings`, `cargo fmt -p gemelli-core -- --check`.
```bash
git add crates/core/src/capture.rs
git commit -m "perf(core): negotiate high-fps MJPEG in NokhwaSource::open

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 4: Real-hardware verification (Windows + StreamCam)

**Files:** none (verification; may produce a follow-up tuning commit).

**Interfaces:** exercises the full CLI pipeline end to end.

- [ ] **Step 1: Fetch vendored deps in this worktree (gitignored, not inherited)**

Run:
```bash
./scripts/fetch-spout.sh
./scripts/fetch-fonts.sh
```
Expected: `vendor/Spout2/` and `vendor/fonts/` populated (needed to build spout/gui in this worktree).

- [ ] **Step 2: Full workspace build + lint + test**

Run:
```bash
cargo build --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: all green.

- [ ] **Step 3: Confirm MJPEG negotiation on the real camera**

Run: `cargo test -p gemelli-core opens_camera_as_mjpeg -- --ignored --nocapture`
Expected: prints `negotiated: … MJPEG` and PASSES (format is MJPEG, not YUYV).

- [ ] **Step 4: Measure end-to-end fps against the real camera + Spout receiver**

Run `./target/debug/gemelli.exe 0 --server-name gemelli`, open a Spout receiver (e.g. the Spout `SpoutReceiver` demo). Confirm:
- the receiver shows the webcam as sender `gemelli`, correct colours, upright;
- the frame rate is materially improved over the ~1–5fps baseline — target **≥30fps** (use the receiver's fps overlay and/or a temporary per-frame timing print, reverted after).
If it falls short, capture per-stage timings again (decode/convert) to see the remaining bottleneck and record it. Do not add multithreading here (out of scope); note it as follow-up if decode/convert still dominates.

- [ ] **Step 5: Record results**

Append a short before/after note (negotiated format, decode/convert ms, observed fps) to the design doc's verification section or the PR description. Commit any doc update:
```bash
git add docs/superpowers/specs/2026-07-09-capture-throughput-design.md
git commit -m "docs: record capture throughput before/after measurements

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Self-Review

**Spec coverage:**
- ① MJPEG format selection → Task 2 (chooser) + Task 3 (wiring). ✓
- ② fast color conversion → Task 1. ✓
- pure, testable chooser `select_mjpeg_format` → Task 2. ✓
- high-fps priority, resolution cap, requested_fps handling → Task 2 logic + tests. ✓
- public API unchanged (`open`/`next_frame`/`Frame`) → Task 3 keeps signature; no other crate touched. ✓
- micro-benchmark for conversion + real-hardware fps → Task 1 Step 5/6, Task 4. ✓
- multithreading out of scope → not in any task; Task 4 Step 4 explicitly defers it. ✓
- verify negotiated format is MJPEG + ≥30fps + Spout receive → Task 3 smoke test + Task 4. ✓

**Placeholder scan:** every code step has complete code; no TBD/TODO. ✓

**Type consistency:** `select_mjpeg_format(&[CameraFormat], Option<u32>, u32, u32) -> Option<CameraFormat>` identical in Task 2 (def) and Task 3 (call). `CameraFormat` accessors `.format()/.frame_rate()/.width()/.height()` used consistently. `MAX_MJPEG_WIDTH`/`MAX_MJPEG_HEIGHT` defined in Task 2, used in Task 3. `rgb_to_bgra` signature unchanged. ✓
