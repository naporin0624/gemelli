//! Stable device selection: resolves a user- or config-supplied selector
//! against the current device enumeration.
//!
//! AVFoundation reassigns numeric device indices between process runs (OBS
//! Virtual Camera has been observed flipping between index 0 and 1), so
//! anything persisted across runs (GUI config) must key off `DeviceId`
//! instead of `DeviceInfo::index`.

use std::fmt;

use crate::capture::DeviceInfo;

/// Opaque, backend-defined stable identifier (AVFoundation uniqueID today,
/// Windows symbolic-link path later). Never parsed, only compared.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceId(String);

impl DeviceId {
    /// None for empty/whitespace-only input — an absent id must be
    /// `Option::None`, never a sentinel empty string.
    pub fn new(raw: &str) -> Option<Self> {
        if raw.trim().is_empty() { None } else { Some(Self(raw.to_string())) }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// How a caller identifies which capture device it wants.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeviceSelector {
    /// Positional index into the current enumeration order (unstable across
    /// runs; kept for interactive use and backward compat).
    Index(u32),
    /// Exact stable-id equality only — what the GUI persists.
    Id(DeviceId),
    /// Free text from the CLI: matched against names and ids by tier.
    Query(String),
}

impl DeviceSelector {
    pub fn resolve<'a>(&self, devices: &'a [DeviceInfo]) -> Result<&'a DeviceInfo, SelectError> {
        match self {
            Self::Index(index) => resolve_index(*index, devices),
            Self::Id(id) => resolve_id(id, devices),
            Self::Query(query) => resolve_query(query, devices),
        }
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum SelectError {
    #[error("device index {index} not found\n{}", format_devices(available))]
    IndexOutOfRange { index: u32, available: Vec<DeviceInfo> },
    #[error("no device matches \"{query}\"\n{}", format_devices(available))]
    NoMatch { query: String, available: Vec<DeviceInfo> },
    #[error("\"{query}\" matches multiple devices\n{}", format_devices(matches))]
    Ambiguous { query: String, matches: Vec<DeviceInfo> },
}

/// Formats one device as `"<index>: <name>  [<id>]"`, omitting the bracket
/// when the device has no stable id.
pub fn device_line(device: &DeviceInfo) -> String {
    match &device.id {
        Some(id) => format!("{}: {}  [{id}]", device.index, device.name),
        None => format!("{}: {}", device.index, device.name),
    }
}

pub fn format_devices(devices: &[DeviceInfo]) -> String {
    devices.iter().map(device_line).collect::<Vec<_>>().join("\n")
}

fn resolve_index(index: u32, devices: &[DeviceInfo]) -> Result<&DeviceInfo, SelectError> {
    devices
        .iter()
        .find(|device| device.index == index)
        .ok_or_else(|| SelectError::IndexOutOfRange { index, available: devices.to_vec() })
}

fn resolve_id<'a>(id: &DeviceId, devices: &'a [DeviceInfo]) -> Result<&'a DeviceInfo, SelectError> {
    devices
        .iter()
        .find(|device| device.id.as_ref() == Some(id))
        .ok_or_else(|| SelectError::NoMatch { query: id.to_string(), available: devices.to_vec() })
}

/// Tier 1: case-insensitive exact name match.
fn match_exact_name<'a>(query: &str, devices: &'a [DeviceInfo]) -> Vec<&'a DeviceInfo> {
    let query_lower = query.to_lowercase();
    devices.iter().filter(|device| device.name.to_lowercase() == query_lower).collect()
}

/// Tier 2: case-insensitive exact id match.
fn match_exact_id<'a>(query: &str, devices: &'a [DeviceInfo]) -> Vec<&'a DeviceInfo> {
    let query_lower = query.to_lowercase();
    devices
        .iter()
        .filter(|device| {
            device.id.as_ref().is_some_and(|id| id.as_str().to_lowercase() == query_lower)
        })
        .collect()
}

/// Tier 3: case-insensitive name prefix match.
fn match_name_prefix<'a>(query: &str, devices: &'a [DeviceInfo]) -> Vec<&'a DeviceInfo> {
    let query_lower = query.to_lowercase();
    devices.iter().filter(|device| device.name.to_lowercase().starts_with(&query_lower)).collect()
}

/// Tier 4: case-insensitive name substring match.
fn match_name_substring<'a>(query: &str, devices: &'a [DeviceInfo]) -> Vec<&'a DeviceInfo> {
    let query_lower = query.to_lowercase();
    devices.iter().filter(|device| device.name.to_lowercase().contains(&query_lower)).collect()
}

/// Turns one tier's hits into a resolution outcome, or `None` to fall
/// through to the next tier. `>1` hit within a tier is always ambiguous —
/// later tiers never get a chance to disambiguate it.
fn settle_tier<'a>(
    hits: Vec<&'a DeviceInfo>,
    query: &str,
) -> Option<Result<&'a DeviceInfo, SelectError>> {
    match hits.as_slice() {
        [] => None,
        [only] => Some(Ok(only)),
        _ => Some(Err(SelectError::Ambiguous {
            query: query.to_string(),
            matches: hits.into_iter().cloned().collect(),
        })),
    }
}

fn resolve_query<'a>(
    query: &str,
    devices: &'a [DeviceInfo],
) -> Result<&'a DeviceInfo, SelectError> {
    // An empty/whitespace-only query would otherwise reach the name-prefix tier, where an empty
    // needle prefixes every device name — resolving the sole device, or reporting `Ambiguous`
    // once there's more than one, instead of the `NoMatch` an unanswered prompt should be.
    if query.trim().is_empty() {
        return Err(SelectError::NoMatch { query: query.to_string(), available: devices.to_vec() });
    }
    if let Some(outcome) = settle_tier(match_exact_name(query, devices), query) {
        return outcome;
    }
    if let Some(outcome) = settle_tier(match_exact_id(query, devices), query) {
        return outcome;
    }
    if let Some(outcome) = settle_tier(match_name_prefix(query, devices), query) {
        return outcome;
    }
    if let Some(outcome) = settle_tier(match_name_substring(query, devices), query) {
        return outcome;
    }

    Err(SelectError::NoMatch { query: query.to_string(), available: devices.to_vec() })
}

#[cfg(test)]
mod tests {
    use super::{DeviceId, DeviceSelector, SelectError, device_line, format_devices};
    use crate::capture::DeviceInfo;

    fn device(index: u32, name: &str, id: Option<&str>) -> DeviceInfo {
        DeviceInfo { index, name: name.to_string(), id: id.and_then(DeviceId::new) }
    }

    #[test]
    fn device_id_rejects_empty() {
        assert_eq!(DeviceId::new(""), None);
        assert_eq!(DeviceId::new("   "), None);
    }

    #[test]
    fn device_id_keeps_value() {
        let id = DeviceId::new("uuid-1").expect("non-empty id");

        assert_eq!(id.as_str(), "uuid-1");
    }

    #[test]
    fn resolve_index_in_range() {
        let devices = vec![device(0, "Cam A", None), device(1, "Cam B", None)];

        let resolved = DeviceSelector::Index(1).resolve(&devices).expect("index 1 exists");

        assert_eq!(resolved.name, "Cam B");
    }

    #[test]
    fn resolve_index_out_of_range() {
        let devices = vec![device(0, "Cam A", Some("id-a")), device(1, "Cam B", Some("id-b"))];

        let error = DeviceSelector::Index(5).resolve(&devices).expect_err("index 5 missing");

        assert!(matches!(error, SelectError::IndexOutOfRange { index: 5, .. }));
        let message = error.to_string();
        assert!(message.contains("Cam A"));
        assert!(message.contains("Cam B"));
        assert!(message.contains("id-a"));
        assert!(message.contains("id-b"));
    }

    #[test]
    fn resolve_id_exact_match() {
        let devices = vec![device(0, "Cam A", Some("id-a")), device(1, "Cam B", Some("id-b"))];
        let selector = DeviceSelector::Id(DeviceId::new("id-b").expect("non-empty"));

        let resolved = selector.resolve(&devices).expect("id-b exists");

        assert_eq!(resolved.name, "Cam B");
    }

    #[test]
    fn resolve_id_missing_is_no_match() {
        let devices = vec![device(0, "Cam A", Some("id-a"))];
        let selector = DeviceSelector::Id(DeviceId::new("id-missing").expect("non-empty"));

        let error = selector.resolve(&devices).expect_err("id-missing absent");

        assert!(matches!(error, SelectError::NoMatch { .. }));
    }

    #[test]
    fn query_exact_name_case_insensitive() {
        let devices = vec![device(0, "Cam A", None)];

        let resolved =
            DeviceSelector::Query("cam a".to_string()).resolve(&devices).expect("case-insensitive");

        assert_eq!(resolved.name, "Cam A");
    }

    #[test]
    fn query_exact_name_beats_prefix() {
        let devices = vec![device(0, "Cam", None), device(1, "Camera 2", None)];

        let resolved =
            DeviceSelector::Query("Cam".to_string()).resolve(&devices).expect("exact name wins");

        assert_eq!(resolved.name, "Cam");
    }

    #[test]
    fn query_duplicate_names_ambiguous() {
        let devices =
            vec![device(0, "USB Camera", Some("id-a")), device(1, "USB Camera", Some("id-b"))];

        let error = DeviceSelector::Query("USB Camera".to_string())
            .resolve(&devices)
            .expect_err("duplicate names");

        assert!(matches!(error, SelectError::Ambiguous { .. }));
        let message = error.to_string();
        assert!(message.contains("id-a"));
        assert!(message.contains("id-b"));
    }

    #[test]
    fn query_exact_id_after_name() {
        let devices = vec![device(0, "Cam A", Some("7626645E-1E13-4E6F-8B77-71B2A5B5F1C7"))];

        let resolved = DeviceSelector::Query("7626645E-1E13-4E6F-8B77-71B2A5B5F1C7".to_string())
            .resolve(&devices)
            .expect("uuid matches");
        let resolved_lower =
            DeviceSelector::Query("7626645e-1e13-4e6f-8b77-71b2a5b5f1c7".to_string())
                .resolve(&devices)
                .expect("lowercased uuid matches");

        assert_eq!(resolved.name, "Cam A");
        assert_eq!(resolved_lower.name, "Cam A");
    }

    #[test]
    fn query_unique_prefix_matches() {
        let devices = vec![device(0, "Logitech C920", None), device(1, "OBS Virtual Camera", None)];

        let resolved =
            DeviceSelector::Query("Logi".to_string()).resolve(&devices).expect("unique prefix");

        assert_eq!(resolved.name, "Logitech C920");
    }

    #[test]
    fn query_prefix_ambiguous() {
        let devices = vec![device(0, "Cam Front", None), device(1, "Cam Back", None)];

        let error = DeviceSelector::Query("Cam".to_string())
            .resolve(&devices)
            .expect_err("ambiguous prefix");

        assert!(matches!(error, SelectError::Ambiguous { .. }));
    }

    #[test]
    fn query_unique_substring_matches() {
        let devices = vec![device(0, "OBS Virtual Camera", None), device(1, "Logitech C920", None)];

        let resolved =
            DeviceSelector::Query("obs".to_string()).resolve(&devices).expect("unique substring");

        assert_eq!(resolved.name, "OBS Virtual Camera");
    }

    #[test]
    fn query_substring_ambiguous() {
        let devices = vec![device(0, "Front Camera", None), device(1, "Back Camera", None)];

        let error = DeviceSelector::Query("camera".to_string())
            .resolve(&devices)
            .expect_err("ambiguous substring");

        assert!(matches!(error, SelectError::Ambiguous { .. }));
    }

    #[test]
    fn query_empty_is_no_match() {
        // An empty (or whitespace-only) query must not fall through to the
        // name-prefix tier, where an empty needle prefixes every name —
        // that would resolve the sole device below, or report `Ambiguous`
        // once there's more than one, instead of the `NoMatch` this ought
        // to be.
        let devices = vec![device(0, "Cam A", Some("id-a"))];

        for query in ["", "   "] {
            let error = DeviceSelector::Query(query.to_string())
                .resolve(&devices)
                .expect_err("empty/whitespace query must not resolve");

            assert!(matches!(error, SelectError::NoMatch { .. }), "query: {query:?}");
            let message = error.to_string();
            assert!(message.contains("Cam A"), "query: {query:?}");
        }
    }

    #[test]
    fn query_no_match_lists_devices() {
        let devices = vec![device(0, "Cam A", Some("id-a"))];

        let error = DeviceSelector::Query("nonexistent".to_string())
            .resolve(&devices)
            .expect_err("no device matches");

        assert!(matches!(error, SelectError::NoMatch { .. }));
        let message = error.to_string();
        assert!(message.contains("Cam A"));
        assert!(message.contains("id-a"));
    }

    #[test]
    fn device_line_includes_id() {
        let line = device_line(&device(0, "Cam A", Some("id-a")));

        assert_eq!(line, "0: Cam A  [id-a]");
    }

    #[test]
    fn device_line_omits_missing_id() {
        let line = device_line(&device(2, "Cam C", None));

        assert_eq!(line, "2: Cam C");
    }

    #[test]
    fn format_devices_joins_lines() {
        let devices = vec![device(0, "Cam A", Some("id-a")), device(1, "Cam B", None)];

        let formatted = format_devices(&devices);

        assert_eq!(formatted, "0: Cam A  [id-a]\n1: Cam B");
    }
}
