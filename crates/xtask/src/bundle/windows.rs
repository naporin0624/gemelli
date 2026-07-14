use std::{
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
};

/// Directory name for the Windows distribution staging dir, e.g. `gemelli-0.4.0-windows-x64`.
/// The `.zip` and the Inno Setup `OutputBaseFilename` both derive from this stem.
pub fn stage_dir_name(version: &str) -> String {
    format!("gemelli-{version}-windows-x64")
}

/// File name of the plain-archive artifact, e.g. `gemelli-0.4.0-windows-x64.zip`.
pub fn zip_name(version: &str) -> String {
    format!("{}.zip", stage_dir_name(version))
}

/// (source, destination) pairs copied into the Windows staging directory: both binaries, the
/// icon Inno Setup references at run time, and the docs bundled alongside them.
pub fn staging_pairs(root: &Path, staging_dir: &Path) -> Vec<(PathBuf, PathBuf)> {
    [
        ("target/release/gemelli.exe", "gemelli.exe"),
        ("target/release/gemelli-gui.exe", "gemelli-gui.exe"),
        ("crates/gui/assets/icon.ico", "icon.ico"),
        ("README.md", "README.md"),
        ("THIRD-PARTY-NOTICES", "THIRD-PARTY-NOTICES"),
    ]
    .into_iter()
    .map(|(source, destination)| (root.join(source), staging_dir.join(destination)))
    .collect()
}

/// Resolves `ISCC.exe`: the `ISCC_PATH` environment override if set, else the path the
/// stock Inno Setup 6 installer (and its chocolatey package) uses.
pub fn iscc_path(env_override: Option<OsString>) -> PathBuf {
    env_override
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files (x86)\Inno Setup 6\ISCC.exe"))
}

/// Assembles `target/dist/gemelli-<version>-windows-x64.zip` and
/// `target/dist/gemelli-<version>-windows-x64-setup.exe` (via Inno Setup's ISCC, resolved
/// through [`iscc_path`]). Builds both release binaries first, mirroring the macOS `dist`.
pub fn dist(root: &Path) -> Result<(), crate::XtaskError> {
    let build_args = super::cmd::cargo_build_release_args(&["gemelli-cli", "gemelli-gui"]);
    super::run_checked("cargo", &build_args, root)?;

    let metadata_json = super::cargo_metadata_json(root)?;
    let version = super::layout::gui_package_version(&metadata_json)?;

    let dist_dir = root.join("target/dist");
    let staging_dir = dist_dir.join(stage_dir_name(&version));
    if staging_dir.exists() {
        fs::remove_dir_all(&staging_dir).map_err(|source| super::io_error(&staging_dir, source))?;
    }
    fs::create_dir_all(&staging_dir).map_err(|source| super::io_error(&staging_dir, source))?;
    for (source, destination) in staging_pairs(root, &staging_dir) {
        fs::copy(&source, &destination).map_err(|error| super::io_error(&destination, error))?;
    }

    let zip_path = dist_dir.join(zip_name(&version));
    let tar_args = super::cmd::tar_zip_args(&zip_path, &dist_dir, &stage_dir_name(&version));
    super::run_checked("tar", &tar_args, root)?;

    let iss_path = root.join("packaging/windows/gemelli.iss");
    let iscc = iscc_path(std::env::var_os("ISCC_PATH"));
    let iscc_invocation_args = super::cmd::iscc_args(&version, &staging_dir, &dist_dir, &iss_path);
    super::run_checked(&iscc, &iscc_invocation_args, root)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_dir_name_embeds_version_between_fixed_segments() {
        assert_eq!(stage_dir_name("0.4.0"), "gemelli-0.4.0-windows-x64");
    }

    #[test]
    fn zip_name_appends_zip_to_the_stage_dir_name() {
        assert_eq!(zip_name("0.4.0"), "gemelli-0.4.0-windows-x64.zip");
    }

    #[test]
    fn staging_pairs_maps_every_distribution_file_into_the_staging_dir() {
        let root = PathBuf::from("/repo");
        let staging_dir = PathBuf::from("/repo/target/dist/gemelli-0.4.0-windows-x64");

        let pairs = staging_pairs(&root, &staging_dir);

        assert_eq!(
            pairs,
            vec![
                (
                    PathBuf::from("/repo/target/release/gemelli.exe"),
                    PathBuf::from("/repo/target/dist/gemelli-0.4.0-windows-x64/gemelli.exe"),
                ),
                (
                    PathBuf::from("/repo/target/release/gemelli-gui.exe"),
                    PathBuf::from("/repo/target/dist/gemelli-0.4.0-windows-x64/gemelli-gui.exe"),
                ),
                (
                    PathBuf::from("/repo/crates/gui/assets/icon.ico"),
                    PathBuf::from("/repo/target/dist/gemelli-0.4.0-windows-x64/icon.ico"),
                ),
                (
                    PathBuf::from("/repo/README.md"),
                    PathBuf::from("/repo/target/dist/gemelli-0.4.0-windows-x64/README.md"),
                ),
                (
                    PathBuf::from("/repo/THIRD-PARTY-NOTICES"),
                    PathBuf::from(
                        "/repo/target/dist/gemelli-0.4.0-windows-x64/THIRD-PARTY-NOTICES"
                    ),
                ),
            ]
        );
    }

    #[test]
    fn iscc_path_prefers_the_env_override() {
        let path = iscc_path(Some(OsString::from(r"D:\tools\ISCC.exe")));

        assert_eq!(path, PathBuf::from(r"D:\tools\ISCC.exe"));
    }

    #[test]
    fn iscc_path_defaults_to_the_standard_inno_setup_6_install() {
        let path = iscc_path(None);

        assert_eq!(path, PathBuf::from(r"C:\Program Files (x86)\Inno Setup 6\ISCC.exe"));
    }
}
