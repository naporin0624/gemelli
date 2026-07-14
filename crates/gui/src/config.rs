//! Persisted GUI configuration. Only the selected capture device is
//! persisted so far; `#[serde(default)]` keeps a config written by an
//! older/newer build loadable — missing fields fall back to `None` rather
//! than failing deserialization outright.

use gemelli_core::capture::DeviceInfo;

#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct GuiConfig {
    /// `DeviceId::as_str()` of the last-selected device. Plain `String` (not
    /// `DeviceId`) so this module doesn't need core's `serde` support.
    pub device_id: Option<String>,
    /// Display name at save time, kept only to name the device in the
    /// missing-camera banner if `device_id` no longer resolves.
    pub device_name: Option<String>,
}

pub fn load_config(storage: Option<&dyn eframe::Storage>) -> GuiConfig {
    storage.and_then(|storage| eframe::get_value(storage, eframe::APP_KEY)).unwrap_or_default()
}

pub fn save_config(storage: &mut dyn eframe::Storage, config: &GuiConfig) {
    eframe::set_value(storage, eframe::APP_KEY, config);
}

/// Finds the device whose stable id matches `id`, wherever it now sits in
/// `devices` — the whole reason to key off id instead of a remembered index
/// is that index shifts when devices are added/removed.
pub fn position_of_id(devices: &[DeviceInfo], id: &str) -> Option<usize> {
    devices
        .iter()
        .position(|device| device.id.as_ref().is_some_and(|device_id| device_id.as_str() == id))
}

fn missing_camera_banner(saved_name: &str, devices: &[DeviceInfo]) -> String {
    match devices.first() {
        Some(first) => {
            format!("saved camera \"{saved_name}\" is not connected — using {}", first.name)
        }
        None => format!("saved camera \"{saved_name}\" is not connected"),
    }
}

/// Pure: (saved config, fresh device list) -> (selected position, optional
/// banner). Never returns an out-of-bounds position; `devices` empty and no
/// match both settle on position `0`, which callers must treat as "no
/// selection" when `devices` is also empty.
pub fn restore_selection(config: &GuiConfig, devices: &[DeviceInfo]) -> (usize, Option<String>) {
    let Some(saved_id) = config.device_id.as_deref() else {
        return (0, None);
    };
    if let Some(position) = position_of_id(devices, saved_id) {
        return (position, None);
    }

    let saved_name = config.device_name.as_deref().unwrap_or(saved_id);
    (0, Some(missing_camera_banner(saved_name, devices)))
}

#[cfg(test)]
mod tests {
    use gemelli_core::capture::DeviceInfo;
    use gemelli_core::selector::DeviceId;

    use super::{GuiConfig, position_of_id, restore_selection};

    fn device(index: u32, name: &str, id: Option<&str>) -> DeviceInfo {
        DeviceInfo { index, name: name.to_string(), id: id.and_then(DeviceId::new) }
    }

    #[test]
    fn position_of_id_finds_matching_device() {
        let devices = vec![device(0, "Cam A", Some("id-a")), device(1, "Cam B", Some("id-b"))];

        assert_eq!(position_of_id(&devices, "id-b"), Some(1));
    }

    #[test]
    fn position_of_id_missing_is_none() {
        let devices = vec![device(0, "Cam A", Some("id-a"))];

        assert_eq!(position_of_id(&devices, "id-missing"), None);
    }

    #[test]
    fn position_of_id_ignores_devices_without_an_id() {
        let devices = vec![device(0, "Cam A", None)];

        assert_eq!(position_of_id(&devices, "id-a"), None);
    }

    #[test]
    fn restore_selection_finds_saved_id_in_a_shuffled_list() {
        // The saved device used to be at index 0; a reorder (unplug/replug,
        // OS re-enumeration) now puts it at index 1 — restore must follow
        // the id, not the remembered position.
        let config = GuiConfig { device_id: Some("id-b".to_string()), device_name: None };
        let devices = vec![device(0, "Cam A", Some("id-a")), device(1, "Cam B", Some("id-b"))];

        let (position, banner) = restore_selection(&config, &devices);

        assert_eq!(position, 1);
        assert_eq!(banner, None);
    }

    #[test]
    fn restore_selection_missing_device_banners_with_saved_name() {
        let config = GuiConfig {
            device_id: Some("id-gone".to_string()),
            device_name: Some("Old Cam".to_string()),
        };
        let devices = vec![device(0, "Cam A", Some("id-a"))];

        let (position, banner) = restore_selection(&config, &devices);

        assert_eq!(position, 0);
        let message = banner.expect("missing device must banner");
        assert!(message.contains("Old Cam"));
        assert!(message.contains("Cam A"));
    }

    #[test]
    fn restore_selection_missing_device_with_no_devices_still_banners() {
        let config = GuiConfig {
            device_id: Some("id-gone".to_string()),
            device_name: Some("Old Cam".to_string()),
        };

        let (position, banner) = restore_selection(&config, &[]);

        assert_eq!(position, 0);
        let message = banner.expect("missing device must banner even with no devices present");
        assert!(message.contains("Old Cam"));
    }

    #[test]
    fn restore_selection_empty_config_defaults_to_first_with_no_banner() {
        let config = GuiConfig::default();
        let devices = vec![device(0, "Cam A", Some("id-a"))];

        let (position, banner) = restore_selection(&config, &devices);

        assert_eq!(position, 0);
        assert_eq!(banner, None);
    }

    #[test]
    fn gui_config_serde_round_trip_tolerates_missing_fields() {
        let config: GuiConfig = serde_json::from_str("{}").expect("missing fields use defaults");

        assert_eq!(config, GuiConfig::default());
    }

    #[test]
    fn gui_config_serde_round_trip_preserves_saved_values() {
        let config = GuiConfig {
            device_id: Some("id-a".to_string()),
            device_name: Some("Cam A".to_string()),
        };

        let json = serde_json::to_string(&config).expect("serializes");
        let round_tripped: GuiConfig = serde_json::from_str(&json).expect("deserializes");

        assert_eq!(round_tripped, config);
    }
}
