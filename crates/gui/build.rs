use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(reason) => {
            eprintln!("crates/gui build.rs failed: {reason}");
            ExitCode::FAILURE
        }
    }
}

// `crates/syphon/build.rs` emits the `-rpath` linker args needed to find the
// vendored Syphon.framework at runtime, but Cargo's `rustc-link-arg`
// instruction only applies to the emitting package's own targets — it does
// not propagate to downstream binaries that merely depend on that crate
// (unlike `rustc-link-lib`/`rustc-link-search`, which do propagate). Since
// this crate's binary links `gemelli-syphon` on macOS, it needs
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
    embed_windows_manifest()?;

    if let Ok(rpaths) = std::env::var("DEP_SYPHON_BRIDGE_RPATH") {
        for rel in rpaths.split(';').filter(|rel| !rel.is_empty()) {
            println!("cargo:rustc-link-arg=-Wl,-rpath,{rel}");
        }
    }

    Ok(())
}

// On Windows/MSVC, embed an application manifest that activates ComCtl32 v6.
// The vendored Spout2 SDK (SpoutUtils) imports COMCTL32 ordinal 345, which
// exists only in the v6 common-controls assembly; without this the loader
// binds the v5 comctl32.dll and gemelli-gui.exe fails to start ("ordinal 345
// not found"). No-op on every other target.
fn embed_windows_manifest() -> Result<(), String> {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if target_os != "windows" || target_env != "msvc" {
        return Ok(());
    }

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map_err(|err| format!("CARGO_MANIFEST_DIR is not set: {err}"))?;
    let manifest = std::path::Path::new(&manifest_dir).join("app.manifest");
    let manifest_str = manifest
        .to_str()
        .ok_or_else(|| format!("manifest path {} is not valid UTF-8", manifest.display()))?;

    println!("cargo:rerun-if-changed=app.manifest");
    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTINPUT:{manifest_str}");

    Ok(())
}
