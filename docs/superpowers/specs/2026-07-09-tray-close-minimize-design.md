# Tray residency & close-to-minimize — design

Date: 2026-07-09
Crate: `gemelli-gui` (Rust / eframe 0.35 / egui 0.35)

## Goal

Make the GUI a tray-resident app. Pressing the window's red close button (×) must
**not** quit the app — it minimizes the window to the Dock while the process keeps
running, reachable from a menu-bar (tray) icon. The app quits **only** when the user
explicitly chooses Quit from the tray menu, or quits via the macOS Dock / Cmd+Q.

### Decided behavior (from stakeholder Q&A)

| Question | Decision |
|---|---|
| Close (×) button | **Minimize to Dock** (`ViewportCommand::Minimized(true)`), app keeps running |
| Tray menu contents | **Minimal**: `Show gemelli` + `Quit gemelli` |
| Tray icon left-click | **Open the menu** (macOS standard status-item behavior) |
| Tray icon appearance | **App color icon** (`assets/tray-icon.png`, 44×44 downscaled from `icon.png`) |
| Dock icon / activation policy | **Unchanged (Regular)** — Dock icon stays, Dock "Quit" really quits |

## Exit-path matrix (the core correctness contract)

```
Red (×) button ───► WindowEvent::CloseRequested ─► intercept ─► CancelClose + Minimized(true)
Cmd+Q / Dock Quit ─► NSApp terminate: ───────────► process exits (never hits close_requested)
Tray "Quit gemelli" ► wants_quit = true ─► ViewportCommand::Close ─► graceful eframe exit
Tray "Show gemelli" ► Minimized(false) + Focus ─► window restored & brought to front
```

Rationale: on winit 0.30 / eframe 0.35 (macOS), only the red-button close emits
`WindowEvent::CloseRequested`. Cmd+Q and the Dock's "Quit" go through the standard
`[NSApp terminate:]` path (muda's `PredefinedMenuItem::quit`) and terminate the process
directly, bypassing the close-request flow. So intercepting `close_requested` catches the
red button **only** — the Quit paths keep working with no extra code. This assumption is a
verification gate (below), not a guarantee.

## Architecture

```
┌─────────────── GemelliApp (eframe::App) ───────────────┐
│  new():                                                 │
│   ├ build_app_menu()   … existing (About / Quit / Lic)  │
│   ├ build_tray()       … NEW tray.rs                    │
│   │     icon = tray-icon.png (RGBA), menu = [Show, Quit] │
│   └ MenuEvent::set_event_handler(forward + repaint)     │
│        forwards every muda event into our own mpsc AND  │
│        calls egui::Context::request_repaint()           │
│                                                         │
│  ui() each frame:                                       │
│   ├ poll_native_events()  drain own rx once →           │
│   │     menu.action_for(id) | tray.action_for(id)       │
│   │       TrayAction::Show → Minimized(false) + Focus   │
│   │       TrayAction::Quit → wants_quit = true          │
│   └ handle_close()  decide_close(close_requested,       │
│         wants_quit):                                     │
│           Minimize → CancelClose + Minimized(true)      │
│           Quit     → send Close (let eframe exit)       │
│           Ignore   → nothing                            │
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

## Components (new `crates/gui/src/tray.rs`)

| Item | Responsibility | Testable |
|---|---|---|
| `enum TrayAction { Show, Quit }` | tray operation meaning as a type | — |
| `struct AppTray { _tray, show_id, quit_id }` | keeps `TrayIcon` alive (mirrors `AppMenu`) | — |
| `fn build_tray() -> Result<AppTray, TrayError>` | decode embedded PNG → build icon + menu | (needs NSApp) |
| `fn decode_icon(&[u8]) -> Result<DecodedIcon, TrayError>` | PNG bytes → RGBA + dims, pure | ✅ unit |
| `AppTray::action_for(&self, id) -> Option<TrayAction>` | map fired menu id to tray action, pure | ✅ unit |

New pure helper in `app.rs`:

| Item | Responsibility | Testable |
|---|---|---|
| `enum CloseDecision { Ignore, Minimize, Quit }` | outcome of a close evaluation | — |
| `fn decide_close(close_requested: bool, wants_quit: bool) -> CloseDecision` | pure branch logic | ✅ unit (4 combos) |

`AppMenu` gains `action_for(&self, id) -> Option<MenuAction>` (wrapping the existing pure
free function) so the unified poller can offer each drained event to both menu and tray.

## GemelliApp state additions

```rust
tray: Option<AppTray>,                 // None if build_tray() failed at startup (app still runs)
events_rx: mpsc::Receiver<muda::MenuEvent>,  // owned drain point (replaces global receiver)
wants_quit: bool,                      // set by tray Quit; gates decide_close
```

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
- `decide_close`: `(false,*) → Ignore`, `(true,false) → Minimize`, `(true,true) → Quit`.
- `decode_icon`: embedded asset decodes to 44×44, RGBA buffer length `44*44*4`.
- `AppTray::action_for`: show id → `Show`, quit id → `Quit`, foreign id → `None`.
- `AppMenu::action_for`: unchanged coverage, licenses id → `OpenLicenses`.

## Verification gates (post-implementation, `/verify` + manual smoke on real app)

1. Red (×) → window minimizes to Dock, process still running, tray icon present.
2. Cmd+Q **and** Dock right-click → Quit → app actually terminates.
3. Tray "Quit gemelli" → app terminates (worker stopped).
4. Tray "Show gemelli" **while minimized** → window restores and comes to front
   (proves the request_repaint wake works while minimized).
5. `cargo fmt --check`, full `cargo test`, `cargo clippy -D warnings` all green.

## Out of scope (YAGNI)

- Start/Stop publishing from the tray (chose minimal menu).
- Monochrome/template tray icon (chose color).
- Left-click toggling the window (chose menu-on-click).
- Hiding the Dock icon / `LSUIElement` accessory policy (Dock must stay).
