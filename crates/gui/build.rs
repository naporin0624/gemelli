use std::process::ExitCode;

use vergen_gix::{Build, Emitter, Gix};

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(reason) => {
            eprintln!("crates/gui build.rs failed: {reason}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    emit_build_id()?;
    embed_windows_manifest()?;
    emit_syphon_rpath()
}

/// Embeds a short git SHA (`VERGEN_GIT_SHA`) and the build date
/// (`VERGEN_BUILD_DATE`) as compile-time env vars. `VERGEN_GIT_SHA` is read
/// back via `option_env!` by `menu::about_metadata` for the native About
/// panel; `VERGEN_BUILD_DATE` is embedded for build provenance and has no
/// in-app reader.
///
/// Deliberately uses vergen-gix's *default* `Emitter` — no `.fail_on_error()`,
/// `.idempotent()`, or `.default_on_error()`. Verified empirically: when git info
/// is unavailable (e.g. building from a source tarball with no `.git` directory),
/// `add_instructions`/`emit` still return `Ok` — they leave `VERGEN_GIT_SHA` unset
/// and print a `cargo:warning`, they do not fail the build.
/// `option_env!("VERGEN_GIT_SHA").unwrap_or("unknown")` on the consumer side is
/// what turns "unset" into a displayable fallback; `VERGEN_BUILD_DATE` has no such
/// gap since it comes from the local clock, not git, so it is always emitted.
fn emit_build_id() -> Result<(), String> {
    let gix = Gix::builder().sha(true).build();
    let build = Build::builder().build_date(true).build();

    Emitter::default()
        .add_instructions(&gix)
        .map_err(|reason| reason.to_string())?
        .add_instructions(&build)
        .map_err(|reason| reason.to_string())?
        .emit()
        .map_err(|reason| reason.to_string())?;

    Ok(())
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
fn emit_syphon_rpath() -> Result<(), String> {
    let Ok(rpaths) = std::env::var("DEP_SYPHON_BRIDGE_RPATH") else {
        return Ok(());
    };

    for rel in rpaths.split(';').filter(|rel| !rel.is_empty()) {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{rel}");
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
