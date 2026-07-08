//! Bundled third-party license data: parsing the generated manifest and filtering it for the
//! licenses window (the window's rendering lives in this same module).

/// Which of the three sources a license entry came from. `Font`/`Native` entries are the
/// hand-written appendix (Syphon Framework, LINE Seed JP) that `cargo xtask gen-licenses` merges
/// in; `Library` is every Rust crate dependency `cargo-bundle-licenses` discovers.
///
/// Only exercised by this module's tests until the licenses window consumes it, hence
/// `allow(dead_code)` outside `cfg(test)` (same pattern as `theme.rs`'s color tokens).
#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LicenseCategory {
    Library,
    Font,
    Native,
}

#[cfg_attr(not(test), allow(dead_code))]
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct LicenseEntry {
    pub name: String,
    pub version: Option<String>,
    pub license: String,
    pub text: String,
    pub homepage: Option<String>,
    pub category: LicenseCategory,
}

/// The generated+committed license manifest written by `cargo xtask gen-licenses`.
/// `include_str!` makes a missing/unreadable file a *compile* error — the only failure mode left
/// at runtime is malformed *content*, which `parse_licenses`'s `Result` surfaces instead of
/// panicking (the workspace denies `unwrap_used`/`expect_used`, so this is the only option
/// anyway).
#[cfg_attr(not(test), allow(dead_code))]
const EMBEDDED_LICENSES_JSON: &str = include_str!("../assets/third-party-licenses.json");

/// Parses the embedded manifest. Never panics.
#[cfg_attr(not(test), allow(dead_code))]
pub fn parse_licenses(json: &str) -> Result<Vec<LicenseEntry>, serde_json::Error> {
    serde_json::from_str(json)
}

/// Case-insensitive substring match on `name`, AND'd with an optional exact `category` match.
/// `query = ""` matches every entry (the empty string is a substring of everything) — this is
/// the window's initial "nothing typed yet" state.
#[cfg_attr(not(test), allow(dead_code))]
pub fn filter_entries<'a>(
    entries: &'a [LicenseEntry],
    query: &str,
    category: Option<LicenseCategory>,
) -> Vec<&'a LicenseEntry> {
    let query_lower = query.to_lowercase();
    entries
        .iter()
        .filter(|entry| entry.name.to_lowercase().contains(&query_lower))
        .filter(|entry| category.as_ref().is_none_or(|wanted| &entry.category == wanted))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_JSON: &str = r#"[
        {
            "name": "serde",
            "version": "1.0.210",
            "license": "MIT OR Apache-2.0",
            "text": "MIT License text goes here.",
            "homepage": "https://serde.rs",
            "category": "library"
        },
        {
            "name": "LINE Seed JP",
            "version": null,
            "license": "OFL-1.1",
            "text": "SIL Open Font License text goes here.",
            "homepage": null,
            "category": "font"
        }
    ]"#;

    #[test]
    fn parse_licenses_reads_a_well_formed_manifest() {
        let entries = parse_licenses(SAMPLE_JSON).unwrap();

        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "serde");
        assert_eq!(entries[0].version, Some("1.0.210".to_string()));
        assert_eq!(entries[0].category, LicenseCategory::Library);
        assert_eq!(entries[1].name, "LINE Seed JP");
        assert_eq!(entries[1].version, None);
        assert_eq!(entries[1].category, LicenseCategory::Font);
    }

    #[test]
    fn parse_licenses_reports_malformed_json_as_an_error_not_a_panic() {
        let result = parse_licenses("{ not valid json");

        assert!(result.is_err());
    }

    #[test]
    fn filter_entries_matches_name_case_insensitively() {
        let entries = parse_licenses(SAMPLE_JSON).unwrap();

        let filtered = filter_entries(&entries, "SERDE", None);

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "serde");
    }

    #[test]
    fn filter_entries_by_category_only() {
        let entries = parse_licenses(SAMPLE_JSON).unwrap();

        let filtered = filter_entries(&entries, "", Some(LicenseCategory::Font));

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "LINE Seed JP");
    }

    #[test]
    fn filter_entries_combines_query_and_category_with_and() {
        let entries = parse_licenses(SAMPLE_JSON).unwrap();

        // Matches the query but not the category -> excluded.
        let filtered = filter_entries(&entries, "serde", Some(LicenseCategory::Font));

        assert!(filtered.is_empty());
    }

    #[test]
    fn filter_entries_with_empty_query_and_no_category_returns_everything() {
        let entries = parse_licenses(SAMPLE_JSON).unwrap();

        let filtered = filter_entries(&entries, "", None);

        assert_eq!(filtered.len(), entries.len());
    }

    #[test]
    fn committed_asset_parses_and_contains_both_appendix_entries() {
        let entries = parse_licenses(EMBEDDED_LICENSES_JSON).unwrap();

        assert!(
            entries.iter().any(|e| e.name == "Syphon Framework"),
            "committed manifest is missing the Syphon Framework appendix entry"
        );
        assert!(
            entries.iter().any(|e| e.name == "LINE Seed JP"),
            "committed manifest is missing the LINE Seed JP appendix entry"
        );
    }
}
