//! Sliding 1-second window frame-rate counter, driven by injected
//! `Instant`s so it's unit-testable without a real clock/thread.

use std::collections::VecDeque;
use std::time::{Duration, Instant};

const WINDOW: Duration = Duration::from_secs(1);

pub struct FpsMeter {
    samples: VecDeque<Instant>,
}

impl FpsMeter {
    pub fn new() -> Self {
        Self { samples: VecDeque::new() }
    }

    /// Records one frame-published event at `now`.
    pub fn record(&mut self, now: Instant) {
        self.samples.push_back(now);
    }

    /// Evicts samples older than the 1-second window as of `now`, then
    /// returns the remaining sample count as an approximate frames/sec.
    pub fn rate(&mut self, now: Instant) -> f32 {
        self.evict_stale(now);
        count_to_f32(self.samples.len())
    }

    fn evict_stale(&mut self, now: Instant) {
        while let Some(&oldest) = self.samples.front() {
            if now.saturating_duration_since(oldest) <= WINDOW {
                break;
            }
            self.samples.pop_front();
        }
    }
}

impl Default for FpsMeter {
    fn default() -> Self {
        Self::new()
    }
}

/// gui's second isolated `as`-cast (see `preview::dim_to_f32` for the
/// std-conversion-gap rationale — the same `f32: From<u32>` gap applies to
/// `usize -> f32` here). Sample counts are bounded by the 1-second sliding
/// window (never remotely near 2^24), so this is lossless in practice.
#[allow(clippy::as_conversions)]
fn count_to_f32(count: usize) -> f32 {
    let count = u32::try_from(count).unwrap_or(u32::MAX);
    count as f32
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn rate_counts_samples_within_the_one_second_window() {
        let mut meter = FpsMeter::new();
        let base = std::time::Instant::now();
        for i in 0..10u64 {
            meter.record(base + Duration::from_millis(i * 100));
        }
        // last sample at 900ms; queried at 900ms, every sample is <= 900ms
        // old, so all 10 remain in the 1s window.
        assert_eq!(meter.rate(base + Duration::from_millis(900)), 10.0);
    }

    #[test]
    fn rate_evicts_samples_older_than_one_second() {
        let mut meter = FpsMeter::new();
        let base = std::time::Instant::now();
        meter.record(base); // ages out by t=1500ms (age 1500ms > 1000ms)
        meter.record(base + Duration::from_millis(900)); // age 600ms, kept
        assert_eq!(meter.rate(base + Duration::from_millis(1500)), 1.0);
    }

    #[test]
    fn rate_on_empty_meter_is_zero() {
        let mut meter = FpsMeter::new();
        assert_eq!(meter.rate(std::time::Instant::now()), 0.0);
    }
}
