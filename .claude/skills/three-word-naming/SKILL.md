---
name: three-word-naming
description: Use when naming or renaming a Rust function, method, struct, enum, trait, or module — especially when a name is heading past three words (ensure_capture_device_present, load_and_apply_config), contains and/or/with/if_missing, or describes several steps at once.
---

# Three-Word Naming

## Overview

Functions, methods, structs, enums, traits, and modules get **at most three words**. A name that needs a fourth word is not a naming problem — it is a scope problem: the code is doing more than one thing, or it is missing the namespace that should carry part of the name. Fix the scope, and the short name falls out.

**Never fix a too-long name by abbreviating, dropping vowels, or fusing words. Shorten the *responsibility*, not the spelling.**

## Counting

- Split snake_case on `_`, CamelCase on capitals: `read_frame_bgr` = 3, `LoadedFaceModel` = 3, `ensure_capture_device_present` = 4 ❌.
- **Every token counts** — suffixes like `_bgr`, `_px`, prepositions like `_to_`, `_from_`. `crop_frame_to_roi` = 4 ❌.
- An acronym or digit group is one word (`sha256` = 1, `bgra8` = 1).
- Idiomatic constructors (`new`, `from_x`, `with_x`) count normally but rarely trip the limit.
- **Scope:** functions, methods, structs/enums/traits, modules. Exempt: `#[test]` functions (names narrate behavior), constants (`DEFAULT_MASK_DILATE_PX` is fine), local variables, trait method implementations (the trait fixed the name), and derive/trait-mandated names (`fmt`, `from`, `try_from`).

## Over three words? Two remedies

**1. Raise the abstraction — split and compose.** A 4+ word name is usually a step list. Each step gets its own ≤3-word function; the composition point keeps a short name describing the *outcome*, not the steps.

**2. Move a word into a namespace.** If the extra words are a noun phrase repeated across the module, they are the module's (or type's) name, not each function's. Callers read `capture_device::ensure(...)` or `frame.rotate(...)` — the context words are written once.

```rust
// ❌ 4 words — ensure + locate + copy + verify crammed into one name
fn ensure_capture_device_present(name: &str, fallback: &str) -> Device { .. }
fn verify_capture_device_format(dev: &Device, want: PixelFormat) -> bool { .. }

// ✅ remedy 2 — module capture_device carries the noun phrase
mod capture_device {
    fn ensure(name: &str, fallback: &str) -> Device { .. }           // capture_device::ensure(...)
    fn verify_format(dev: &Device, want: PixelFormat) -> bool { .. } // capture_device::verify_format(...)
}

// ✅ remedy 1 — or split by responsibility where a module isn't warranted
fn open_fallback_device(fallback: &str) -> Device { .. }
fn ensure_device(name: &str, fallback: &str) -> Device { .. }  // outcome, not step list
```

The same move works with a struct as the namespace: `CaptureDevice::ensure()`, `FrameBuffer::flush()`, or a method on the value itself: `frame.rotate(angle)`.

Rust convention pushes this further: **don't repeat the module or type name inside the function** — `frame::rotate_frame` ❌ → `frame::rotate` ✅. That repetition is remedy 2 done halfway; clippy's `module_name_repetitions` lint catches the type-name version of the same mistake.

## Name smells that predict a 4th word

| Smell in the name | What it reveals | Fix |
|---|---|---|
| `_and_` (`open_and_configure_device`) | two responsibilities | split: `open_device` + `configure_device`, compose at the caller |
| `_with_` / `_by_` tail (`filter_frames_by_size_and_format`) | parameters leaking into the name | criteria are *arguments*: `filter_frames(min_px, format)` |
| `_if_missing` / `_if_needed` | a guard clause fused into the verb | `ensure_` already implies conditional: `ensure_device` |
| repeated noun phrase across functions (`capture_device_*`) | a missing module/type | namespace it (`capture_device::*`); each function keeps its verb |
| step-list verb chain (`decode_rotate_encode`) | orchestration named by its steps | name the outcome (`transcode_frame`); steps become their own functions |

## Common mistakes

| Mistake | Fix |
|---|---|
| Abbreviating to sneak under the limit (`ens_cap_dev_present`) | Still multiple responsibilities. Split or namespace — never compress spelling. |
| Vague 1-worder to dodge the limit (`process`, `handle`, `do_it`) | Under-specific is as bad as over-long. Three *precise* words beat one vague one. |
| Counting `_bgr`/`_px` as "free" suffixes | They count. `crop_frame_to_roi` → put the ROI in the type, split the step, or namespace it. |
| Renaming without re-scoping (`ensure_capture_device_present` → `ensure_capture`) | Now the name lies — it no longer promises presence. Split first, then name what's left. |
| Applying the limit to tests, constants, or trait impls | Test names narrate behavior; constants encode ranges; trait-mandated names (`fmt`, `from`, `try_from`) aren't yours to shorten. All exempt. |

## Red Flags — STOP

- The name has a 4th `_` segment (or 4th capitalized word in CamelCase) → stop, split or namespace.
- You wrote `_and_`, `_with_`, `_if_` inside a function name → the scope is too broad.
- You're about to abbreviate a word to fit → wrong axis; shrink the responsibility.
- Two or more functions share a 2-word noun prefix → extract the module/type namespace.
- You typed the module or type name again inside the function (`frame::rotate_frame`) → clippy's `module_name_repetitions` is about to fire; drop the repeat.
