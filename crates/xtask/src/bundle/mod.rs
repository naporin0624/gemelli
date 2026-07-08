use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

pub mod cmd;
pub mod layout;
pub mod plist;

use layout::AppBundlePaths;
use plist::PlistFields;

const APP_BUNDLE_NAME: &str = "gemelli.app";
const EXECUTABLE_NAME: &str = "gemelli-gui";
const AARCH64_TARGET: &str = "aarch64-apple-darwin";
const X86_64_TARGET: &str = "x86_64-apple-darwin";
const BUNDLE_RPATH: &str = "@executable_path/../Frameworks";

fn io_error(path: &Path, source: std::io::Error) -> crate::XtaskError {
    crate::XtaskError::Io { path: path.display().to_string(), source }
}

/// Runs `command` with `args` in `cwd`, mapping spawn failure and nonzero exit into `XtaskError`.
fn run_checked(command: &str, args: &[OsString], cwd: &Path) -> Result<(), crate::XtaskError> {
    let output = Command::new(command)
        .args(args)
        .current_dir(cwd)
        .output()
        .map_err(|source| crate::XtaskError::Spawn { command: command.to_string(), source })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        return Err(crate::XtaskError::Subprocess { command: command.to_string(), stderr });
    }

    Ok(())
}

/// Runs `cargo metadata --format-version 1 --no-deps` in `root` and returns its stdout, the
/// authoritative source for each workspace package's version (not xtask's own `CARGO_PKG_VERSION`).
fn cargo_metadata_json(root: &Path) -> Result<String, crate::XtaskError> {
    let command = "cargo metadata";
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(root)
        .output()
        .map_err(|source| crate::XtaskError::Spawn { command: command.to_string(), source })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
        return Err(crate::XtaskError::Subprocess { command: command.to_string(), stderr });
    }

    String::from_utf8(output.stdout)
        .map_err(|source| crate::XtaskError::Utf8 { command: command.to_string(), source })
}

fn release_binary_path(root: &Path, target: &str) -> PathBuf {
    root.join("target").join(target).join("release").join(EXECUTABLE_NAME)
}

/// Builds the GUI release binary for one architecture `target`, mirroring
/// `cargo build --release -p gemelli-gui --target <target>` run from the workspace root.
fn build_gui_release(root: &Path, target: &str) -> Result<(), crate::XtaskError> {
    let args = cmd::cargo_build_target_args("gemelli-gui", target);
    run_checked("cargo", &args, root)
}

/// Recursively copies `source` into the directory `destination_dir`, preserving the symlinks
/// (`Versions/Current`, top-level convenience links) that macOS frameworks depend on — `cp -R`
/// is the simplest tool that gets this right, unlike a naive file-by-file Rust copy.
fn copy_framework_into(
    source: &Path,
    destination_dir: &Path,
    cwd: &Path,
) -> Result<(), crate::XtaskError> {
    let args = [
        OsString::from("-R"),
        source.as_os_str().to_os_string(),
        destination_dir.as_os_str().to_os_string(),
    ];
    run_checked("cp", &args, cwd)
}

/// Assembles a distributable `.app` bundle at `<root>/target/dist/gemelli.app`, rebuilding it
/// from scratch on every call. Returns the bundle's root path.
pub fn bundle(root: &Path) -> Result<PathBuf, crate::XtaskError> {
    let dist_dir = root.join("target/dist");
    let paths = AppBundlePaths::new(&dist_dir, APP_BUNDLE_NAME, EXECUTABLE_NAME);

    if paths.root.exists() {
        fs::remove_dir_all(&paths.root).map_err(|source| io_error(&paths.root, source))?;
    }
    fs::create_dir_all(&paths.contents).map_err(|source| io_error(&paths.contents, source))?;
    fs::create_dir_all(&paths.macos).map_err(|source| io_error(&paths.macos, source))?;
    fs::create_dir_all(&paths.frameworks).map_err(|source| io_error(&paths.frameworks, source))?;
    fs::create_dir_all(&paths.resources).map_err(|source| io_error(&paths.resources, source))?;

    build_gui_release(root, AARCH64_TARGET)?;
    build_gui_release(root, X86_64_TARGET)?;

    let aarch64_binary = release_binary_path(root, AARCH64_TARGET);
    let x86_64_binary = release_binary_path(root, X86_64_TARGET);
    let lipo_args = cmd::lipo_create_args(&[aarch64_binary, x86_64_binary], &paths.executable);
    run_checked("lipo", &lipo_args, root)?;

    let syphon_framework = root.join("vendor/Syphon.framework");
    copy_framework_into(&syphon_framework, &paths.frameworks, root)?;

    let rpath_args = cmd::add_rpath_args(BUNDLE_RPATH, &paths.executable);
    run_checked("install_name_tool", &rpath_args, root)?;

    let metadata_json = cargo_metadata_json(root)?;
    let version = layout::gui_package_version(&metadata_json)?;
    let fields = PlistFields {
        bundle_name: "gemelli",
        display_name: "gemelli",
        identifier: "com.naporin0624.gemelli",
        executable: EXECUTABLE_NAME,
        icon_file: "icon",
        short_version: &version,
        version: &version,
        min_system_version: "11.0",
        camera_usage_description: "gemelli shares your camera feed as a Syphon texture.",
    };
    let info_plist_xml = plist::info_plist_xml(&fields);
    fs::write(&paths.info_plist, info_plist_xml)
        .map_err(|source| io_error(&paths.info_plist, source))?;

    let icon_source = root.join("crates/gui/assets/icon.icns");
    let icon_dest = paths.resources.join("icon.icns");
    fs::copy(&icon_source, &icon_dest).map_err(|source| io_error(&icon_dest, source))?;

    let notices_source = root.join("THIRD-PARTY-NOTICES");
    let notices_dest = paths.resources.join("THIRD-PARTY-NOTICES");
    fs::copy(&notices_source, &notices_dest).map_err(|source| io_error(&notices_dest, source))?;

    Ok(paths.root)
}
