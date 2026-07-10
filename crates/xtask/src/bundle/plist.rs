/// Values interpolated into the generated `Info.plist`. Every field is the raw display value —
/// XML metacharacters are escaped by `info_plist_xml`, not by the caller.
pub struct PlistFields<'a> {
    pub bundle_name: &'a str,
    pub display_name: &'a str,
    pub identifier: &'a str,
    pub executable: &'a str,
    pub icon_file: &'a str,
    pub short_version: &'a str,
    pub version: &'a str,
    pub min_system_version: &'a str,
    pub camera_usage_description: &'a str,
}

/// Escapes the three XML metacharacters that are unsafe inside a `<string>` element body.
/// `&` must be replaced first so its own escape sequence is not re-escaped.
fn escape_xml(value: &str) -> String {
    value.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// Renders a macOS `Info.plist` XML document for the gemelli `.app` bundle.
/// `NSCameraUsageDescription` is mandatory: without it macOS denies/crashes camera access.
pub fn info_plist_xml(fields: &PlistFields) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n\t<key>CFBundleName</key>\n\t<string>{bundle_name}</string>\n\t<key>CFBundleDisplayName</key>\n\t<string>{display_name}</string>\n\t<key>CFBundleIdentifier</key>\n\t<string>{identifier}</string>\n\t<key>CFBundleExecutable</key>\n\t<string>{executable}</string>\n\t<key>CFBundleIconFile</key>\n\t<string>{icon_file}</string>\n\t<key>CFBundleShortVersionString</key>\n\t<string>{short_version}</string>\n\t<key>CFBundleVersion</key>\n\t<string>{version}</string>\n\t<key>CFBundlePackageType</key>\n\t<string>APPL</string>\n\t<key>LSMinimumSystemVersion</key>\n\t<string>{min_system_version}</string>\n\t<key>NSHighResolutionCapable</key>\n\t<true/>\n\t<key>NSCameraUsageDescription</key>\n\t<string>{camera_usage_description}</string>\n</dict>\n</plist>\n",
        bundle_name = escape_xml(fields.bundle_name),
        display_name = escape_xml(fields.display_name),
        identifier = escape_xml(fields.identifier),
        executable = escape_xml(fields.executable),
        icon_file = escape_xml(fields.icon_file),
        short_version = escape_xml(fields.short_version),
        version = escape_xml(fields.version),
        min_system_version = escape_xml(fields.min_system_version),
        camera_usage_description = escape_xml(fields.camera_usage_description),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn gemelli_fields() -> PlistFields<'static> {
        PlistFields {
            bundle_name: "gemelli",
            display_name: "gemelli",
            identifier: "com.naporin0624.gemelli",
            executable: "gemelli-gui",
            icon_file: "icon",
            short_version: "0.2.0",
            version: "0.2.0",
            min_system_version: "11.0",
            camera_usage_description: "gemelli shares your camera feed as a Syphon texture.",
        }
    }

    #[test]
    fn renders_exact_info_plist_xml_for_gemelli_fields() {
        let rendered = info_plist_xml(&gemelli_fields());

        let expected = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n<dict>\n\t<key>CFBundleName</key>\n\t<string>gemelli</string>\n\t<key>CFBundleDisplayName</key>\n\t<string>gemelli</string>\n\t<key>CFBundleIdentifier</key>\n\t<string>com.naporin0624.gemelli</string>\n\t<key>CFBundleExecutable</key>\n\t<string>gemelli-gui</string>\n\t<key>CFBundleIconFile</key>\n\t<string>icon</string>\n\t<key>CFBundleShortVersionString</key>\n\t<string>0.2.0</string>\n\t<key>CFBundleVersion</key>\n\t<string>0.2.0</string>\n\t<key>CFBundlePackageType</key>\n\t<string>APPL</string>\n\t<key>LSMinimumSystemVersion</key>\n\t<string>11.0</string>\n\t<key>NSHighResolutionCapable</key>\n\t<true/>\n\t<key>NSCameraUsageDescription</key>\n\t<string>gemelli shares your camera feed as a Syphon texture.</string>\n</dict>\n</plist>\n";

        assert_eq!(rendered, expected);
    }

    #[test]
    fn escapes_ampersand_and_angle_brackets_in_interpolated_fields() {
        let mut fields = gemelli_fields();
        fields.display_name = "Tom & Jerry <preview>";

        let rendered = info_plist_xml(&fields);

        assert!(rendered.contains("<string>Tom &amp; Jerry &lt;preview&gt;</string>"));
        assert!(!rendered.contains("Tom & Jerry <preview>"));
    }

    #[test]
    fn escape_xml_replaces_each_metacharacter_independently() {
        assert_eq!(escape_xml("plain"), "plain");
        assert_eq!(escape_xml("a & b"), "a &amp; b");
        assert_eq!(escape_xml("<tag>"), "&lt;tag&gt;");
        assert_eq!(escape_xml("&<>"), "&amp;&lt;&gt;");
    }
}
