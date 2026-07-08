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
//
// Rather than duplicating the rpath list here, read it back from syphon's
// `links = "syphon_bridge"` build-script metadata (published as
// `cargo::metadata=rpath=...` in crates/syphon/build.rs) via the
// `DEP_SYPHON_BRIDGE_RPATH` env var Cargo derives from it. This var is only
// set when the syphon crate is an active dependency (macOS targets), so its
// absence on other platforms is expected and not an error.
fn run() -> Result<(), String> {
    let Ok(rpaths) = std::env::var("DEP_SYPHON_BRIDGE_RPATH") else {
        return Ok(());
    };

    for rel in rpaths.split(';').filter(|rel| !rel.is_empty()) {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{rel}");
    }

    Ok(())
}
