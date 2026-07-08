use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(reason) => {
            eprintln!("crates/spout build.rs failed: {reason}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS")
        .map_err(|err| format!("CARGO_CFG_TARGET_OS is not set: {err}"))?;

    // Non-Windows builds compile the `#![cfg(target_os = "windows")]`-gated
    // empty crate (see src/lib.rs) — there is no native bridge to build.
    if target_os != "windows" {
        return Ok(());
    }

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map_err(|err| format!("CARGO_MANIFEST_DIR is not set: {err}"))?;
    let crate_dir = Path::new(&manifest_dir);
    let workspace_root = crate_dir
        .parent()
        .and_then(Path::parent)
        .ok_or_else(|| format!("{manifest_dir} has no workspace root two levels up"))?;
    let spout2 = workspace_root.join("vendor").join("Spout2");
    let spout_dx = spout2.join("SpoutDirectX").join("SpoutDX");
    let spout_gl = spout2.join("SpoutGL");

    if !spout_dx.join("SpoutDX.cpp").exists() {
        return Err(format!(
            "Spout2 SDK not found at {}. Run scripts/fetch-spout.sh first.",
            spout2.display()
        ));
    }

    println!("cargo:rerun-if-changed=cpp/spout_bridge.cpp");
    println!("cargo:rerun-if-changed=cpp/spout_bridge.h");
    println!("cargo:rerun-if-changed={}", spout2.display());

    cc::Build::new()
        .cpp(true)
        .file("cpp/spout_bridge.cpp")
        .file(spout_dx.join("SpoutDX.cpp"))
        .file(spout_gl.join("SpoutDirectX.cpp"))
        .file(spout_gl.join("SpoutSenderNames.cpp"))
        .file(spout_gl.join("SpoutFrameCount.cpp"))
        .file(spout_gl.join("SpoutUtils.cpp"))
        .file(spout_gl.join("SpoutCopy.cpp"))
        .file(spout_gl.join("SpoutSharedMemory.cpp"))
        .include(&spout_dx)
        .include(&spout_gl)
        .include("cpp")
        .flag("/EHsc")
        .flag("/std:c++17")
        .try_compile("spout_bridge")
        .map_err(|err| format!("failed to compile cpp/spout_bridge.cpp: {err}"))?;

    for lib in
        ["d3d11", "dxgi", "user32", "gdi32", "shell32", "ole32", "comdlg32", "comctl32", "shlwapi"]
    {
        println!("cargo:rustc-link-lib={lib}");
    }

    Ok(())
}
