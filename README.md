# gemelli

A small CLI and GUI tool that captures a webcam feed, applies rotate / flip / crop /
scale transforms, and publishes the result as a shared GPU texture — Syphon on macOS today, with
Spout on Windows defined as a trait only (not yet implemented). Sister tool of
[ravioli](https://github.com/naporin0624/ravioli).

## Setup

1. Install the toolchain via [mise](https://mise.jdx.dev/):
   ```bash
   mise install
   ```
   This pins the Rust, Node, and pnpm versions declared in `mise.toml`.

2. Install dev tooling (husky pre-commit hook: `cargo fmt --check` + `cargo clippy -D warnings`
   + `cargo test`):
   ```bash
   pnpm install
   ```

3. Build the Syphon bridge (macOS only — required to build `gemelli-syphon` / run the CLI's
   publish step). `crates/syphon` links against Apple's Syphon.framework, which is not
   vendored/prebuilt — it's built locally from a git submodule the first time you set up the
   repo:
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
   ```
   `vendor/syphon-src` (the submodule source) and `vendor/Syphon.framework` (the build output)
   are both gitignored except for the submodule pointer in `.gitmodules` — every clone rebuilds
   the framework locally rather than committing a binary.

   If Gatekeeper quarantines the copied framework and Syphon servers silently fail to appear to
   clients, clear it: `xattr -dr com.apple.quarantine vendor/Syphon.framework`.

4. Fetch the LINE Seed JP font (required to build `gemelli-gui` — the font file is embedded
   into the binary at compile time via `include_bytes!`, so `cargo build -p gemelli-gui` and
   `cargo build --workspace` fail without it):
   ```bash
   ./scripts/fetch-fonts.sh
   ```
   This downloads LINE Seed JP from the official [line/seed](https://github.com/line/seed)
   release into `vendor/fonts/` (gitignored, like the Syphon build output above). The font
   exists so the GUI can render Japanese camera device names (e.g. built-in cameras on a
   Japanese-locale macOS) — the UI itself is English. LINE Seed JP is licensed under the SIL
   Open Font License 1.1; see [`THIRD-PARTY-NOTICES`](./THIRD-PARTY-NOTICES), and the full
   license text lands at `vendor/fonts/LICENSE` alongside the font.

5. Build the workspace:
   ```bash
   cargo build --workspace
   ```

### Windows (Spout)

Fetch the Spout2 SDK (compiled into `gemelli-spout` at build time):

    ./scripts/fetch-spout.sh

Requires the MSVC C++ toolchain (Visual Studio Build Tools, `x86_64-pc-windows-msvc`).
Then build/run as usual, e.g. `cargo run -p gemelli-cli -- --list-devices`.
Publishing appears as a Spout sender named by `--server-name` (default `gemelli`).

## CLI usage

```
gemelli [DEVICE_INDEX] [OPTIONS]
```

| Option | Values | Default | Description |
|---|---|---|---|
| `DEVICE_INDEX` | integer | interactive prompt (TTY) / error (non-TTY) | Camera device index; see `--list-devices` |
| `--list-devices` | flag | — | List available cameras as `{index}: {name}` and exit |
| `--rotate <N>` | `0`, `90`, `180`, `270` | `0` | Clockwise rotation |
| `--flip <F>` | `h`, `v`, `hv` | no flip | Mirror horizontally, vertically, or both |
| `--crop <SPEC>` | `WxH+X+Y`, e.g. `1280x720+320+180` | no crop | Crop before any other transform |
| `--scale <SPEC>` | `WxH` or a positive factor, e.g. `960x540` or `0.5` | no scale | Resize, applied last |
| `--server-name <NAME>` | string | `gemelli` | Name of the published Syphon server |
| `--fps <N>` | positive integer (`0` rejected) | none: highest resolution, then best fps at that resolution | Prefers a format that exactly matches `N` fps; falls back to the highest-resolution format if no exact match exists (best-effort) |

Transform order is always **crop → rotate → flip → scale**, regardless of the order options are
given on the command line.

Examples:
```bash
gemelli --list-devices
gemelli 0 --rotate 90 --flip h --scale 0.5
gemelli 0 --crop 1280x720+320+180 --server-name my-camera
```

Exit codes: `0` clean shutdown (including Ctrl+C) · `1` runtime error (printed to stderr) ·
`2` CLI usage error (invalid/missing argument).

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

## Manual verification checklist

The automatable parts of this checklist (build/test/clippy/fmt, `--list-devices`, a timed run
with transforms + SIGINT, a timed GUI launch, and both negative paths) are re-run as part of
every release; the visual steps require a human at a machine with a real camera and a Syphon
client installed and are **not** automated.

### CLI

- [ ] `cargo build --workspace`, `cargo test --workspace`,
      `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo fmt --all -- --check`
      all pass.
- [ ] `cargo run -p gemelli-cli -- --list-devices` prints one `{index}: {name}` line per attached
      camera.
- [ ] Open a Syphon client (e.g. Syphon Recorder or Simple Client) as the receiving app.
- [ ] Run `cargo run -p gemelli-cli -- 0 --rotate 90 --flip h --scale 0.5` and confirm in the
      client:
  - [ ] a server named `gemelli` appears in the server list,
  - [ ] the received image is rotated 90° clockwise relative to the raw camera feed,
  - [ ] the received image is horizontally mirrored,
  - [ ] the received image's resolution is half the camera's native resolution (width and height
        both halved, within nearest-neighbor rounding).
- [ ] Press Ctrl+C in the terminal running the CLI and confirm:
  - [ ] the process exits promptly (no hang) with exit code `0`,
  - [ ] the Syphon client shows the server disappearing (publisher dropped cleanly).
- [ ] `cargo run -p gemelli-cli -- < /dev/null` prints
      `error: no device specified and stdin is not a TTY` and exits `1`.
- [ ] `cargo run -p gemelli-cli -- --rotate 45` prints a clap usage error naming the valid
      rotation values and exits `2`.
- [ ] `cargo run -p gemelli-cli -- <index not in --list-devices output>` prints a clear
      "device index not found" error and exits `1`.

### GUI (real camera + Syphon Recorder)

- [ ] `cargo run -p gemelli-gui` launches, stays alive (no panic, no stderr noise), and shows
      the sidebar/preview layout with the theme applied.
- [ ] Select a real attached camera and click **Start**: the GUI preview shows the live feed,
      and Syphon Recorder shows a server named per the sidebar's "Server name" field whose image
      matches the GUI preview pixel-for-pixel (same content, same orientation).
- [ ] A camera whose device name contains Japanese characters renders correctly in the device
      combo (no tofu/placeholder glyphs — this is what the embedded LINE Seed JP font is for).
- [ ] While publishing, change rotate, flip, scale, and crop (via drag on the preview and via
      the numeric W/H/X/Y fields) one at a time: each change appears in **both** the GUI preview
      and the Syphon Recorder image at the same time, with no visible lag or tearing beyond
      normal frame latency.
- [ ] While publishing, switch to a second attached camera in the device combo (if more than one
      is available — otherwise note it as untested): the Syphon Recorder image switches to the
      new camera's feed within ~1 second, without the server disappearing from the client's
      server list.
- [ ] While publishing, edit the server name field and commit it (click away or press Tab): the
      old-named server disappears from Syphon Recorder's list and a new one under the new name
      appears, still showing the live feed.
- [ ] While publishing, physically unplug the active camera (or otherwise force it offline): the
      GUI shows an error banner within a few seconds, the status bar switches to "○ stopped",
      and the process does **not** panic or hang.
- [ ] With the banner still showing, plug the camera back in, click **Refresh**, select the
      camera, and click **Start**: publishing resumes and the Syphon client sees the server
      reappear.
- [ ] Close the GUI window: the process exits promptly (no hang) and the Syphon server
      disappears from the client's list (clean publisher drop, matching the CLI's Ctrl+C
      behavior).

### Windows / Spout

- [ ] **Windows / Spout:** run `gemelli` against a real camera, open a Spout
  receiver (e.g. the Spout `SpoutReceiver` demo or OBS with the Spout2 source),
  and confirm the `gemelli` sender shows the webcam. Verify colours are correct
  (BGRA, not channel-swapped), the image is upright, and rotate/flip/crop/scale
  are reflected. Repeat via `gemelli-gui`.

## License

MIT — see workspace `Cargo.toml` (`license = "MIT"`).

This project vendors [Syphon-Framework](https://github.com/Syphon/Syphon-Framework) (via the
`vendor/syphon-src` git submodule) as a build-time dependency of `gemelli-syphon`.
Syphon-Framework is distributed under a BSD-style license; the full text is reproduced in
[`THIRD-PARTY-NOTICES`](./THIRD-PARTY-NOTICES) and ships with the submodule at
`vendor/syphon-src/LICENSE`. No Syphon-Framework source is copied into this repository — it is
fetched and built locally per the Setup steps above.

The GUI embeds [LINE Seed JP](https://github.com/line/seed) (SIL Open Font License 1.1),
fetched at build time by `scripts/fetch-fonts.sh` — see
[`THIRD-PARTY-NOTICES`](./THIRD-PARTY-NOTICES). No font file is committed to this repository.

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
