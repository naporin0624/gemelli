use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

/// Directory name for the Windows distribution staging dir, e.g. `gemelli-0.4.0-windows-x64`.
/// The `.zip` and the Inno Setup `OutputBaseFilename` both derive from this stem.
#[allow(dead_code, reason = "wired up by the windows dist orchestration in a follow-up task")]
pub fn stage_dir_name(version: &str) -> String {
    format!("gemelli-{version}-windows-x64")
}

/// File name of the plain-archive artifact, e.g. `gemelli-0.4.0-windows-x64.zip`.
#[allow(dead_code, reason = "wired up by the windows dist orchestration in a follow-up task")]
pub fn zip_name(version: &str) -> String {
    format!("{}.zip", stage_dir_name(version))
}

/// (source, destination) pairs copied into the Windows staging directory: both binaries, the
/// icon Inno Setup references at run time, and the docs bundled alongside them.
#[allow(dead_code, reason = "wired up by the windows dist orchestration in a follow-up task")]
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
#[allow(dead_code, reason = "wired up by the windows dist orchestration in a follow-up task")]
pub fn iscc_path(env_override: Option<OsString>) -> PathBuf {
    env_override
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files (x86)\Inno Setup 6\ISCC.exe"))
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
