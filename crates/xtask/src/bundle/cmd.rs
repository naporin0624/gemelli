use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

/// `lipo -create <inputs...> -output <output>` — combines per-architecture binaries into one
/// universal2 binary.
pub fn lipo_create_args(inputs: &[PathBuf], output: &Path) -> Vec<OsString> {
    let mut args = vec![OsString::from("-create")];
    args.extend(inputs.iter().map(|input| input.as_os_str().to_os_string()));
    args.push(OsString::from("-output"));
    args.push(output.as_os_str().to_os_string());
    args
}

/// `install_name_tool -add_rpath <rpath> <binary>` — points a binary at the framework search
/// path appropriate to where it was placed in the distribution layout.
pub fn add_rpath_args(rpath: &str, binary: &Path) -> Vec<OsString> {
    vec![OsString::from("-add_rpath"), OsString::from(rpath), binary.as_os_str().to_os_string()]
}

/// `cargo build --release -p <package> --target <target>` — builds one architecture slice
/// ahead of a `lipo -create` combine step.
pub fn cargo_build_target_args(package: &str, target: &str) -> Vec<OsString> {
    vec![
        OsString::from("build"),
        OsString::from("--release"),
        OsString::from("-p"),
        OsString::from(package),
        OsString::from("--target"),
        OsString::from(target),
    ]
}

/// `hdiutil create -volname <volume> -srcfolder <srcfolder> -ov -format UDZO <output>` —
/// packages a folder (the `.app` bundle) into a compressed, overwrite-if-exists `.dmg`.
pub fn hdiutil_create_args(volume: &str, srcfolder: &Path, output: &Path) -> Vec<OsString> {
    vec![
        OsString::from("create"),
        OsString::from("-volname"),
        OsString::from(volume),
        OsString::from("-srcfolder"),
        srcfolder.as_os_str().to_os_string(),
        OsString::from("-ov"),
        OsString::from("-format"),
        OsString::from("UDZO"),
        output.as_os_str().to_os_string(),
    ]
}

/// `tar czf <output> -C <chdir> <entry>` — archives `entry` (a directory name relative to
/// `chdir`) without embedding `chdir`'s own absolute path in the tarball.
pub fn tar_czf_args(output: &Path, chdir: &Path, entry: &str) -> Vec<OsString> {
    vec![
        OsString::from("czf"),
        output.as_os_str().to_os_string(),
        OsString::from("-C"),
        chdir.as_os_str().to_os_string(),
        OsString::from(entry),
    ]
}

/// `cargo build --release -p <pkg>...` — host-target release build of the given packages,
/// used on Windows where a single-arch build (no `--target`) is all that's needed.
#[allow(dead_code, reason = "wired up by the Windows packaging task that calls this builder")]
pub fn cargo_build_release_args(packages: &[&str]) -> Vec<OsString> {
    let mut args = vec![OsString::from("build"), OsString::from("--release")];
    for package in packages {
        args.push(OsString::from("-p"));
        args.push(OsString::from(*package));
    }
    args
}

/// `tar -a -c -f <output> -C <chdir> <entry>` — bsdtar (bundled with Windows 10+) infers the
/// zip format from the `.zip` extension via `-a`, avoiding a zip crate dependency.
#[allow(dead_code, reason = "wired up by the Windows packaging task that calls this builder")]
pub fn tar_zip_args(output: &Path, chdir: &Path, entry: &str) -> Vec<OsString> {
    vec![
        OsString::from("-a"),
        OsString::from("-c"),
        OsString::from("-f"),
        output.as_os_str().to_os_string(),
        OsString::from("-C"),
        chdir.as_os_str().to_os_string(),
        OsString::from(entry),
    ]
}

/// `ISCC.exe /DMyAppVersion=<v> /DSourceDir=<dir> /DOutputDir=<dir> <script.iss>` — ISCC
/// resolves relative paths against the .iss file's directory, not the CWD, so callers must
/// pass absolute staging/output paths.
#[allow(dead_code, reason = "wired up by the Windows packaging task that calls this builder")]
pub fn iscc_args(version: &str, source_dir: &Path, output_dir: &Path, iss: &Path) -> Vec<OsString> {
    let mut source_define = OsString::from("/DSourceDir=");
    source_define.push(source_dir.as_os_str());
    let mut output_define = OsString::from("/DOutputDir=");
    output_define.push(output_dir.as_os_str());
    vec![
        OsString::from(format!("/DMyAppVersion={version}")),
        source_define,
        output_define,
        iss.as_os_str().to_os_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lipo_create_args_lists_each_input_then_output_flag() {
        let inputs = vec![
            PathBuf::from("target/aarch64/gemelli-gui"),
            PathBuf::from("target/x86_64/gemelli-gui"),
        ];
        let output = PathBuf::from("target/dist/gemelli-gui");

        let args = lipo_create_args(&inputs, &output);

        assert_eq!(
            args,
            vec![
                OsString::from("-create"),
                OsString::from("target/aarch64/gemelli-gui"),
                OsString::from("target/x86_64/gemelli-gui"),
                OsString::from("-output"),
                OsString::from("target/dist/gemelli-gui"),
            ]
        );
    }

    #[test]
    fn add_rpath_args_is_flag_rpath_then_binary() {
        let binary = PathBuf::from("target/dist/gemelli.app/Contents/MacOS/gemelli-gui");

        let args = add_rpath_args("@executable_path/../Frameworks", &binary);

        assert_eq!(
            args,
            vec![
                OsString::from("-add_rpath"),
                OsString::from("@executable_path/../Frameworks"),
                OsString::from("target/dist/gemelli.app/Contents/MacOS/gemelli-gui"),
            ]
        );
    }

    #[test]
    fn cargo_build_target_args_matches_release_build_invocation() {
        let args = cargo_build_target_args("gemelli-gui", "aarch64-apple-darwin");

        assert_eq!(
            args,
            vec![
                OsString::from("build"),
                OsString::from("--release"),
                OsString::from("-p"),
                OsString::from("gemelli-gui"),
                OsString::from("--target"),
                OsString::from("aarch64-apple-darwin"),
            ]
        );
    }

    #[test]
    fn hdiutil_create_args_matches_udzo_dmg_invocation() {
        let srcfolder = PathBuf::from("target/dist/gemelli.app");
        let output = PathBuf::from("target/dist/gemelli-0.2.0-macos.dmg");

        let args = hdiutil_create_args("gemelli", &srcfolder, &output);

        assert_eq!(
            args,
            vec![
                OsString::from("create"),
                OsString::from("-volname"),
                OsString::from("gemelli"),
                OsString::from("-srcfolder"),
                OsString::from("target/dist/gemelli.app"),
                OsString::from("-ov"),
                OsString::from("-format"),
                OsString::from("UDZO"),
                OsString::from("target/dist/gemelli-0.2.0-macos.dmg"),
            ]
        );
    }

    #[test]
    fn tar_czf_args_chdirs_before_naming_the_entry() {
        let output = PathBuf::from("target/dist/gemelli-0.2.0-macos-universal.tar.gz");
        let chdir = PathBuf::from("target/dist");

        let args = tar_czf_args(&output, &chdir, "gemelli-0.2.0-macos-universal");

        assert_eq!(
            args,
            vec![
                OsString::from("czf"),
                OsString::from("target/dist/gemelli-0.2.0-macos-universal.tar.gz"),
                OsString::from("-C"),
                OsString::from("target/dist"),
                OsString::from("gemelli-0.2.0-macos-universal"),
            ]
        );
    }

    #[test]
    fn cargo_build_release_args_lists_each_package_after_its_own_flag() {
        let args = cargo_build_release_args(&["gemelli-cli", "gemelli-gui"]);

        assert_eq!(
            args,
            vec![
                OsString::from("build"),
                OsString::from("--release"),
                OsString::from("-p"),
                OsString::from("gemelli-cli"),
                OsString::from("-p"),
                OsString::from("gemelli-gui"),
            ]
        );
    }

    #[test]
    fn tar_zip_args_uses_auto_format_and_chdirs_before_naming_the_entry() {
        let output = PathBuf::from("target/dist/gemelli-0.4.0-windows-x64.zip");
        let chdir = PathBuf::from("target/dist");

        let args = tar_zip_args(&output, &chdir, "gemelli-0.4.0-windows-x64");

        assert_eq!(
            args,
            vec![
                OsString::from("-a"),
                OsString::from("-c"),
                OsString::from("-f"),
                OsString::from("target/dist/gemelli-0.4.0-windows-x64.zip"),
                OsString::from("-C"),
                OsString::from("target/dist"),
                OsString::from("gemelli-0.4.0-windows-x64"),
            ]
        );
    }

    #[test]
    fn iscc_args_passes_defines_then_script_path() {
        let source_dir = PathBuf::from("/work/target/dist/gemelli-0.4.0-windows-x64");
        let output_dir = PathBuf::from("/work/target/dist");
        let iss = PathBuf::from("/work/packaging/windows/gemelli.iss");

        let args = iscc_args("0.4.0", &source_dir, &output_dir, &iss);

        assert_eq!(
            args,
            vec![
                OsString::from("/DMyAppVersion=0.4.0"),
                OsString::from("/DSourceDir=/work/target/dist/gemelli-0.4.0-windows-x64"),
                OsString::from("/DOutputDir=/work/target/dist"),
                OsString::from("/work/packaging/windows/gemelli.iss"),
            ]
        );
    }
}
