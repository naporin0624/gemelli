---
name: prefix-match-processor
description: Use when branching on the prefix of a string with starts_with/strip_prefix (namespaced families like device URI schemes "avf://", "syphon://" or CLI transform tokens "rotate=", "flip=") and the branch count keeps growing, when a starts_with if-chain is hard to test or extend per family, or when adding several new prefix families to an existing dispatcher.
---

# Prefix-Match Processor

## Overview

A growing if/`starts_with` (or `strip_prefix`) chain where each branch carries real logic is a **processor registry** waiting to happen (a.k.a. the plugin / strategy registry, chain-of-responsibility). Replace the chain with a trait, a `Vec<Box<dyn Processor>>` of self-contained implementers, and a runner that returns the first match. Adding a family becomes "write a struct, register it"; the dispatcher never grows.

The input is typically a tagged value — model it with `precise-type-modeling`. Composing the `Result` inside `run` (`.map`, `.map_err`) follows `chaining-result-combinators`.

## When to extract — the threshold

| Signal | Keep inline `if`/`match` | Enum + exhaustive `match` | Processor registry (`Vec<Box<dyn Processor>>`) |
|---|---|---|---|
| Family count | ≤ ~5, one-line mapping each | Known, fixed set — no new family expected | > ~5 families, **or** growing over time |
| Branch complexity | trivial | multi-line logic OK, all variants live in one file | multi-line logic owned/tested per family |
| Extensibility | n/a | adding a variant means editing the `match` — the compiler forces exhaustiveness, which is a feature when the set really is closed | adding a family = new struct + one registration line, no central edit |

The enum route is `branching-modeled-state-with-match`'s territory: when every scheme is fixed at compile time, an exhaustive `match` is simpler and compiler-checked. The registry earns its place once families are added independently over time or need isolated tests — do not extract 2-3 one-liners either way; that's premature.

## Before (a prefix if-chain that outgrew itself) ❌

```rust
fn resolve_device(uri: &str) -> Result<Device, String> {
    if uri.starts_with("avf://") {
        return parse_avf(uri);
    }
    if uri.starts_with("v4l2://") {
        return parse_v4l2(uri);
    }
    if uri.starts_with("syphon://") {
        return parse_syphon(uri);
    }
    // …a dozen more, each growing its own validation, untestable per-family…
    Err(format!("unknown device uri: {uri}"))
}
```

## After (the canonical shape) ✅

**The processor contract** — `None` means "not mine, try the next processor"; `Some(Err(_))` means "mine, but parsing failed" — the two must never be conflated:

```rust
pub trait Processor {
    /// `None`      -> this processor does not claim `uri`.
    /// `Some(Ok)`  -> claimed and parsed successfully.
    /// `Some(Err)` -> claimed, but the input after the prefix is invalid.
    fn run(&self, uri: &str) -> Option<Result<Device, DeviceError>>;
}
```

**One self-contained processor per family** — the prefix check lives inside `run`, via `strip_prefix`:

```rust
struct AvfProcessor;

impl Processor for AvfProcessor {
    fn run(&self, uri: &str) -> Option<Result<Device, DeviceError>> {
        let index = uri.strip_prefix("avf://")?; // None: not our scheme
        let parsed = index
            .parse::<u32>()
            .map_err(|_| DeviceError::InvalidIndex(index.to_string()));
        Some(parsed.map(Device::AvFoundation))
    }
}
```

**The registry + runner — first `Some` wins** (the only place that lists families; ordered most-specific-prefix first):

```rust
pub struct Registry {
    processors: Vec<Box<dyn Processor>>,
}

impl Registry {
    pub fn dispatch(&self, uri: &str) -> Result<Device, DeviceError> {
        for processor in &self.processors {
            if let Some(result) = processor.run(uri) {
                return result;
            }
        }
        Err(DeviceError::UnknownScheme(uri.to_string()))
    }
}

let registry = Registry {
    processors: vec![
        Box::new(AvfProcessor),
        Box::new(V4l2Processor),
        Box::new(SyphonProcessor),
        Box::new(SpoutProcessor),
    ],
};
```

Adding a family: write `MediaFoundationProcessor`, push it into the `vec!`. Test it alone: `AvfProcessor.run("avf://0")` — no registry, no other families.

## Order matters

First `Some` wins, so register the **more specific prefix before the broader one**: `syphon://output/` before a bare `syphon://`. A broad prefix registered first shadows every specific one after it.

## Common mistakes

| Mistake | Fix |
|---|---|
| Processor panics, or returns `None` for both "not mine" and "mine but invalid" | Non-match MUST be `None`; claimed-but-invalid MUST be `Some(Err(_))`. Conflating them lets the runner silently fall through to the wrong processor. |
| Re-implementing the dispatch loop ad hoc at each call site | One shared `Registry::dispatch`; only processors are per-family. |
| Suffix/detail parsing placed in the dispatcher | Keep `strip_prefix` + suffix branching **inside** `run`; the registry loop stays dumb. |
| A `HashMap<&str, fn(&str) -> _>` keyed by prefix instead of the ordered `Vec` | A map keys on exact equality/hash and can't express prefix matching + specificity ordering. Use an ordered `Vec`. |
| Broad prefix registered before a specific one | Order most-specific-first; first match wins. |
| Extracting 2-3 one-liners into processors | Premature — keep the inline branch, or reach for an enum `match`, until the registry threshold is met. |

## Red Flags — STOP

- A `starts_with`/`strip_prefix` if-chain that keeps gaining branches, or branches gaining their own multi-line logic → extract to a processor registry (or an enum `match` if the family set is closed).
- About to invent a new dispatch shape (`HashMap<prefix, fn>`, `Vec<(&str, fn)>`, a `match` over `starts_with` guards) → use the `Processor` trait + `Vec<Box<dyn Processor>>` + first-`Some` runner instead.
- A processor returning `None` to mean "claimed but failed" → it must be `Some(Err(_))`; `None` is reserved for "not mine".
- Per-family logic that can only be tested by driving the whole registry → each processor must be unit-testable on its own.
