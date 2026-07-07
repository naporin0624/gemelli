---
name: branching-modeled-state-with-match
description: Use when writing Rust that branches on an enum modeling state — a capture pipeline state, a Rotation/Flip mode, or any state-machine variant — to choose behavior or a value, or when adding a variant to such an enum. Also when reaching for an if-let chain, a let-else, matches!, or a `_` wildcard arm to consume an enum you own.
---

# Branching Modeled State With match

## Overview

When a value models state as an enum, **branch on it with `match` listing every variant, and never add a `_` wildcard arm or `..` catch-all on an enum you own.** A forgotten variant then becomes a compiler error that points at the exact `match`, not a silent runtime fallthrough — exhaustiveness checking is built into the language here, not something you have to simulate.

This skill is the *consume* side of a modeled type. **REQUIRED SUB-SKILL:** use `precise-type-modeling` to model the enum in the first place (every state a real variant, no stringly-typed status field). See also `early-return-guards` for guard clauses that short-circuit on a single variant.

## The recipe (NEW and existing code)

```rust
enum CaptureState {
    Idle,
    Capturing { frame_count: u64 },
    Paused { at_frame: u64 },
    Failed { reason: String },
}

fn status_label(state: &CaptureState) -> String {
    match state {
        CaptureState::Idle => "待機中".to_string(),
        CaptureState::Capturing { frame_count } => {
            format!("キャプチャ中（{frame_count} フレーム）")
        }
        CaptureState::Paused { at_frame } => format!("一時停止（{at_frame} フレーム目）"),
        CaptureState::Failed { reason } => format!("失敗（{reason}）"),
        // no `_` arm — every variant above is named
    }
}
```

Add a variant `Cancelled { at_frame: u64 }` to `CaptureState` and `status_label` no longer compiles: `error[E0004]: non-exhaustive patterns`, pointing straight at this `match`. Add the arm, the build goes green. That is the entire payoff, and a `_` arm anywhere in the match set silently defeats it — the compiler stops telling you where to look.

Grouping variants with `|` stays exhaustive and is fine, e.g. dispatching on `Rotation`:

```rust
enum Rotation { R0, R90, R180, R270 }

fn swaps_dimensions(rotation: Rotation) -> bool {
    match rotation {
        Rotation::R90 | Rotation::R270 => true,
        Rotation::R0 | Rotation::R180 => false,
    }
}
```

## `if let` / `let else`, and `matches!`

`if let` or `let ... else` on a multi-variant enum is only safe when the function genuinely cares about **one** variant and every other variant is uniformly "do nothing":

```rust
// OK: Idle, Paused, Failed all mean "nothing to record"
if let CaptureState::Capturing { frame_count } = state {
    telemetry.record_frame(*frame_count);
}
```

The moment two variants need *different* handling, that's dispatch, not a guard — go back to `match` with every variant named. `matches!` is for boolean predicates only (`let is_capturing = matches!(state, CaptureState::Capturing { .. });`); it discards fields and can't drive per-variant behavior, so never reach for it to pick a return value.

## The `#[non_exhaustive]` exemption

A `_` arm is required — and only there — for enums you do **not** own, when the defining crate marks them `#[non_exhaustive]` (common for hardware/format enums from capture or GPU-interop crates):

```rust
match foreign_crate::PixelFormat::from(raw) {
    foreign_crate::PixelFormat::Bgra8 => Format::Bgra8,
    foreign_crate::PixelFormat::Rgba8 => Format::Rgba8,
    _ => return Err(CaptureError::UnsupportedFormat), // required: crate can add variants without a semver break
}
```

This does not license a `_` arm on `CaptureState` or `Rotation` — those are yours, model them fully.

## Why not the alternatives

| Anti-pattern | Why it fails |
|---|---|
| `match state { X => …, _ => fallback }` on an enum you own | The wildcard absorbs every future variant — adding one compiles clean and ships silently. |
| `if let X = state { .. } else { fallback }` when other variants need distinct handling | The `else` branch conflates all other variants into one fallback with no compiler signal when a new one needs its own case. |
| `matches!(state, X | Y)` used to pick a return value | `matches!` only yields a `bool` and discards per-variant fields — it can't drive behavior, only a predicate. |
| Matching on a raw `String`/`u8` status code instead of the modeled enum | No exhaustiveness at all; typos and missing cases compile fine. Model first (`precise-type-modeling`), then match. |

## Common mistakes

- Adding a `_ => fallback` "to be safe" — it disables exhaustiveness checking entirely.
- Reaching for `if let` when two or more non-matched variants actually need different behavior — that's dispatch, use `match`.
- Copying a `#[non_exhaustive]` wildcard pattern onto your own crate's enums, where nothing forces it.
- Matching on a stringly- or numerically-tagged field instead of the modeled enum.

## Red Flags — STOP

- About to write `match state { … , _ => … }` on an enum defined in this crate → name the remaining variants instead.
- About to write `if let` where the "else" case actually needs its own logic per variant → use `match`.
- About to use `matches!` to choose a return value or side effect → use `match`; `matches!` is a predicate only.
- About to add a variant and a `_` arm swallows it with no compiler error → remove the wildcard before it ships silently.
