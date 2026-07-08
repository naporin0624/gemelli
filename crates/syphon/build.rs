use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(reason) => {
            eprintln!("crates/syphon build.rs failed: {reason}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS")
        .map_err(|err| format!("CARGO_CFG_TARGET_OS is not set: {err}"))?;

    // Non-macOS builds compile the `#![cfg(target_os = "macos")]`-gated empty
    // crate (see src/lib.rs) — there is no native bridge to build.
    if target_os != "macos" {
        return Ok(());
    }

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map_err(|err| format!("CARGO_MANIFEST_DIR is not set: {err}"))?;
    let crate_dir = Path::new(&manifest_dir);
    let workspace_root = crate_dir
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| format!("{manifest_dir} has no workspace root two levels up"))?;
    let vendor_dir = workspace_root.join("vendor");
    let vendor_str = vendor_dir
        .to_str()
        .ok_or_else(|| format!("vendor path {} is not valid UTF-8", vendor_dir.display()))?;

    println!("cargo:rerun-if-changed=cpp/syphon_bridge.mm");
    println!("cargo:rerun-if-changed=cpp/syphon_bridge.h");
    println!("cargo:rerun-if-changed={vendor_str}/Syphon.framework");

    cc::Build::new()
        .file("cpp/syphon_bridge.mm")
        .include("cpp")
        .flag("-ObjC++")
        .flag("-std=c++17")
        .flag("-fobjc-arc")
        .flag("-F")
        .flag(vendor_str)
        .try_compile("syphon_bridge")
        .map_err(|err| format!("failed to compile cpp/syphon_bridge.mm: {err}"))?;

    // C++ runtime, required because syphon_bridge.mm is compiled as C++17.
    println!("cargo:rustc-link-lib=c++");

    println!("cargo:rustc-link-lib=framework=Syphon");
    println!("cargo:rustc-link-lib=framework=Metal");
    println!("cargo:rustc-link-lib=framework=IOSurface");
    println!("cargo:rustc-link-lib=framework=Cocoa");
    println!("cargo:rustc-link-lib=framework=QuartzCore");
    println!("cargo:rustc-link-search=framework={vendor_str}");

    // rpath so built binaries find vendor/Syphon.framework without a
    // system-wide install. `cargo build`/`test` binaries land at
    // target/debug/, target/debug/deps/, or — with an explicit --target
    // triple — one directory deeper; cover each depth back to the workspace
    // root so every binary kind resolves the same relative vendor/ path.
    for rel in [
        "@loader_path/../../vendor",
        "@loader_path/../../../vendor",
        "@loader_path/../../../../vendor",
    ] {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{rel}");
    }

    Ok(())
}
