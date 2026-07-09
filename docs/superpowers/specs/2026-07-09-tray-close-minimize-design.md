# Tray residency & close-to-minimize — design

Date: 2026-07-09
Crate: `gemelli-gui` (Rust / eframe 0.35 / egui 0.35)

## Goal

Make the GUI a tray-resident app. Closing the window must **not** quit the app; the process
keeps running and stays reachable from a menu-bar (tray) icon. The app quits **only** from
the tray's "Quit gemelli", the macOS Dock, or Cmd+Q.

### Decided behavior

| Question | Decision |
|---|---|
| Red close (×) button | **Disabled** (greyed out) — clicking it does nothing; it can never quit |
| Minimizing the window | The native yellow minimize button (→ Dock) or the tray |
| Tray menu contents | **Minimal**: `Show gemelli` + `Quit gemelli` |
| Tray icon left-click | **Open the menu** (macOS standard status-item behavior) |
| Tray icon appearance | **App color icon** (`assets/tray-icon.png`, 44×44 downscaled from `icon.png`) |
| Dock icon / activation policy | **Unchanged (Regular)** — Dock icon stays, Dock "Quit" really quits |

### Why the × is disabled instead of intercepted (the load-bearing finding)

The obvious design — intercept the red-button close and send `CancelClose` +
`Minimized(true)` to minimize instead of quit — **cannot be made reliable in eframe**.
Verified empirically (winit 0.30.13 / eframe 0.35, driving the real window over the macOS
Accessibility API and reading eframe's own debug logs):

- eframe evaluates the root-viewport close decision every frame in
  `epi_integration::update`: if `close_requested` and the app did **not** emit `CancelClose`
  this frame, it sets `self.close = true` and the process exits.
- But `App::ui` (where our `CancelClose` would be sent) is guarded by `if is_visible` — it
  is **skipped on non-visible frames**. After the window has been minimized once, a later
  `WindowEvent::CloseRequested` is processed on a frame eframe considers not-visible, so
  `App::ui` (and the `CancelClose`) never runs, yet the close decision does → the app quits.
- Confirmed reproduction: close → minimize (canceled, OK) → restore → close again logs
  `Closing root viewport (ViewportCommand::CancelClose was not sent)` with **no** `App::ui`
  call on that frame → exit. Forcing background repaints only reduced the frequency; it did
  not eliminate it. (Electron's Cannelloni does this fine because `preventDefault` on the
  `close` event is visibility-independent — eframe has no equivalent.)

So the robust guarantee "the close button can never quit" is obtained by **disabling the
close button** via `ViewportBuilder::with_close_button(false)`. There is then no
`CloseRequested` to intercept, and no App-level close handling at all.

## Exit-path matrix (the core correctness contract)

```
Red (×) button ───────► disabled (greyed) ─────────► nothing happens; cannot quit
Yellow − button ──────► native miniaturize ────────► window minimizes to the Dock
Tray "Show gemelli" ──► Minimized(false) + Focus ──► window restored & brought to front
Tray "Quit gemelli" ──► stop_worker() → exit(0) ───► immediate, frame-independent quit
Cmd+Q / Dock → Quit ──► [NSApp terminate:] ────────► process terminates
```

Cmd+Q and the Dock's "Quit" go through muda's `PredefinedMenuItem::quit` → `[NSApp
terminate:]`; winit has no `applicationShouldTerminate:` override, so they terminate
directly (never emitting `CloseRequested`). Tray "Quit" does **not** use
`ViewportCommand::Close` (which only takes effect on a subsequent frame that may not run
while the window is backgrounded); it stops the worker for a clean camera/Syphon release
and then calls `std::process::exit(0)`.

## Architecture

```
┌─────────────── GemelliApp (eframe::App) ───────────────┐
│  main():                                                │
│   └ ViewportBuilder::with_close_button(false)           │
│        red × disabled → no CloseRequested ever fires     │
│  new():                                                 │
│   ├ build_app_menu()   … existing (About / Quit / Lic)  │
│   ├ build_tray()       … NEW tray.rs                    │
│   │     icon = tray-icon.png (RGBA), menu = [Show, Quit] │
│   └ MenuEvent::set_event_handler(forward + repaint)     │
│        forwards every muda event into our own mpsc AND  │
│        calls egui::Context::request_repaint()           │
│                                                         │
│  ui() each frame:                                       │
│   └ poll_native_events()  drain own rx once →           │
│         menu.action_for(id) | tray.action_for(id)       │
│           TrayAction::Show → Minimized(false) + Focus   │
│           TrayAction::Quit → stop_worker(); exit(0)     │
│   (no close interception — the close button is disabled) │
└─────────────────────────────────────────────────────────┘
```

## The two hazards this design exists to solve

`muda::MenuEvent::receiver()` is a **single global channel**. Both the app menu and the
tray menu emit into it. Today `menu.rs` drains that global receiver directly. Adding a
tray naively creates two problems:

1. **Event stealing** — whichever poller drains first consumes the other's events.
2. **Dead loop while minimized** — egui's `update`/`ui` may not run while the window is
   minimized, so tray menu events would queue but never be applied → tray unresponsive.

**Both are solved by one move:** register a single global
`muda::MenuEvent::set_event_handler` in `new()` that (a) forwards every event into our own
`mpsc::Sender<MenuEvent>` and (b) calls `ctx.request_repaint()`. This unifies draining to
one owned receiver (no stealing) and wakes the event loop on every tray/menu action even
while minimized. Consequence: `MenuEvent::receiver()` goes silent (the custom handler
replaces muda's default), so `menu.rs` must stop reading the global receiver — polling
moves to a single `poll_native_events` in `app.rs`.

## muda version unification (must-verify)

`tray-icon` re-exports its own `muda`. If tray-icon's bundled muda is a different
semver-incompatible version from our direct `muda 0.19.3`, there are **two** muda copies
with **two** distinct global channels — the single-handler design silently breaks.

Resolution: after adding `tray-icon`, run
`cargo tree -p gemelli-gui -i muda` and confirm exactly **one** muda version resolves.
- If unified → keep the direct `muda` dependency; `muda::MenuEvent` and
  `tray_icon::menu::MenuEvent` are the same type.
- If split → **drop the direct `muda` dependency** and use `tray_icon::menu::*` for the
  app menu too (update `menu.rs` imports). This guarantees a single muda / single channel.

**Verified**: `tray-icon 0.24.1` and the direct `muda 0.19.3` resolve to a single muda
(`cargo tree -p gemelli-gui -i muda` shows only `v0.19.3`), so the unified branch holds —
no fallback needed.

## Components (new `crates/gui/src/tray.rs`)

| Item | Responsibility | Testable |
|---|---|---|
| `enum TrayAction { Show, Quit }` | tray operation meaning as a type | — |
| `struct AppTray { _tray, show_id, quit_id }` | keeps `TrayIcon` alive (mirrors `AppMenu`) | — |
| `fn build_tray() -> Result<AppTray, TrayError>` | decode embedded PNG → build icon + menu | (needs NSApp) |
| `fn decode_icon(&[u8]) -> Result<DecodedIcon, TrayError>` | PNG bytes → RGBA + dims, pure | ✅ unit |
| `AppTray::action_for(&self, id) -> Option<TrayAction>` | map fired menu id to tray action, pure | ✅ unit |

There is **no** `app.rs` close handling: `main.rs` disables the close button with
`ViewportBuilder::with_close_button(false)`, so no `CloseRequested` is ever produced and
`ui()` needs no close-interception logic.

`AppMenu` gains `action_for(&self, id) -> Option<MenuAction>` (wrapping the existing pure
free function) so the unified poller can offer each drained event to both menu and tray.

## GemelliApp state additions

```rust
tray: Option<AppTray>,                 // None if build_tray() failed at startup (app still runs)
events_rx: mpsc::Receiver<muda::MenuEvent>,  // owned drain point (replaces global receiver)
```

No `wants_quit` flag: the close path never quits, and the tray "Quit" arm exits the process
directly, so there is no cross-frame quit intent to track.

## Dependencies

- `tray-icon` — version that shares muda `0.19.x` (verify via `cargo tree`; see above).
- `png` — lightweight PNG decode for the embedded tray icon (avoids the heavy `image`
  crate and any runtime resize; the icon is pre-sized to 44×44 at build time via `sips`).

## Assets

- `crates/gui/assets/tray-icon.png` — 44×44 RGBA, downscaled from the existing
  1024×1024 `icon.png` (44px = macOS retina menu-bar 22pt). Committed as a tracked asset;
  embedded with `include_bytes!`.
- `crates/gui/assets/icon.png` — the 1024×1024 source, also committed (currently untracked).

## Error handling

- `build_tray` returns `Result<_, TrayError>` (thiserror over `png::DecodingError`,
  `tray_icon::BadIcon`, `muda::Error`). On failure `new()` logs to stderr and stores
  `tray: None` — the app runs without a tray rather than crashing (mirrors the existing
  `menu: None` degradation).
- Clippy `unwrap_used` / `expect_used` / `as_conversions` remain denied — `decode_icon`
  uses `?` throughout; any width/height cast uses the project's sanctioned conversion
  helpers, not `as`.

## Testing (TDD, t-wada)

Unit tests (pure, no NSApp / no event loop):
- `decode_icon`: embedded asset decodes to 44×44, RGBA buffer length `44*44*4`.
- `AppTray::action_for`: show id → `Show`, quit id → `Quit`, foreign id → `None`.
- `AppMenu::action_for`: unchanged coverage, licenses id → `OpenLicenses`.

The disabled close button and the tray "Quit" `process::exit` arm depend on the eframe
event loop / macOS window state, so they are covered by the manual verification gates below
rather than unit tests.

## Verification gates (post-implementation, `/verify` + manual smoke on real app)

1. Red (×) is greyed out; clicking it repeatedly does nothing — the process keeps running
   and the tray icon stays present. (Verified over the Accessibility API:
   `AXCloseButton enabled=false`, survives repeated `AXPress`.)
2. Yellow − button minimizes the window to the Dock.
3. Cmd+Q **and** Dock right-click → Quit → app actually terminates.
4. Tray "Quit gemelli" → app terminates (worker stopped).
5. Tray "Show gemelli" **while minimized** → window restores and comes to front
   (proves the request_repaint wake works while minimized).
6. `cargo fmt --check`, full `cargo test`, `cargo clippy -D warnings` all green.

## Out of scope (YAGNI)

- Start/Stop publishing from the tray (chose minimal menu).
- Monochrome/template tray icon (chose color).
- Left-click toggling the window (chose menu-on-click).
- Hiding the Dock icon / `LSUIElement` accessory policy (Dock must stay).
