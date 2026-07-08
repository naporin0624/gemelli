# gemelli-spout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Windows-only `gemelli-spout` crate that publishes gemelli's BGRA webcam frames to Spout, wired into CLI and GUI exactly like the existing macOS `gemelli-syphon`.

**Architecture:** `SpoutPublisher` implements `gemelli_core::publish::TexturePublisher` over a thin `extern "C"` C++ bridge (`cpp/spout_bridge.cpp`) that wraps Spout2's `spoutDX` sender. Because `TexturePublisher::publish` hands a CPU BGRA buffer, we use `spoutDX::SendImage` (CPU path), not GPU texture sharing. The whole crate is `#![cfg(target_os = "windows")]`-gated so release-please can still parse its manifest.

**Tech Stack:** Rust (edition 2024), `cc` crate, MSVC (`/std:c++17`), Spout2 SDK 2.007.017 (D3D11), fetched into `vendor/Spout2/`.

## Global Constraints

- Rust edition: `2024` (workspace). Toolchain pinned `1.96.1` (mise); local dev observed on 1.93 — either builds.
- Workspace clippy denies `unwrap_used`, `expect_used`, `as_conversions` (`Cargo.toml [workspace.lints.clippy]`). `clippy.toml` sets `allow-unwrap-in-tests = true`, `allow-expect-in-tests = true`, so tests may use `unwrap`/`expect` but **must not** use `as` casts; use `u32::try_from`/`usize::try_from`.
- Native publisher crates are gated crate-wide via `#![cfg(target_os = "...")]` in `src/lib.rs`, and depended on **without** a `[target.'cfg(...)'.dependencies]` table (release-please's Rust manifest updater cannot parse cfg target tables). Follow `crates/syphon` verbatim.
- `Frame` is BGRA8 tightly-packed (stride = `width*4`), invariant `data.len() == width*height*4`.
- Spout2 SDK pinned tag: `2.007.017`. Sender format: `DXGI_FORMAT_B8G8R8A8_UNORM`.
- Never commit without being asked; commits below are part of the requested PR work.
- Commit message trailer: `Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`.

---

### Task 1: Spout2 SDK fetch script

**Files:**
- Create: `scripts/fetch-spout.sh`

**Interfaces:**
- Produces: `vendor/Spout2/SpoutDirectX/SpoutDX/{SpoutDX.cpp,SpoutDX.h}` and `vendor/Spout2/SpoutGL/*` on disk (gitignored). `crates/spout/build.rs` (Task 2) consumes these paths.

- [ ] **Step 1: Write the fetch script**

Create `scripts/fetch-spout.sh` (mirror of `scripts/fetch-fonts.sh`):

```bash
#!/usr/bin/env bash
# Fetch the Spout2 SDK (BSD-2-Clause) into vendor/Spout2/ for the gemelli-spout
# native bridge — crates/spout/build.rs compiles SpoutDX + SpoutGL from here.
# Mirrors scripts/fetch-fonts.sh; vendor/Spout2 is gitignored. Required before
# `cargo build -p gemelli-spout` on Windows.
set -euo pipefail

TAG="2.007.017"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="$ROOT/vendor/Spout2"
URL="https://github.com/leadedge/Spout2/archive/refs/tags/${TAG}.tar.gz"

tmp="$ROOT/_spout2_tmp"
rm -rf "$tmp" "$DEST"
mkdir -p "$tmp" "$DEST/SpoutDirectX"
trap 'rm -rf "$tmp"' EXIT

echo "Downloading Spout2 ${TAG} from $URL" >&2
if ! curl -fsSL "$URL" -o "$tmp/spout2.tar.gz"; then
  echo "ERROR: failed to download $URL" >&2
  exit 1
fi

tar -xzf "$tmp/spout2.tar.gz" -C "$tmp"
src="$tmp/Spout2-${TAG}/SPOUTSDK"
if [ ! -f "$src/SpoutDirectX/SpoutDX/SpoutDX.cpp" ] || [ ! -d "$src/SpoutGL" ]; then
  echo "ERROR: expected SDK layout not found under $src" >&2
  exit 1
fi

# Preserve the SDK's directory structure: SpoutDX.h references ../../SpoutGL/.
cp -R "$src/SpoutDirectX/SpoutDX" "$DEST/SpoutDirectX/SpoutDX"
cp -R "$src/SpoutGL" "$DEST/SpoutGL"
# Keep the license next to the vendored source (referenced by THIRD-PARTY-NOTICES).
cp "$tmp/Spout2-${TAG}/LICENSE" "$DEST/LICENSE" 2>/dev/null || true

echo "Spout2 SDK ${TAG} fetched to $DEST" >&2
```

- [ ] **Step 2: Make it executable and run it**

Run:
```bash
chmod +x scripts/fetch-spout.sh
./scripts/fetch-spout.sh
```
Expected: prints "Spout2 SDK 2.007.017 fetched to …/vendor/Spout2".

- [ ] **Step 3: Verify the SDK files landed**

Run:
```bash
ls vendor/Spout2/SpoutDirectX/SpoutDX/SpoutDX.cpp \
   vendor/Spout2/SpoutGL/SpoutDirectX.cpp \
   vendor/Spout2/SpoutGL/SpoutSenderNames.cpp \
   vendor/Spout2/SpoutGL/SpoutFrameCount.cpp \
   vendor/Spout2/SpoutGL/SpoutUtils.cpp \
   vendor/Spout2/SpoutGL/SpoutCopy.cpp \
   vendor/Spout2/SpoutGL/SpoutSharedMemory.cpp
```
Expected: all seven paths exist (no "No such file" error).

- [ ] **Step 4: Commit**

```bash
git add scripts/fetch-spout.sh
git commit -m "build(spout): add Spout2 SDK fetch script

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```
(`vendor/Spout2/` is gitignored, so only the script is committed.)

---

### Task 2: `gemelli-spout` crate — native bridge + `SpoutPublisher`

**Files:**
- Create: `crates/spout/Cargo.toml`
- Create: `crates/spout/build.rs`
- Create: `crates/spout/cpp/spout_bridge.h`
- Create: `crates/spout/cpp/spout_bridge.cpp`
- Create: `crates/spout/src/ffi.rs`
- Create: `crates/spout/src/lib.rs`
- Modify: `Cargo.toml` (workspace `members`)

**Interfaces:**
- Consumes: `gemelli_core::frame::Frame` (`.width() -> u32`, `.height() -> u32`, `.data() -> &[u8]`), `gemelli_core::publish::{TexturePublisher, PublishError}` (`PublishError::ServerCreate { name, reason }`, `PublishError::Publish { reason }`), and `vendor/Spout2/**` from Task 1.
- Produces: `gemelli_spout::SpoutPublisher` with `fn new(server_name: &str) -> Result<Self, PublishError>` and `impl TexturePublisher`. Consumed by Tasks 4 (CLI) and 5 (GUI).

- [ ] **Step 1: Add the crate to the workspace**

Modify `Cargo.toml` (root) `members`:
```toml
members = ["crates/core", "crates/cli", "crates/gui", "crates/syphon", "crates/spout"]
```

- [ ] **Step 2: Write `crates/spout/Cargo.toml`**

```toml
[package]
name = "gemelli-spout"
version = "0.2.0"
edition.workspace = true
license.workspace = true
repository.workspace = true
# Matches the gemelli-syphon convention (see crates/syphon/Cargo.toml): the
# `links` key namespaces this crate's native lib so nothing else links it twice.
# Unlike Syphon (a dylib framework needing rpath), Spout2 links statically, so
# no rpath metadata is published for downstream crates to consume.
links = "spout_bridge"

[lints]
workspace = true

[dependencies]
gemelli-core = { path = "../core" }

[build-dependencies]
cc = "1"
```

- [ ] **Step 3: Write the native bridge header `crates/spout/cpp/spout_bridge.h`**

```cpp
#ifndef GEMELLI_SPOUT_BRIDGE_H
#define GEMELLI_SPOUT_BRIDGE_H

#include <cstdint>

#ifdef __cplusplus
extern "C" {
#endif

// Opaque handle to a Spout DirectX sender.
typedef struct SpoutBridgeHandle SpoutBridgeHandle;

// Create a Spout sender advertised under `name` (NUL-terminated UTF-8).
// Returns nullptr on failure (no D3D11 device or name rejected).
SpoutBridgeHandle* spout_bridge_create(const char* name);

// Send one BGRA8 frame. `pixels` must point to at least `pitch * height`
// readable bytes; `pitch` is the row stride in bytes (>= width*4). Pixels are
// copied before returning. Returns true on success.
bool spout_bridge_send_bgra(SpoutBridgeHandle* handle,
                            const uint8_t* pixels,
                            uint32_t width,
                            uint32_t height,
                            uint32_t pitch);

// Release the sender and its D3D11 device.
void spout_bridge_destroy(SpoutBridgeHandle* handle);

#ifdef __cplusplus
}
#endif

#endif // GEMELLI_SPOUT_BRIDGE_H
```

- [ ] **Step 4: Write the native bridge `crates/spout/cpp/spout_bridge.cpp`**

```cpp
// C++ bridge exposing a minimal Spout DirectX sender to Rust FFI. Sender-only,
// CPU-pixel path: gemelli hands a BGRA8 CPU buffer per frame
// (gemelli-core::Frame), so we use spoutDX::SendImage rather than the GPU
// texture-handle path used by richer Spout integrations.

#include "spout_bridge.h"

#include "SpoutDX.h"

struct SpoutBridgeHandle {
    spoutDX sender;
};

extern "C" {

SpoutBridgeHandle* spout_bridge_create(const char* name) {
    if (!name) {
        return nullptr;
    }

    SpoutBridgeHandle* handle = new SpoutBridgeHandle();

    if (!handle->sender.OpenDirectX11()) {
        delete handle;
        return nullptr;
    }

    if (!handle->sender.SetSenderName(name)) {
        handle->sender.CloseDirectX11();
        delete handle;
        return nullptr;
    }

    // gemelli-core::Frame is BGRA8; advertising the matching sender format
    // makes SendImage's UpdateSubresource copy land in the right channel order.
    handle->sender.SetSenderFormat(DXGI_FORMAT_B8G8R8A8_UNORM);

    return handle;
}

bool spout_bridge_send_bgra(SpoutBridgeHandle* handle,
                            const uint8_t* pixels,
                            uint32_t width,
                            uint32_t height,
                            uint32_t pitch) {
    if (!handle || !pixels || width == 0 || height == 0) {
        return false;
    }
    // Reject a caller-declared stride narrower than one packed BGRA row.
    if (static_cast<uint64_t>(pitch) < static_cast<uint64_t>(width) * 4) {
        return false;
    }

    return handle->sender.SendImage(pixels, width, height, pitch);
}

void spout_bridge_destroy(SpoutBridgeHandle* handle) {
    if (!handle) {
        return;
    }
    handle->sender.ReleaseSender();
    handle->sender.CloseDirectX11();
    delete handle;
}

} // extern "C"
```

- [ ] **Step 5: Write `crates/spout/build.rs`**

```rust
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(reason) => {
            eprintln!("crates/spout build.rs failed: {reason}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS")
        .map_err(|err| format!("CARGO_CFG_TARGET_OS is not set: {err}"))?;

    // Non-Windows builds compile the `#![cfg(target_os = "windows")]`-gated
    // empty crate (see src/lib.rs) — there is no native bridge to build.
    if target_os != "windows" {
        return Ok(());
    }

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map_err(|err| format!("CARGO_MANIFEST_DIR is not set: {err}"))?;
    let crate_dir = Path::new(&manifest_dir);
    let workspace_root = crate_dir
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| format!("{manifest_dir} has no workspace root two levels up"))?;
    let spout2 = workspace_root.join("vendor").join("Spout2");
    let spout_dx = spout2.join("SpoutDirectX").join("SpoutDX");
    let spout_gl = spout2.join("SpoutGL");

    if !spout_dx.join("SpoutDX.cpp").exists() {
        return Err(format!(
            "Spout2 SDK not found at {}. Run scripts/fetch-spout.sh first.",
            spout2.display()
        ));
    }

    println!("cargo:rerun-if-changed=cpp/spout_bridge.cpp");
    println!("cargo:rerun-if-changed=cpp/spout_bridge.h");
    println!("cargo:rerun-if-changed={}", spout2.display());

    cc::Build::new()
        .cpp(true)
        .file("cpp/spout_bridge.cpp")
        .file(spout_dx.join("SpoutDX.cpp"))
        .file(spout_gl.join("SpoutDirectX.cpp"))
        .file(spout_gl.join("SpoutSenderNames.cpp"))
        .file(spout_gl.join("SpoutFrameCount.cpp"))
        .file(spout_gl.join("SpoutUtils.cpp"))
        .file(spout_gl.join("SpoutCopy.cpp"))
        .file(spout_gl.join("SpoutSharedMemory.cpp"))
        .include(&spout_dx)
        .include(&spout_gl)
        .include("cpp")
        .flag("/EHsc")
        .flag("/std:c++17")
        .try_compile("spout_bridge")
        .map_err(|err| format!("failed to compile cpp/spout_bridge.cpp: {err}"))?;

    for lib in
        ["d3d11", "dxgi", "user32", "gdi32", "shell32", "ole32", "comdlg32", "comctl32", "shlwapi"]
    {
        println!("cargo:rustc-link-lib={lib}");
    }

    Ok(())
}
```

- [ ] **Step 6: Write `crates/spout/src/ffi.rs`**

```rust
use std::os::raw::c_char;

/// Opaque handle to the native Spout bridge (defined only in
/// `cpp/spout_bridge.cpp`). Rust never reads through it, only holds pointers.
#[repr(C)]
pub struct SpoutBridgeHandle {
    _private: [u8; 0],
}

// Edition 2024 requires FFI declaration blocks to be `unsafe extern "C"`.
unsafe extern "C" {
    pub fn spout_bridge_create(name: *const c_char) -> *mut SpoutBridgeHandle;

    /// `pixels` must point to at least `pitch * height` readable, initialized
    /// bytes. The bridge copies them before returning.
    pub fn spout_bridge_send_bgra(
        handle: *mut SpoutBridgeHandle,
        pixels: *const u8,
        width: u32,
        height: u32,
        pitch: u32,
    ) -> bool;

    pub fn spout_bridge_destroy(handle: *mut SpoutBridgeHandle);
}
```

- [ ] **Step 7: Write `crates/spout/src/lib.rs` with the failing unit test**

```rust
//! Spout DirectX publisher, Windows-only. The whole crate body is cfg-gated
//! below (not via a `[target.'cfg(...)'.dependencies]` table in downstream
//! Cargo.tomls) so release-please's Rust manifest updater — which cannot parse
//! `cfg()` target tables — can still bump this crate's version. Mirrors
//! crates/syphon.
#![cfg(target_os = "windows")]

mod ffi;

use std::ffi::CString;
use std::ptr::NonNull;

use gemelli_core::frame::Frame;
use gemelli_core::publish::{PublishError, TexturePublisher};

/// Sender-only Spout DirectX publisher. Wraps the opaque bridge handle
/// returned by `spout_bridge_create`.
pub struct SpoutPublisher {
    handle: NonNull<ffi::SpoutBridgeHandle>,
}

// SAFETY: `SpoutBridgeHandle` owns a `spoutDX` sender (its D3D11 device +
// immediate context). `SpoutPublisher` is not `Clone` and exposes no way to
// obtain a second handle to the same native object, so moving one to another
// thread (e.g. the capture/publish thread) never creates concurrent access
// from two threads at once. Mirrors SyphonPublisher.
unsafe impl Send for SpoutPublisher {}

impl SpoutPublisher {
    /// Creates a new Spout sender advertised under `server_name`.
    pub fn new(server_name: &str) -> Result<Self, PublishError> {
        let c_name = CString::new(server_name).map_err(|err| PublishError::ServerCreate {
            name: server_name.to_string(),
            reason: err.to_string(),
        })?;

        // SAFETY: `c_name` is a valid, NUL-terminated C string alive for the
        // duration of this call. `spout_bridge_create` copies the name into
        // its own sender before returning; it retains no pointer.
        let raw = unsafe { ffi::spout_bridge_create(c_name.as_ptr()) };

        let handle = NonNull::new(raw).ok_or_else(|| PublishError::ServerCreate {
            name: server_name.to_string(),
            reason: "spout_bridge_create returned a null handle".to_string(),
        })?;

        Ok(Self { handle })
    }
}

impl TexturePublisher for SpoutPublisher {
    fn publish(&mut self, frame: &Frame) -> Result<(), PublishError> {
        let pitch = frame.width().checked_mul(4).ok_or_else(|| PublishError::Publish {
            reason: format!("frame width {} overflows pitch (width * 4)", frame.width()),
        })?;

        // SAFETY: `self.handle` was created by `spout_bridge_create` in `new`
        // and is not destroyed until `Drop::drop` (which takes `&mut self`, so
        // it cannot race this `&mut self` call). `frame.data()` is exactly
        // `width * height * 4` bytes (a `Frame` invariant), so `pitch * height`
        // never reads past its end. The bridge copies the pixels into its own
        // texture before returning, so no aliasing or use-after-free results.
        let ok = unsafe {
            ffi::spout_bridge_send_bgra(
                self.handle.as_ptr(),
                frame.data().as_ptr(),
                frame.width(),
                frame.height(),
                pitch,
            )
        };

        if ok {
            Ok(())
        } else {
            Err(PublishError::Publish {
                reason: "spout_bridge_send_bgra returned false".to_string(),
            })
        }
    }
}

impl Drop for SpoutPublisher {
    fn drop(&mut self) {
        // SAFETY: `self.handle` is the handle created in `new` and has not been
        // destroyed yet — `drop` runs at most once per `SpoutPublisher` and it
        // is not `Clone`, so no other reference to it can be live concurrently.
        unsafe { ffi::spout_bridge_destroy(self.handle.as_ptr()) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_rejects_interior_nul() {
        let result = SpoutPublisher::new("bad\0name");

        assert!(matches!(result, Err(PublishError::ServerCreate { .. })));
    }

    #[test]
    #[ignore = "requires a real Windows GPU session; run manually with \
                `cargo test -p gemelli-spout -- --ignored` and observe the frame \
                in a Spout receiver (e.g. SpoutReceiver / OBS Spout2 source)"]
    fn publish_one_solid_color_frame() {
        let width = 64_u32;
        let height = 64_u32;
        let pixel = [0_u8, 0, 255, 255]; // solid red, BGRA
        let len = usize::try_from(width)
            .and_then(|w| usize::try_from(height).map(|h| w * h * 4))
            .expect("64 * 64 * 4 fits in usize");
        let data = pixel.iter().copied().cycle().take(len).collect();
        let frame = Frame::new(width, height, data).expect("valid frame");

        let mut publisher = SpoutPublisher::new("gemelli-spout-smoke").expect("sender create");
        publisher.publish(&frame).expect("publish");

        // Give a receiver a moment to observe the frame before Drop tears the
        // sender down.
        std::thread::sleep(std::time::Duration::from_secs(5));
    }
}
```

- [ ] **Step 8: Run the unit test — it must build (native bridge links) and pass**

Run: `cargo test -p gemelli-spout --lib new_rejects_interior_nul`
Expected: compiles the C++ bridge + SDK, links, and PASSES (the NUL is rejected at the `CString::new` stage before any FFI call). If the build errors with "Spout2 SDK not found", Task 1 was not run.

- [ ] **Step 9: Lint**

Run: `cargo clippy -p gemelli-spout --all-targets -- -D warnings` and `cargo fmt -p gemelli-spout -- --check`
Expected: no warnings, formatting clean.

- [ ] **Step 10: Commit**

```bash
git add crates/spout Cargo.toml
git commit -m "feat(spout): add gemelli-spout Windows publisher over spoutDX

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: Register the crate for release-please + third-party notices

**Files:**
- Modify: `release-please-config.json`
- Modify: `.release-please-manifest.json`
- Modify: `THIRD-PARTY-NOTICES`

**Interfaces:**
- Consumes: the `crates/spout` package name/version from Task 2.
- Produces: nothing code-facing; metadata only.

- [ ] **Step 1: Add the package to `release-please-config.json`**

Under `"packages"`, add after the `crates/syphon` line:
```json
    "crates/syphon": { "release-type": "rust" },
    "crates/spout": { "release-type": "rust" }
```

- [ ] **Step 2: Add the version to `.release-please-manifest.json`**

```json
  "crates/syphon": "0.2.0",
  "crates/spout": "0.2.0"
```

- [ ] **Step 3: Append the Spout2 notice to `THIRD-PARTY-NOTICES`**

Append (after the existing entries, preceded by the `====` divider line used between entries):

```
================================================================================

Spout2 SDK
https://github.com/leadedge/Spout2 (release 2.007.017)

Fetched at build time by scripts/fetch-spout.sh into vendor/Spout2/ and compiled
into the gemelli-spout crate (Windows only) via crates/spout/build.rs. The full
license text is written to vendor/Spout2/LICENSE by the fetch script; it is not
committed to this repository (vendor/Spout2/ is gitignored).

BSD 2-Clause License

Copyright (c) 2020-2024, Lynn Jarvis
All rights reserved.

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this
   list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice,
   this list of conditions and the following disclaimer in the documentation
   and/or other materials provided with the distribution.

THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS" AND
ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED
WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE LIABLE FOR
ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
(INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES;
LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON
ANY THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
(INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE OF THIS
SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
```

- [ ] **Step 4: Commit**

```bash
git add release-please-config.json .release-please-manifest.json THIRD-PARTY-NOTICES
git commit -m "build(spout): register gemelli-spout for release-please + notices

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 4: Wire `SpoutPublisher` into the CLI

**Files:**
- Modify: `crates/cli/Cargo.toml`
- Modify: `crates/cli/src/run.rs`

**Interfaces:**
- Consumes: `gemelli_spout::SpoutPublisher::new` (Task 2).
- Produces: CLI `create_publisher` returns a Spout publisher on Windows.

- [ ] **Step 1: Add the dependency to `crates/cli/Cargo.toml`**

After the `gemelli-syphon = { path = "../syphon" }` line (keep its existing explanatory comment), add:
```toml
# Same non-cfg-gated dependency pattern as gemelli-syphon (see the comment
# above): platform gating lives inside gemelli-spout's crate-wide cfg.
gemelli-spout = { path = "../spout" }
```

- [ ] **Step 2: Add the Windows arm and widen the fallback in `crates/cli/src/run.rs`**

Replace the two existing `create_publisher` definitions:
```rust
#[cfg(target_os = "macos")]
fn create_publisher(server_name: &str) -> Result<Box<dyn TexturePublisher>, CliError> {
    let publisher = gemelli_syphon::SyphonPublisher::new(server_name)?;
    Ok(Box::new(publisher))
}

#[cfg(not(target_os = "macos"))]
fn create_publisher(_server_name: &str) -> Result<Box<dyn TexturePublisher>, CliError> {
    Err(CliError::UnsupportedPlatform)
}
```
with:
```rust
#[cfg(target_os = "macos")]
fn create_publisher(server_name: &str) -> Result<Box<dyn TexturePublisher>, CliError> {
    let publisher = gemelli_syphon::SyphonPublisher::new(server_name)?;
    Ok(Box::new(publisher))
}

#[cfg(target_os = "windows")]
fn create_publisher(server_name: &str) -> Result<Box<dyn TexturePublisher>, CliError> {
    let publisher = gemelli_spout::SpoutPublisher::new(server_name)?;
    Ok(Box::new(publisher))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn create_publisher(_server_name: &str) -> Result<Box<dyn TexturePublisher>, CliError> {
    Err(CliError::UnsupportedPlatform)
}
```

- [ ] **Step 3: Widen the dead-code allow on the `UnsupportedPlatform` variant**

In the `CliError` enum, change the attribute on the `UnsupportedPlatform` variant from:
```rust
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    #[error("Syphon/Spout publishing is not supported on this platform")]
    UnsupportedPlatform,
```
to:
```rust
    // Constructed only on platforms without a native publisher; on macOS
    // (Syphon) and Windows (Spout) the constructing arm is compiled out.
    #[cfg_attr(any(target_os = "macos", target_os = "windows"), allow(dead_code))]
    #[error("Syphon/Spout publishing is not supported on this platform")]
    UnsupportedPlatform,
```

- [ ] **Step 4: Build, test, lint the CLI**

Run:
```bash
cargo test -p gemelli-cli
cargo clippy -p gemelli-cli --all-targets -- -D warnings
```
Expected: PASS with no warnings. (On Windows this now links `gemelli-spout`; the existing `unsupported_platform_message` test still compiles because the variant exists — it is just not constructed by `create_publisher` on Windows.)

- [ ] **Step 5: Commit**

```bash
git add crates/cli/Cargo.toml crates/cli/src/run.rs
git commit -m "feat(cli): publish via Spout on Windows

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 5: Wire `SpoutPublisher` into the GUI

**Files:**
- Modify: `crates/gui/Cargo.toml`
- Modify: `crates/gui/src/worker.rs`

**Interfaces:**
- Consumes: `gemelli_spout::SpoutPublisher::new` (Task 2).
- Produces: GUI `open_publisher` returns a Spout publisher on Windows.

- [ ] **Step 1: Add the dependency to `crates/gui/Cargo.toml`**

After the `gemelli-syphon = { path = "../syphon" }` line (keep its comment), add:
```toml
# Same non-cfg-gated dependency pattern as gemelli-syphon (see the comment
# above): platform gating lives inside gemelli-spout's crate-wide cfg.
gemelli-spout = { path = "../spout" }
```

- [ ] **Step 2: Add the Windows arm and widen the fallback in `crates/gui/src/worker.rs`**

Replace:
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
```
with:
```rust
#[cfg(target_os = "macos")]
fn open_publisher(server_name: &str) -> Result<Box<dyn TexturePublisher>, PublishError> {
    let publisher = gemelli_syphon::SyphonPublisher::new(server_name)?;
    Ok(Box::new(publisher))
}

#[cfg(target_os = "windows")]
fn open_publisher(server_name: &str) -> Result<Box<dyn TexturePublisher>, PublishError> {
    let publisher = gemelli_spout::SpoutPublisher::new(server_name)?;
    Ok(Box::new(publisher))
}

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
fn open_publisher(server_name: &str) -> Result<Box<dyn TexturePublisher>, PublishError> {
    Err(PublishError::ServerCreate {
        name: server_name.to_string(),
        reason: "Syphon/Spout publishing is not supported on this platform".to_string(),
    })
}
```

- [ ] **Step 3: Build, test, lint the GUI**

Run (fetch fonts first if not present — `./scripts/fetch-fonts.sh`):
```bash
cargo test -p gemelli-gui
cargo clippy -p gemelli-gui --all-targets -- -D warnings
```
Expected: PASS with no warnings.

- [ ] **Step 4: Commit**

```bash
git add crates/gui/Cargo.toml crates/gui/src/worker.rs
git commit -m "feat(gui): publish via Spout on Windows

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 6: Switch CI to a single Ubuntu job

**Files:**
- Modify: `.github/workflows/ci.yml`

**Interfaces:** none (CI config only).

- [ ] **Step 1: Replace `.github/workflows/ci.yml` with an Ubuntu-only job**

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  check:
    runs-on: ubuntu-latest
    name: Lint & Test
    steps:
      - uses: actions/checkout@v7

      - uses: dtolnay/rust-toolchain@1.96.1
        with:
          components: rustfmt, clippy

      - uses: Swatinem/rust-cache@v2

      # gemelli-core's nokhwa (input-native) needs V4L headers to build on Linux.
      - name: Install V4L dev headers
        run: sudo apt-get update && sudo apt-get install -y libv4l-dev pkg-config

      # gemelli-gui embeds LINE Seed JP via include_bytes! — see scripts/fetch-fonts.sh.
      - name: Fetch fonts
        run: ./scripts/fetch-fonts.sh

      - name: cargo fmt --check
        run: cargo fmt --all -- --check

      - name: cargo clippy
        run: cargo clippy --workspace --all-targets -- -D warnings

      # Native publishers (gemelli-syphon / gemelli-spout) are cfg-gated to
      # macOS / Windows and compile to empty stubs here; their bridges are
      # verified locally per README's "Manual verification checklist".
      # Hardware-dependent tests are #[ignore]d.
      - name: cargo test
        run: cargo test --workspace
```

- [ ] **Step 2: Commit**

```bash
git add .github/workflows/ci.yml
git commit -m "ci: run lint/test on Ubuntu; native bridges are local-only

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 7: Document Windows setup + verification in README

**Files:**
- Modify: `README.md`

**Interfaces:** none (docs only).

- [ ] **Step 1: Read the relevant README sections**

Run: open `README.md` and locate the "Setup" section (mentions `vendor/syphon-src` / `Build Syphon.framework`) and the "Manual verification checklist" section referenced by `ci.yml`/tests.

- [ ] **Step 2: Add a Windows (Spout) setup subsection**

In "Setup", add a Windows subsection mirroring the macOS one:
```markdown
### Windows (Spout)

Fetch the Spout2 SDK (compiled into `gemelli-spout` at build time):

    ./scripts/fetch-spout.sh

Requires the MSVC C++ toolchain (Visual Studio Build Tools, `x86_64-pc-windows-msvc`).
Then build/run as usual, e.g. `cargo run -p gemelli-cli -- --list-devices`.
Publishing appears as a Spout sender named by `--server-name` (default `gemelli`).
```

- [ ] **Step 3: Add a Windows row to the Manual verification checklist**

Add checklist items:
```markdown
- [ ] **Windows / Spout:** run `gemelli` against a real camera, open a Spout
  receiver (e.g. the Spout `SpoutReceiver` demo or OBS with the Spout2 source),
  and confirm the `gemelli` sender shows the webcam. Verify colours are correct
  (BGRA, not channel-swapped), the image is upright, and rotate/flip/crop/scale
  are reflected. Repeat via `gemelli-gui`.
```

- [ ] **Step 4: Commit**

```bash
git add README.md
git commit -m "docs: document Windows/Spout setup and verification

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 8: Real-hardware E2E verification (Windows)

**Files:** none (verification only; may produce a follow-up fix commit).

**Interfaces:** exercises the whole pipeline end to end.

- [ ] **Step 1: Full workspace build + lint + test on Windows**

Run:
```bash
./scripts/fetch-spout.sh
./scripts/fetch-fonts.sh
cargo build --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: all green. Native bridge compiles and links.

- [ ] **Step 2: Ensure a Spout receiver is available**

Confirm a receiver exists to observe output: the Spout `SpoutReceiver` demo (from the Spout release) or OBS Studio with a "Spout2 Capture" source. If none is installed, install one before continuing.

- [ ] **Step 3: Run the ignored GPU smoke test and observe**

Run: `cargo test -p gemelli-spout -- --ignored publish_one_solid_color_frame`
While it sleeps (5s), open the receiver and select sender `gemelli-spout-smoke`.
Expected: a solid **red** 64×64 frame (confirms BGRA channel order is correct).

- [ ] **Step 4: Run the real CLI against a webcam**

Run:
```bash
cargo run -p gemelli-cli -- --list-devices
cargo run -p gemelli-cli -- --device <N> --server-name gemelli
```
Open the receiver, select `gemelli`. Confirm: live webcam, correct colours, upright orientation, correct resolution. Then test `--rotate 90`, `--flip h`, a `--crop`, and `--scale` and confirm each is reflected. Ctrl+C exits cleanly (exit 0).

- [ ] **Step 5: Verify the GUI path**

Run: `cargo run -p gemelli-gui`. Pick the device, Start publishing, and confirm the receiver shows the frames; adjust rotate/flip/crop/scale live and confirm they propagate.

- [ ] **Step 6: If orientation is inverted, fix and re-verify**

Spout's `SendImage` does not flip; Syphon publishes `flipped:YES`. If the receiver shows the image upside-down relative to the preview, add a documented vertical row-flip in `spout_bridge_send_bgra` (copy rows bottom-to-top into a scratch buffer before `SendImage`) and re-run Steps 3–5. If orientation is already correct, record that in the checklist and change nothing.

- [ ] **Step 7: Tick the README checklist and commit any fix**

If Step 6 required a change:
```bash
git add crates/spout/cpp/spout_bridge.cpp README.md
git commit -m "fix(spout): flip frames vertically to match receiver orientation

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```
Otherwise commit only the checked-off README checklist if edited.

---

## Self-Review

**Spec coverage:**
- sender-only, CPU `SendImage` path → Task 2 (bridge + publisher). ✓
- Spout2 SDK fetched to `vendor/Spout2/` → Task 1. ✓
- crate mirrors syphon (cfg gate, `links`, non-target-gated dep) → Task 2 + Tasks 4/5. ✓
- CLI/GUI wiring with `#[cfg(target_os = "windows")]` → Tasks 4, 5. ✓
- Ubuntu-only CI → Task 6. ✓
- release-please registration → Task 3. ✓
- THIRD-PARTY-NOTICES (Spout2 BSD-2) → Task 3. ✓
- BGRA format / orientation notes → Task 2 (format) + Task 8 (orientation E2E). ✓
- TDD NUL test + ignored smoke → Task 2. ✓
- real-hardware E2E → Task 8. ✓
- README Windows setup + checklist → Task 7. ✓
- `gemelli-core` unchanged → no task touches it. ✓

**Placeholder scan:** no TBD/TODO; every code step shows full content. ✓

**Type consistency:** FFI names (`spout_bridge_create`/`spout_bridge_send_bgra`/`spout_bridge_destroy`) match between `spout_bridge.h`, `spout_bridge.cpp`, and `ffi.rs`. `SpoutPublisher::new(&str) -> Result<Self, PublishError>` matches the CLI/GUI call sites. `pitch` param name consistent across bridge/ffi. ✓
