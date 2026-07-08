use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Every path inside a `.app` bundle that `cargo xtask bundle` needs to create or write to,
/// precomputed from the bundle's root location so callers never hand-join path segments.
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

/// Subset of a `cargo metadata --format-version 1 --no-deps` package entry needed to resolve a
/// package's version; every other field (dependencies, targets, features, ...) is left
/// undeserialized by serde.
#[derive(Deserialize)]
struct CargoMetadataPackage {
    name: String,
    version: String,
}

/// Subset of `cargo metadata --format-version 1 --no-deps` output needed to resolve a package's
/// version.
#[derive(Deserialize)]
struct CargoMetadata {
    packages: Vec<CargoMetadataPackage>,
}

/// Extracts the `gemelli-gui` package version from `cargo metadata --format-version 1 --no-deps`
/// output. This is the authoritative GUI version — xtask's own `CARGO_PKG_VERSION` names the
/// xtask crate, not the GUI, so it cannot be used as a stand-in.
pub fn gui_package_version(metadata_json: &str) -> Result<String, crate::XtaskError> {
    let metadata: CargoMetadata = serde_json::from_str(metadata_json)?;
    metadata
        .packages
        .into_iter()
        .find(|package| package.name == "gemelli-gui")
        .map(|package| package.version)
        .ok_or_else(|| crate::XtaskError::PackageNotFound("gemelli-gui".to_string()))
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

    #[test]
    fn gui_package_version_finds_gemelli_gui_among_several_workspace_packages() {
        let metadata_json = r#"{
            "packages": [
                {
                    "name": "gemelli-core",
                    "version": "0.2.0",
                    "id": "path+file:///repo#gemelli-core@0.2.0",
                    "dependencies": []
                },
                {
                    "name": "gemelli-gui",
                    "version": "0.2.0",
                    "id": "path+file:///repo#gemelli-gui@0.2.0",
                    "dependencies": []
                },
                {
                    "name": "xtask",
                    "version": "0.1.0",
                    "id": "path+file:///repo#xtask@0.1.0",
                    "dependencies": []
                }
            ],
            "workspace_members": []
        }"#;

        let version = gui_package_version(metadata_json);

        assert!(matches!(version, Ok(ref v) if v == "0.2.0"));
    }

    #[test]
    fn gui_package_version_errors_when_gemelli_gui_is_absent() {
        let metadata_json = r#"{"packages": [{"name": "gemelli-core", "version": "0.2.0"}]}"#;

        let version = gui_package_version(metadata_json);

        assert!(
            matches!(version, Err(crate::XtaskError::PackageNotFound(ref name)) if name == "gemelli-gui")
        );
    }

    #[test]
    fn gui_package_version_errors_on_invalid_json() {
        let version = gui_package_version("not json");

        assert!(matches!(version, Err(crate::XtaskError::Json(_))));
    }
}
