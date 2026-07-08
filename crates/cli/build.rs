use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(reason) => {
            eprintln!("crates/cli build.rs failed: {reason}");
            ExitCode::FAILURE
        }
    }
}

// `crates/syphon/build.rs` emits the `-rpath` linker args needed to find the
// vendored Syphon.framework at runtime, but Cargo's `rustc-link-arg`
// instruction only applies to the emitting package's own targets — it does
// not propagate to downstream binaries that merely depend on that crate
// (unlike `rustc-link-lib`/`rustc-link-search`, which do propagate). Since
// this crate's binary links `webcam-sharedtexture-syphon` on macOS, it needs
// the same rpath entries itself or `@rpath/Syphon.framework/...` cannot be
// resolved at process launch.
fn run() -> Result<(), String> {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS")
        .map_err(|err| format!("CARGO_CFG_TARGET_OS is not set: {err}"))?;

    if target_os != "macos" {
        return Ok(());
    }

    // Covers `target/debug/<bin>` and `target/debug/deps/<test-binary>`
    // (and the equivalent one level deeper under an explicit --target triple).
    for rel in [
        "@loader_path/../../vendor",
        "@loader_path/../../../vendor",
        "@loader_path/../../../../vendor",
    ] {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{rel}");
    }

    Ok(())
}
