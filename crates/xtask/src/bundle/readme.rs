/// Renders the `README.txt` shipped inside a CLI distribution tarball, explaining what the
/// tarball is, the unsigned-build quarantine workaround, and the `Syphon.framework` co-location
/// requirement the binary's rpath depends on.
pub fn cli_readme_txt(version: &str) -> String {
    format!(
        "gemelli {version} (macOS CLI)\n\nWhat this is:\ngemelli captures a webcam and shares it as a Syphon texture that other\nmacOS apps can consume.\n\nFirst run (Gatekeeper):\nThis build is unsigned. Before running it, remove the quarantine\nattribute from this directory:\n\n    xattr -dr com.apple.quarantine <path-to-this-directory>\n\nSyphon.framework:\nSyphon.framework must stay in this directory, next to the `gemelli`\nbinary. The binary resolves it via a relative rpath and will not run if\nthe framework is moved elsewhere.\n\nGetting started:\n\n    ./gemelli --help\n"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_exact_readme_txt_for_a_version() {
        let rendered = cli_readme_txt("0.2.0");

        let expected = "gemelli 0.2.0 (macOS CLI)\n\nWhat this is:\ngemelli captures a webcam and shares it as a Syphon texture that other\nmacOS apps can consume.\n\nFirst run (Gatekeeper):\nThis build is unsigned. Before running it, remove the quarantine\nattribute from this directory:\n\n    xattr -dr com.apple.quarantine <path-to-this-directory>\n\nSyphon.framework:\nSyphon.framework must stay in this directory, next to the `gemelli`\nbinary. The binary resolves it via a relative rpath and will not run if\nthe framework is moved elsewhere.\n\nGetting started:\n\n    ./gemelli --help\n";

        assert_eq!(rendered, expected);
    }

    #[test]
    fn embeds_the_given_version_in_the_title_line() {
        let rendered = cli_readme_txt("9.9.9");

        assert!(rendered.starts_with("gemelli 9.9.9 (macOS CLI)\n"));
    }
}
