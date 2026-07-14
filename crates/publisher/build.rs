use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(reason) => {
            eprintln!("crates/publisher build.rs failed: {reason}");
            ExitCode::FAILURE
        }
    }
}

// linguine's own build.rs sets the `-rpath` linker arg needed to find the
// vendored Syphon.framework at runtime, but Cargo's `rustc-link-arg`
// instruction only applies to the emitting package's own targets — it does
// not propagate to a downstream crate that merely depends on linguine
// (unlike `rustc-link-lib`/`rustc-link-search`, which do propagate). Since
// this crate links `linguine` on macOS, it needs the same rpath entry
// itself — and because gemelli-publisher isn't the final binary either, it
// has to re-publish the value one hop further so crates/cli's and
// crates/gui's build scripts can pick it up in turn.
//
// linguine publishes the resolved framework directory via its own
// `links = "linguine"` build-script metadata channel, which Cargo exposes
// to *directly*-dependent crates' build scripts (this crate, since it
// depends on linguine directly) as `DEP_LINGUINE_RPATH`. Only linguine's
// direct dependent (this crate) can read that var at all — Cargo does not
// forward `DEP_*` vars transitively — so re-emit it under this crate's own
// `links = "gemelli_publisher"` channel, which cli/gui (depending on
// gemelli-publisher, not linguine, directly) can then read back as
// `DEP_GEMELLI_PUBLISHER_RPATH`.
//
// This crate also has to set the same rpath on *its own* unit test binary
// (this package has no `[[bin]]`/`tests/` integration-test target — its
// only self-produced executable is the `--lib --test` unit test binary
// built from `#[cfg(test)]` in src/lib.rs) for the same "link-arg doesn't
// propagate" reason: `cargo test -p gemelli-publisher` links linguine's
// compiled Syphon FFI calls into that binary, so without an rpath of its
// own it fails at process launch with a dyld "Library not loaded" error,
// same as any other consumer would. The bare `cargo:rustc-link-arg` (not
// `-bins`/`-tests`, which target `[[bin]]`/`tests/*.rs` files this package
// has none of) is the selector that reaches a lib crate's own unit test
// binary.
fn run() -> Result<(), String> {
    let Ok(rpath) = std::env::var("DEP_LINGUINE_RPATH") else {
        // Not set on non-macOS targets (linguine's macOS-only build.rs
        // branch never runs there) — nothing to do.
        return Ok(());
    };

    println!("cargo:rustc-link-arg=-Wl,-rpath,{rpath}");
    println!("cargo::metadata=rpath={rpath}");

    Ok(())
}
