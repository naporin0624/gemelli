//! CPU micro-benchmark for the Spout output path (Windows only). A/B/C-
//! compares `SendMode::StagingRowCopy`, `SendMode::SendImage`, and
//! `SendMode::StagingSse` at 1080p, 4K, and an unaligned-pitch crop,
//! reporting per-frame wall/CPU time, end-to-end publish throughput (wall
//! time, including the SpoutDX send call — not pure memcpy), single-core
//! CPU% at 60 fps, and CPU-time speedup vs StagingRowCopy (the baseline,
//! matching the original send strategy this crate started from).
//!
//! Set `BENCH_FORMAT=markdown` to print a GitHub-flavored-markdown heading
//! and table per case instead of the default aligned-text tables (e.g. for
//! posting results as a PR comment from CI). The default output is
//! unaffected by this switch.

#[cfg(not(target_os = "windows"))]
fn main() {
    eprintln!("bench_spout_cpu is Windows-only (Spout backend).");
}

#[cfg(target_os = "windows")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use gemelli_spout::SendMode;

    let modes = [
        ("StagingRowCopy", SendMode::StagingRowCopy),
        ("SendImage", SendMode::SendImage),
        ("StagingSse", SendMode::StagingSse),
    ];
    let format = BenchFormat::from_env();

    // (label, width, height, frames, warmup)
    let cases: [(&str, u32, u32, u32, u32); 3] = [
        ("1080p", 1920, 1080, 600, 60),
        ("4K", 3840, 2160, 300, 30),
        ("cropped-unaligned", 2458, 1080, 600, 60),
    ];

    for (label, width, height, frames, warmup) in cases {
        bench_resolution(label, width, height, frames, warmup, &modes, format)?;
    }
    Ok(())
}

/// Output style for `bench_resolution`, selected once at start-up via the
/// `BENCH_FORMAT` env var. Kept as its own type (rather than a bare bool) so
/// call sites read as intent, not a flag, and so a third format can be added
/// without renaming a `markdown: bool` parameter everywhere.
#[cfg(target_os = "windows")]
#[derive(Clone, Copy, PartialEq, Eq)]
enum BenchFormat {
    /// The original aligned-text tables (default; byte-identical to before
    /// this switch existed).
    Text,
    /// GitHub-flavored markdown: one `###` heading and one table per case.
    Markdown,
}

#[cfg(target_os = "windows")]
impl BenchFormat {
    /// Reads `BENCH_FORMAT` from the environment; any value other than
    /// exactly `"markdown"` (including unset) selects `Text`.
    fn from_env() -> Self {
        match std::env::var("BENCH_FORMAT") {
            Ok(value) if value == "markdown" => BenchFormat::Markdown,
            _ => BenchFormat::Text,
        }
    }
}

#[cfg(target_os = "windows")]
fn bench_resolution(
    label: &str,
    width: u32,
    height: u32,
    frames: u32,
    warmup: u32,
    modes: &[(&str, gemelli_spout::SendMode)],
    format: BenchFormat,
) -> Result<(), Box<dyn std::error::Error>> {
    use gemelli_core::frame::Frame;
    use gemelli_spout::{SpoutPublisher, metrics};
    use std::time::Duration;

    let bytes_per_frame = metrics::frame_bytes(width, height);
    let target_fps = 60.0;

    // Non-uniform content so the memcpy isn't optimised against a zero page.
    let pixel_count = usize::try_from(bytes_per_frame).unwrap_or(0);
    let mut buf = vec![0_u8; pixel_count];
    for (i, byte) in buf.iter_mut().enumerate() {
        *byte = u8::try_from(i & 0xff).unwrap_or(0);
    }
    let frame = Frame::new(width, height, buf)?;

    match format {
        BenchFormat::Text => {
            println!();
            println!("== Spout output CPU benchmark: {label} ==");
            println!(
                "resolution : {width}x{height}  ({:.2} MB/frame, BGRA)",
                f64::from(bytes_per_frame) / 1e6
            );
            println!("frames     : {frames} (warmup {warmup})");
            println!(
                "{:<20} {:>12} {:>12} {:>12} {:>12} {:>12}",
                "mode", "wall us/f", "cpu us/f", "MB/s", "CPU%@60", "speedup"
            );
        }
        BenchFormat::Markdown => {
            println!();
            println!("### {label} ({width}x{height}, {frames} frames)");
            println!();
            println!("| mode | wall µs/frame | cpu µs/frame | MB/s | CPU% @60fps | speedup |");
            println!("| --- | --- | --- | --- | --- | --- |");
        }
    }

    // CPU time per frame for each mode; index 0 is the StagingRowCopy baseline.
    let mut baseline_cpu = Duration::ZERO;

    for (i, (name, mode)) in modes.iter().enumerate() {
        // Fresh publisher per mode so staging-texture cache state doesn't
        // bleed between strategies.
        let mut publisher = SpoutPublisher::new("gemelli-bench")?;

        for _ in 0..warmup {
            publisher.publish_mode(&frame, *mode)?;
        }

        let cpu_start = process_cpu_time();
        let wall_start = std::time::Instant::now();
        for _ in 0..frames {
            publisher.publish_mode(&frame, *mode)?;
        }
        let wall = wall_start.elapsed();
        let cpu = process_cpu_time().saturating_sub(cpu_start);
        drop(publisher);

        let wall_us = metrics::per_frame_micros(wall, frames);
        let cpu_us = metrics::per_frame_micros(cpu, frames);
        let mb_s = metrics::throughput_mb_s(bytes_per_frame, frames, wall);
        let cpu_per_frame = cpu.checked_div(frames).unwrap_or(Duration::ZERO);
        let cpu_pct = metrics::cpu_load_percent(cpu_per_frame, target_fps);

        if i == 0 {
            baseline_cpu = cpu;
        }
        let speedup = metrics::speedup_ratio(baseline_cpu, cpu);

        match format {
            BenchFormat::Text => {
                println!(
                    "{name:<20} {wall_us:>12.2} {cpu_us:>12.2} {mb_s:>12.0} {cpu_pct:>12.2} {speedup:>11.2}x"
                );
            }
            BenchFormat::Markdown => {
                println!(
                    "{}",
                    metrics::markdown_row(name, wall_us, cpu_us, mb_s, cpu_pct, speedup)
                );
            }
        }
    }

    Ok(())
}

/// User+kernel CPU time consumed by this process so far.
///
/// Unlike `crates/syphon/examples/bench_syphon_cpu.rs`, which reads
/// thread-specific CPU time via POSIX `CLOCK_THREAD_CPUTIME_ID`, Windows has
/// no thread-scoped clock exposed through the `libc` crate (only
/// `GetThreadTimes`, a raw Win32 API this bridge does not otherwise bind).
/// `libc::clock()` measures whole-process CPU time instead — a reasonable
/// proxy here since this benchmark binary is single-threaded end to end.
#[cfg(target_os = "windows")]
fn process_cpu_time() -> std::time::Duration {
    // The Windows CRT's `clock()` ticks at 1000 Hz (CLOCKS_PER_SEC == 1000),
    // unlike POSIX's 1_000_000 Hz. The `libc` crate does not bind
    // `CLOCKS_PER_SEC` for any target, so the CRT-documented value is used
    // directly.
    const WINDOWS_CLOCKS_PER_SEC: i64 = 1000;

    // SAFETY: `clock()` takes no arguments and only reads process-global CRT
    // state; it has no preconditions beyond being linked against the CRT,
    // which every Windows Rust binary is.
    let ticks = i64::from(unsafe { libc::clock() });
    let millis = ticks.saturating_mul(1000) / WINDOWS_CLOCKS_PER_SEC;
    std::time::Duration::from_millis(u64::try_from(millis).unwrap_or(0))
}
