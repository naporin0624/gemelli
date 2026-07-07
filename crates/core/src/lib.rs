//! Core library placeholder for the webcam -> Spout/Syphon sharing tool.
//!
//! This crate currently only exposes scaffolding used to verify the
//! workspace builds correctly. Real capture/conversion logic is added later.

/// Returns the crate's package name as reported by Cargo at compile time.
///
/// Placeholder used to confirm downstream crates can link against `core`.
pub fn crate_name() -> &'static str {
    env!("CARGO_PKG_NAME")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches_package() {
        assert_eq!(crate_name(), "webcam-sharedtexture-core");
    }
}
