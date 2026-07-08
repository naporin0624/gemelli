//! Native application menu (macOS: `gemelli ▸ About / Quit`, `Help ▸ Open Source
//! Licenses…`) built with `muda`.
//!
//! `muda` is a normal (not platform-gated) dependency, so this module compiles on
//! every target; only the `Menu::init_for_nsapp` call inside `build_app_menu` —
//! which installs the menu as the app's NSApp main menu — is macOS-only.

use muda::{AboutMetadata, Menu, MenuId, MenuItem, PredefinedMenuItem, Submenu};

/// What the running app should do in response to a menu activation. `About` and
/// `Quit` are native `PredefinedMenuItem`s handled entirely by the OS (or by muda
/// itself for `Quit` on non-macOS platforms) — they never surface here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuAction {
    OpenLicenses,
}

/// Short git SHA embedded by `build.rs` via vergen-gix, or `"unknown"` when it
/// could not be determined at build time (e.g. building from a source tarball
/// with no `.git` directory — `build.rs`'s `emit_build_id` does not fail the
/// build in that case, it just leaves the var unset).
fn build_id() -> &'static str {
    option_env!("VERGEN_GIT_SHA").unwrap_or("unknown")
}

/// Assembles this app's `AboutMetadata`. Kept as a pure function (no globals, no
/// I/O beyond reading compile-time env vars) so its contents are unit-testable
/// without constructing a real menu.
fn about_metadata() -> AboutMetadata {
    AboutMetadata {
        name: Some("gemelli".to_string()),
        version: Some(env!("CARGO_PKG_VERSION").to_string()),
        short_version: Some(build_id().to_string()),
        authors: Some(vec!["naporitan".to_string()]),
        copyright: Some("\u{a9} 2026 naporitan".to_string()),
        website: Some("https://napochaan.com".to_string()),
        ..Default::default()
    }
}

/// Maps a fired `MenuEvent`'s id to the `MenuAction` it represents. `None` covers
/// ids muda already handled natively (About, Quit) or any id from a menu we
/// didn't build.
fn action_for(event_id: &MenuId, licenses_id: &MenuId) -> Option<MenuAction> {
    (event_id == licenses_id).then_some(MenuAction::OpenLicenses)
}

/// The app's native menu bar, plus the id needed to recognize its one custom item.
pub struct AppMenu {
    // Never read again after `build_app_menu` installs it, but must stay alive
    // for the app's lifetime: dropping `Menu` frees its native (NSMenu on macOS)
    // backing storage, which would tear down the menu bar it was just installed
    // as. The field is write-only in every build, so it needs an unconditional
    // `#[allow(dead_code)]`.
    #[allow(dead_code)]
    menu: Menu,
    licenses_id: MenuId,
}

impl AppMenu {
    /// Maps a fired `MenuEvent`'s id to this menu's `MenuAction`, if any.
    /// Drained via `app.rs`'s single `poll_native_events` — a custom
    /// `MenuEvent::set_event_handler` (installed once in `GemelliApp::new` so
    /// the app menu and the tray menu share one event channel) replaces muda's
    /// default receiver, so polling `MenuEvent::receiver()` here directly would
    /// see nothing.
    pub fn action_for(&self, id: &MenuId) -> Option<MenuAction> {
        action_for(id, &self.licenses_id)
    }
}

/// Builds the `gemelli ▸ About / Quit` and `Help ▸ Open Source Licenses…` menu.
///
/// On macOS this also installs it as the app's main menu (`init_for_nsapp`) —
/// safe to call here because `build_app_menu` is only ever invoked from
/// `GemelliApp::new`, which eframe calls after `NSApplication` already exists.
pub fn build_app_menu() -> Result<AppMenu, muda::Error> {
    let licenses_id = MenuId::new("gemelli-open-source-licenses");
    let licenses_item =
        MenuItem::with_id(licenses_id.clone(), "Open Source Licenses\u{2026}", true, None);

    let app_submenu = Submenu::with_items(
        "gemelli",
        true,
        &[
            &PredefinedMenuItem::about(None, Some(about_metadata())),
            &PredefinedMenuItem::separator(),
            &PredefinedMenuItem::quit(None),
        ],
    )?;
    let help_submenu = Submenu::with_items("Help", true, &[&licenses_item])?;

    let menu = Menu::with_items(&[&app_submenu, &help_submenu])?;

    #[cfg(target_os = "macos")]
    menu.init_for_nsapp();

    Ok(AppMenu { menu, licenses_id })
}

#[cfg(test)]
mod tests {
    use super::{MenuAction, about_metadata, action_for};
    use muda::MenuId;

    #[test]
    fn about_metadata_has_the_expected_fields() {
        let metadata = about_metadata();

        assert_eq!(metadata.name, Some("gemelli".to_string()));
        assert_eq!(metadata.version, Some(env!("CARGO_PKG_VERSION").to_string()));
        assert_eq!(metadata.authors, Some(vec!["naporitan".to_string()]));
        assert_eq!(metadata.copyright, Some("\u{a9} 2026 naporitan".to_string()));
        assert_eq!(metadata.website, Some("https://napochaan.com".to_string()));
        assert!(metadata.short_version.is_some());
    }

    #[test]
    fn action_for_maps_the_licenses_id_to_open_licenses() {
        let licenses_id = MenuId::new("gemelli-open-source-licenses");

        assert_eq!(action_for(&licenses_id, &licenses_id), Some(MenuAction::OpenLicenses));
    }

    #[test]
    fn action_for_ignores_ids_it_does_not_recognize() {
        let licenses_id = MenuId::new("gemelli-open-source-licenses");
        let other_id = MenuId::new("some-other-item");

        assert_eq!(action_for(&other_id, &licenses_id), None);
    }
}
