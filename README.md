# gemelli

A small CLI (and, later, GUI) tool that captures a webcam feed, applies rotate / flip / crop /
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

4. Build the workspace:
   ```bash
   cargo build --workspace
   ```

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
| `--fps <N>` | integer | camera default | Requested capture fps (best-effort) |

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

## Manual verification checklist

The automatable parts of this checklist (build/test/clippy/fmt, `--list-devices`, a timed run
with transforms + SIGINT, and both negative paths) are re-run as part of every release; the
visual step requires a human at a machine with a Syphon client installed and is **not**
automated.

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

## License

MIT — see workspace `Cargo.toml` (`license = "MIT"`).

This project vendors [Syphon-Framework](https://github.com/Syphon/Syphon-Framework) (via the
`vendor/syphon-src` git submodule) as a build-time dependency of `gemelli-syphon`.
Syphon-Framework is distributed under a BSD-style license; the full text is reproduced in
[`THIRD-PARTY-NOTICES`](./THIRD-PARTY-NOTICES) and ships with the submodule at
`vendor/syphon-src/LICENSE`. No Syphon-Framework source is copied into this repository — it is
fetched and built locally per the Setup steps above.
