use std::path::{Path, PathBuf};

/// Every path inside a `.app` bundle that `cargo xtask bundle` needs to create or write to,
/// precomputed from the bundle's root location so callers never hand-join path segments.
///
/// Only exercised by this module's tests until the shell layer wires `cargo xtask bundle`,
/// hence `allow(dead_code)` outside `cfg(test)`.
#[cfg_attr(not(test), allow(dead_code))]
pub struct AppBundlePaths {
    pub root: PathBuf,
    pub contents: PathBuf,
    pub macos: PathBuf,
    pub frameworks: PathBuf,
    pub resources: PathBuf,
    pub info_plist: PathBuf,
    pub executable: PathBuf,
}

impl AppBundlePaths {
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn new(dist_dir: &Path, app_bundle_name: &str, executable_name: &str) -> Self {
        let root = dist_dir.join(app_bundle_name);
        let contents = root.join("Contents");
        let macos = contents.join("MacOS");
        let frameworks = contents.join("Frameworks");
        let resources = contents.join("Resources");
        let info_plist = contents.join("Info.plist");
        let executable = macos.join(executable_name);

        Self { root, contents, macos, frameworks, resources, info_plist, executable }
    }
}

/// Directory name for a CLI distribution tarball's contents, e.g. `gemelli-0.2.0-macos-universal`.
#[cfg_attr(not(test), allow(dead_code))]
pub fn tarball_dir_name(version: &str) -> String {
    format!("gemelli-{version}-macos-universal")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_every_app_bundle_path_from_dist_dir_and_names() {
        let dist_dir = PathBuf::from("target/dist");

        let paths = AppBundlePaths::new(&dist_dir, "gemelli.app", "gemelli-gui");

        assert_eq!(paths.root, PathBuf::from("target/dist/gemelli.app"));
        assert_eq!(paths.contents, PathBuf::from("target/dist/gemelli.app/Contents"));
        assert_eq!(paths.macos, PathBuf::from("target/dist/gemelli.app/Contents/MacOS"));
        assert_eq!(paths.frameworks, PathBuf::from("target/dist/gemelli.app/Contents/Frameworks"));
        assert_eq!(paths.resources, PathBuf::from("target/dist/gemelli.app/Contents/Resources"));
        assert_eq!(paths.info_plist, PathBuf::from("target/dist/gemelli.app/Contents/Info.plist"));
        assert_eq!(
            paths.executable,
            PathBuf::from("target/dist/gemelli.app/Contents/MacOS/gemelli-gui")
        );
    }

    #[test]
    fn tarball_dir_name_embeds_version_between_fixed_segments() {
        assert_eq!(tarball_dir_name("0.2.0"), "gemelli-0.2.0-macos-universal");
    }
}
