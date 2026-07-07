---
name: precise-type-modeling
description: Use when authoring or converting Rust types in this webcam-to-Syphon/Spout tool — defining a struct, enum, or function signature for capture devices, frames, rotation/crop/scale options, or pipeline state; typing several correlated fields as Option<T>; or about to reach for String, serde_json::Value, HashMap<String, String>, an over-permissive Vec<u8>, an `as` cast, or unwrap()/expect() to avoid modeling a value.
---

# Precise Type Modeling

## Overview

A type must model reality **exactly**: every argument and every state, no looser than the truth. Stringly-typed fields, bare `serde_json::Value`, `HashMap<String, String>` for owned data, over-permissive `Vec<u8>` blobs, `as` casts, and `unwrap()`/`expect()` are where models go to die — each says "I gave up modeling here." The goal is not "it compiles" but "impossible states are unrepresentable," which is also why clippy denies `unwrap_used`, `expect_used`, and `as_conversions` here.

## The rules (minimum bar)

1. **No escape-hatch types for data you control.** Ban `String`/stringly-typed values, `serde_json::Value`, and `HashMap<String, String>` for "arbitrary" device options, flags, or metadata — "arbitrary" almost always means "I didn't enumerate it yet." Enumerate the fields in a struct, or model the variants as an `enum`.
2. **Untyped data only at a true external boundary, and only in transit.** Raw JSON from a device driver, bytes from a capture SDK, or env vars may arrive untyped — parse it immediately into a concrete type (`serde::Deserialize`, `TryFrom`, a `parse_*` function), chaining with `?`/`.map_err` rather than nested matches (see `chaining-result-combinators`). `serde_json::Value`/`Vec<u8>` must never rest in a field or return type — it's a doorway, not a room.
3. **Model every state as an enum — the enum IS the state-transition diagram.** N possible states = N variants, each carrying only that state's data, so illegal field access (reading a delivered frame off a failed capture) is a compile error. Consuming it exhaustively via `match` is the job of `branching-modeled-state-with-match` — reference it, don't re-derive it here.
4. **Optional fields are a smell — usually a collapsed enum.** If `retry_after_ms` is only `Some` when capture failed, and `syphon_slot` is only `Some` when it succeeded, that's two variants, not two `Option<T>` fields on one struct. Reserve `Option<T>` for a field that is genuinely, independently absent (`device_label: Option<String>`), not for state.
5. **Compiler-checked construction over `as` casts or `unsafe` transmutes.** Build values with type-annotated `let` bindings and struct/enum literals so the compiler checks every field. Numeric narrowing/widening belongs to `explicit-primitive-conversion` — follow that skill, don't duplicate it here.
6. **Newtype wrap primitive units and IDs.** `DeviceIndex(u32)`, `PixelX(u32)` vs `NormalizedX(f32)` — when two arguments share a primitive type, a caller can swap them and the compiler stays silent. A newtype per unit turns that mix-up into a compile error instead of a rotated, cropped, or misindexed frame.

## Before / after

### Optionals hiding a state machine → enum (rules 3, 4)

```rust
// ❌ every field optional; nothing stops reading `.frame` on a failed capture
struct CaptureOutcome {
    ok: bool,
    frame: Option<Frame>, syphon_slot: Option<SyphonSlot>,   // success only
    error: Option<CaptureError>, retry_after_ms: Option<u32>, // transient failure only
    device_index: Option<DeviceIndex>, supported: Option<Vec<PixelFormat>>, // format mismatch only
}
```

```rust
// ✅ the enum names each state; each variant carries ONLY its own data
enum CaptureOutcome {
    Delivered { frame: Frame, syphon_slot: SyphonSlot },
    TransientFailure { error: CaptureError, retry_after_ms: u32 },
    FormatMismatch { device_index: DeviceIndex, supported: Vec<PixelFormat> },
}

fn summarize(outcome: &CaptureOutcome) -> String {
    match outcome {
        CaptureOutcome::Delivered { frame, .. } =>
            format!("delivered {}x{} frame", frame.width, frame.height),
        CaptureOutcome::TransientFailure { error, retry_after_ms } =>
            format!("failed ({error:?}), retry in {retry_after_ms}ms"),
        CaptureOutcome::FormatMismatch { device_index, supported } =>
            format!("device {device_index:?} needs one of {supported:?}"),
        // add a 4th variant -> this match fails to compile until handled
    }
}
```

### "Arbitrary pass-through" → enumerated concrete type (rules 1, 2)

```rust
// ❌ HashMap<String, String> is a giveaway: the pipeline flags were never modeled
struct CaptureOptions {
    client_label: Option<String>,
    require_gpu_upload: bool,
    flags: HashMap<String, String>,   // "callers can pass anything"
}
```

```rust
// ✅ name the flags; a new flag is a one-line type change, reviewable at compile time
struct PipelineFlags {
    denoise_enabled: bool,
    target_fps: u32,
}
struct CaptureOptions {
    client_label: Option<String>,
    require_gpu_upload: bool,
    flags: PipelineFlags,
}
```

Parse driver replies straight into the model at the boundary — never store the raw `Value`: `let devices: DeviceList = serde_json::from_str(&raw_reply)?;`

### Newtypes over bare primitives (rule 6)

```rust
// ❌ four u32 args in the same position are trivially swappable
fn crop(x: u32, y: u32, width: u32, height: u32) { /* ... */ }

fn call_site(x: u32, y: u32, width: u32, height: u32) {
    crop(height, width, x, y); // compiles, silently crops the wrong region
}
```

```rust
// ✅ newtypes make the mix-up a compile error, not a corrupted frame
struct PixelX(u32);
struct PixelY(u32);
struct PixelWidth(u32);
struct PixelHeight(u32);
fn crop(x: PixelX, y: PixelY, width: PixelWidth, height: PixelHeight) { /* ... */ }
```

## Quick reference

| You're about to write | Do instead |
|---|---|
| `String`/stringly-typed field for data you own | model it: enum or struct; if truly external, parse at the boundary |
| `serde_json::Value`/`HashMap<String, String>` as a field or return type | enumerate fields as concrete variants; parse via `Deserialize`/`TryFrom` first |
| struct with many `Option<T>` fields | split into an `enum` by state |
| `bool` flag + optional payload | one variant per state, payload non-optional inside it |
| `x as T` | a checked conversion — see `explicit-primitive-conversion` |
| two same-typed positional args (`u32, u32, ...`) | newtypes per unit/axis |
| `unwrap()`/`expect()` to "make it compile" | model the failure as a variant, or return `Result` |

## Common rationalizations

| Excuse | Reality |
|---|---|
| "The flags are arbitrary, I need `HashMap<String, String>`" | "Arbitrary" = unenumerated. List them; add a field when a flag is born. |
| "Optionals are simpler than an enum" | They push every `if let Some` check onto every reader, forever. `match` checks once. |
| "`as` is fine, I know the range" | `TryFrom` proves it for the compiler; `as` truncates silently the day you're wrong. |
| "`serde_json::Value` is the safe version of untyped data" | Only if narrowed immediately — as a resting field it's untyped data with extra steps. |
| "I'll model it properly later" | The `Option`-soup struct ships and never gets revisited. Model it now. |

## Red Flags — STOP

- Reaching for `String`, `serde_json::Value`, `HashMap<String, String>`, or `Vec<u8>` for "config"/"options"/"metadata"/"payload" you actually control.
- A struct where most fields are `Option<T>`, or a `bool`/`status` field that decides which *other* fields are meaningful — both are a discriminant; make it an enum.
- An `as` cast on anything other than a range already proven at compile time, or `unwrap()`/`expect()` papering over a state that should be its own variant — clippy denies both anyway.
- Two same-typed primitive parameters that could be transposed without a compile error.
