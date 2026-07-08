use std::{
    ffi::OsString,
    path::{Path, PathBuf},
};

/// `lipo -create <inputs...> -output <output>` — combines per-architecture binaries into one
/// universal2 binary.
///
/// Only exercised by this module's tests until the shell layer wires `cargo xtask bundle`/`dist`,
/// hence `allow(dead_code)` outside `cfg(test)` on every function below.
#[cfg_attr(not(test), allow(dead_code))]
pub fn lipo_create_args(inputs: &[PathBuf], output: &Path) -> Vec<OsString> {
    let mut args = vec![OsString::from("-create")];
    args.extend(inputs.iter().map(|input| input.as_os_str().to_os_string()));
    args.push(OsString::from("-output"));
    args.push(output.as_os_str().to_os_string());
    args
}

/// `install_name_tool -add_rpath <rpath> <binary>` — points a binary at the framework search
/// path appropriate to where it was placed in the distribution layout.
#[cfg_attr(not(test), allow(dead_code))]
pub fn add_rpath_args(rpath: &str, binary: &Path) -> Vec<OsString> {
    vec![OsString::from("-add_rpath"), OsString::from(rpath), binary.as_os_str().to_os_string()]
}

/// `cargo build --release -p <package> --target <target>` — builds one architecture slice
/// ahead of a `lipo -create` combine step.
#[cfg_attr(not(test), allow(dead_code))]
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
#[cfg_attr(not(test), allow(dead_code))]
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
}
