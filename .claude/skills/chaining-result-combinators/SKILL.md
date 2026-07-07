---
name: chaining-result-combinators
description: Use when composing or chaining Rust Result values — sequencing multi-step fallible operations, tempted by unwrap()/expect()/panic! on an expected failure, matching a Result mid-pipeline just to rewrap it in Ok/Err, discarding a Result with let _ = ..., or deciding where a Result finally becomes a process exit code or a rendered UI error.
---

# Chaining Result combinators

## Overview

Once a value is a `Result`, **keep it one.** Compose every step with combinators (`.and_then` / `.map` / `.map_err` / `.or_else` / `.inspect_err`) or the `?` operator, and collapse exactly once, at the consumption edge, into the outside-world value — a process exit code, a GUI error dialog, a log line. The error channel stays the crate's typed enum the whole way; the edge is where — and the only place where — it collapses.

`crates/core` never collapses: it returns `Result<T, CoreError>` outward and lets `crates/cli` / `crates/gui` decide what a failure means to a human. **RELATED:** `precise-type-modeling` owns the error enum design itself (thiserror variants, `#[from]`); `branching-modeled-state-with-match` owns the exhaustive match at the edge; `early-return-guards` owns validating preconditions before the fallible chain starts.

## The combinators (the whole vocabulary)

| Combinator | Use for |
|---|---|
| `?` | Inside a function returning `Result`: propagate the next fallible step straight-line, converting via `From`/`#[from]`. The idiomatic backbone — prefer it over `.and_then` in a function body. |
| `.and_then(fn)` | Next step that can itself fail, as an expression (`fn` returns a `Result`). Short-circuits on `Err`. Use when chaining at expression level. |
| `.map(fn)` | Transform the success value (cannot fail). |
| `.map_err(fn)` | Normalize the error — e.g. into the crate's `CoreError` enum via `CoreError::from` so the final match is exhaustive. |
| `.or_else(fn)` | **Recover** from an error: return `Ok(fallback)` for the variant you handle, `Err(e)` to re-propagate the rest. |
| `.inspect(fn)` / `.inspect_err(fn)` | Fire a side-effect (log, notify) without changing the value — the analog of `andTee`/`tap`. |

Both keep the value a `Result`; pick `?` for linear propagation inside a `Result`-returning function, combinators when transforming or normalizing at expression level. Either way the banned move is the same: collapsing to a bare value before the edge.

## The recipe

Opening a webcam device, configuring format, starting capture, publishing to Syphon — each step can fail; the chain never unwraps mid-flow:

```rust
// device.rs — crate: core (std-only sketch; real code uses thiserror's #[from])
pub enum CoreError {
    DeviceOpen(String),
    FormatUnsupported(String),
    CaptureStart(String),
    Publish(String),
}

pub struct Device;
pub struct Format;
pub struct Capture;
pub struct SyphonSender;

pub fn open_device(name: &str) -> Result<Device, CoreError> { /* .. */ Ok(Device) }
pub fn configure_format(dev: Device, fmt: &Format) -> Result<Device, CoreError> { /* .. */ Ok(dev) }
pub fn start_capture(dev: Device) -> Result<Capture, CoreError> { /* .. */ Ok(Capture) }
pub fn publish_to_syphon(cap: &Capture) -> Result<SyphonSender, CoreError> { /* .. */ Ok(SyphonSender) }
fn log_started(_cap: &Capture) { /* metrics/logging side-effect */ }

// Expression-chain style: never unwrap mid-flow.
pub fn run_pipeline(name: &str, fmt: &Format) -> Result<SyphonSender, CoreError> {
    open_device(name)
        .and_then(|dev| configure_format(dev, fmt))
        .map_err(|e| {
            // recover the one variant that needs it; every other variant passes through unchanged
            if let CoreError::FormatUnsupported(_) = &e {
                return CoreError::FormatUnsupported(format!("falling back to default format: {e}"));
            }
            e
        })
        .and_then(start_capture)
        .inspect(log_started)
        .and_then(|cap| publish_to_syphon(&cap))
}

// `?`-style: equivalent, idiomatic inside a function body returning Result.
pub fn run_pipeline_qm(name: &str, fmt: &Format) -> Result<SyphonSender, CoreError> {
    let dev = open_device(name)?;
    let dev = configure_format(dev, fmt)?;
    let cap = start_capture(dev)?;
    log_started(&cap);
    publish_to_syphon(&cap)
}
```

Consume once, at the edge — `crates/cli`'s `main`:

```rust
use std::process::ExitCode;

fn main() -> ExitCode {
    match run_pipeline("Default Camera", &Format::default()) {
        Ok(_sender) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}
```

In `crates/gui`, the analogous collapse point is wherever the app renders the error to the user (a toast, a status panel) — never deeper in the render/event loop.

## Where the edge is

The collapse belongs only where the `Result` leaves the crate for the outside world: `crates/cli`'s `main` (exit code), `crates/gui`'s error-display point, or a test assertion (`#[cfg(test)]` may `.unwrap()`/`.expect()` freely — a failing assertion is the point). `crates/core` — device access, format negotiation, Syphon/Spout publishing — returns `Result<T, CoreError>` onward and never prints, exits, or panics on an expected failure.

## Anti-patterns

| Instead of | Do |
|---|---|
| `match r { Ok(v) => .., Err(e) => .. }` mid-pipeline just to rewrap in `Ok`/`Err` | `.and_then` (success path) / `.or_else` (recovery) |
| `.unwrap()` / `.expect("...")` on an expected failure (device missing, format rejected) | Carry the `Result`; collapse only at the edge. Clippy's `unwrap_used`/`expect_used` deny this outside tests. |
| `let _ = start_capture(dev);` silently discarding a fallible call | `.and_then` / `?` — handle or propagate, never drop |
| `if r.is_err() { return Err(...) } let v = r.unwrap();` mid-flow | `?` or `.and_then` |
| `match panicking_api() { ... }` wrapping a foreign/io call mid-pipeline | `.map_err(CoreError::from)` or a `#[from]` variant, once, at the wrap point |
| Hand-rolled `as` cast to coerce an error/value across a boundary | A named conversion (`From`/`TryFrom`) — `as_conversions` is denied for a reason |

## Red Flags — STOP

- About to write `match` on a `Result` somewhere that is not the consumption edge → use `.and_then`/`.or_else`, or `?` if you're inside a `Result`-returning function.
- About to call `.unwrap()` or `.expect(...)` outside `#[cfg(test)]` → that's the compiler telling you the failure is unhandled, not that it can't happen.
- About to write `let _ = fallible_call();` → the `Result` is telling you something; handle it or propagate with `?`.
- About to reach for `panic!`/`unreachable!` on a value that came from I/O, a device, or user input → model it as a `CoreError` variant instead.
- A `match`/`if let Err` wrapping code that already returns a `Result`, just to re-throw the same error → let `?` or `.map_err` do it.
