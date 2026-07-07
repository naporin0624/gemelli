---
name: explicit-primitive-conversion
description: Use when converting between numeric types in Rust, converting string↔number, or about to write `as` — `x as u32`, usize↔u32 index/pixel math, f64→u32 frame-dimension math, or formatting a value into a String.
---

# Explicit Primitive Conversion

## Overview

The `as` cast hides two different failure modes behind one keyword. Float→int `as` **saturates**: `300.0_f64 as u8` is `255`, and `f64::NAN as u8` is `0` — not a panic, not a wraparound, a silent clamp. Int→int narrowing `as` **truncates to the low bits**: `300_i64 as u8` is `44`, and `-1_i32 as u32` is `4294967295` — the sign is reinterpreted, not preserved. Both are legal Rust and exactly what `clippy::as_conversions` and the `clippy::cast_possible_truncation` family already deny in this workspace. Those lints stop you at the call site; they cannot tell you **which** explicit conversion says what you meant. That choice is this skill.

**`as` for numeric/string conversion is banned outside a marked FFI/bit-manipulation boundary. No exceptions.**

**RELATED:** a value that was never modeled as a concrete type in the first place (a bare `String`/`serde_json::Value` standing in for a number) isn't this skill's job to fix — model it with `precise-type-modeling` first, then convert the modeled value explicitly.

## The rule

| Goal | ❌ banned (implicit/lossy) | ✅ required (explicit) |
|---|---|---|
| widen a number (lossless) | `x as i64`, `x as f64` | `i64::from(x)` / `x.into()` |
| narrow a number (fallible) | `len as u32`, `index as u16` | `u32::try_from(len)?` — handle or propagate the error |
| string → number | `s.parse().unwrap()` | `s.parse::<u32>()?` (turbofish or `let x: u32 = ...?`), error handled |
| number/value → string | ad-hoc `format!("{}", frame)` at every call site | `frame.to_string()`, or `impl Display for Frame` once |
| float → int pixel math | `(width as f64 * scale) as u32` inline | name the rounding — `.round()`/`.floor()`/`.trunc()` — then convert in one named, audited helper |
| bool from a count/flag | `(has_frame as u8) == 1`, `count as bool`-style tricks | a real comparison: `count != 0`, `frame_index.is_some()` |

**Still allowed:** `as` inside a dedicated FFI/bit-manipulation module — e.g. the Syphon/Metal texture-handle or Spout/DXGI shared-surface boundary — behind `#[allow(clippy::as_conversions)]` with a comment stating the invariant that makes it safe. Also allowed: the `as` that std structurally requires for `f64 → i64` (there is no `TryFrom<f64>` in std), *if* it is isolated in a single named helper directly after `.round()`, not repeated inline. Provably-lossless platform casts like `usize as u64` should still prefer `u64::try_from(x)` — "provably lossless" is an assumption a future platform can break; `TryFrom` cannot be wrong.

## Before → after

```rust
// ❌ before — every conversion hides its failure mode
fn scaled_frame_size(width: usize, height: usize, scale: f64) -> (u32, u32) {
    let w = (width as f64 * scale) as u32;   // truncates toward zero, silently
    let h = (height as f64 * scale) as u32;  // a negative scale would wrap, not error
    (w, h)
}

fn parse_fps(arg: &str) -> u32 {
    arg.parse().unwrap()                     // panics on "30fps", "-5", or ""
}

fn frame_at(raw_index: i64) -> u32 {
    raw_index as u32                         // negative raw_index becomes a huge index
}
```

```rust
// ✅ after — the rounding, the fallibility, and the range are all named
fn scale_dimension(len: usize, scale: f64) -> Result<u32, TryFromIntError> {
    // std has no TryFrom<usize> for f64 or TryFrom<f64> for u32; both casts
    // are audited here, once, right after `.round()` removes the fraction.
    let scaled = (len as f64 * scale).round();
    u32::try_from(scaled as i64)
}

fn scaled_frame_size(width: usize, height: usize, scale: f64) -> Result<(u32, u32), TryFromIntError> {
    let w = scale_dimension(width, scale)?;
    let h = scale_dimension(height, scale)?;
    Ok((w, h))
}

fn parse_fps(arg: &str) -> Result<u32, ParseIntError> {
    arg.parse::<u32>()                       // turbofish names the target; caller handles Err
}

fn frame_at(raw_index: i64) -> Result<u32, TryFromIntError> {
    u32::try_from(raw_index)                 // negative raw_index is now a named error
}
```

## Why int→int `as` is the one that hides bugs

`as` between integer types keeps the low bits and reinterprets the sign — it never asks permission. A frame counter that overflows `u32`, or a crop offset that went negative three call sites away, becomes a wrong-but-plausible number instead of a caught error. `try_from`/`try_into` turns the same bug into a `Result` at the exact line it happens:

- "this index fits in a smaller counter" → `u32::try_from(index)?`, not `index as u32`
- "this f64 is a valid pixel count" → round it, then `try_from`, never a bare cast
- "this offset should never be negative" → `try_from` returns `Err`; `as` returns a wrong `u32`

## Common mistakes

| Mistake | Fix |
|---|---|
| `x as u32` "because the value is always in range" | Prove it: `u32::try_from(x)?` and propagate, or `.expect("documented invariant: ...")` at worst. `try_from` costs nothing on the happy path. |
| `(w as f64 * scale) as u32` inline in business logic | Isolate the float→int cast in one named, commented helper; call the helper, never repeat the cast. |
| `arg.parse().unwrap()` on a CLI flag | `arg.parse::<u32>()` and propagate/handle `ParseIntError` — CLI args are untrusted input, same as a network payload. |
| `format!("{}", my_type)` scattered at every call site | `impl Display for MyType` once; call `.to_string()` or `{}` through the trait everywhere else. |
| `#[allow(clippy::as_conversions)]` on a whole file "to unblock the build" | Scope the allow to the one function/module that needs it, with a comment naming the invariant. |

## Red Flags — STOP

- About to write `as` between two numeric types outside an FFI/bit-manipulation module → widening or narrowing? Use `From` or `TryFrom`, not `as`.
- About to write `.unwrap()` after `.parse()` → is the input a literal (infallible), or untrusted — a CLI arg, file, or network value? Untrusted needs `Result` handling.
- About to write `(x as f64 * y) as u32` or any float math ending in `as <int>` → name the rounding, then push the cast into one audited helper.
- About to compare a numeric value to `0`/`1` to fake a boolean (`x as u8 == 1`) → write the comparison directly; it already returns `bool`.
- About to widen `#[allow(clippy::as_conversions)]` beyond a single FFI/bit-manipulation boundary → narrow the scope and write the invariant in a comment.

`clippy::as_conversions` and the `clippy::cast_possible_truncation`/`cast_sign_loss`/`cast_precision_loss` family already deny the lossy call sites; this skill exists for the part they can't check — *which explicit conversion (`From`, `TryFrom`, `parse`, `to_string`, a named rounding helper) expresses the intent*.
