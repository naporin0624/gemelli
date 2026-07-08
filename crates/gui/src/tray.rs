//! Menu-bar tray icon (`Show gemelli` / `Quit gemelli`) built with `tray-icon`.
//!
//! `tray-icon` re-exports the same `muda` version this crate depends on directly
//! (verified via `cargo tree -p gemelli-gui -i muda`), so `muda::MenuEvent` and
//! `tray_icon::menu::MenuEvent` are the same type on the same global event
//! channel — no separate tray-event plumbing is needed, only `MenuId` mapping.

use muda::{Menu, MenuId, MenuItem};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};

const TRAY_ICON_BYTES: &[u8] = include_bytes!("../assets/tray-icon.png");

/// What the running app should do in response to a tray menu activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayAction {
    Show,
    Quit,
}

/// A decoded RGBA image ready for `tray_icon::Icon::from_rgba`.
pub struct DecodedIcon {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, thiserror::Error)]
pub enum TrayError {
    #[error(transparent)]
    Png(#[from] png::DecodingError),
    #[error(transparent)]
    BadIcon(#[from] tray_icon::BadIcon),
    #[error(transparent)]
    Tray(#[from] tray_icon::Error),
    #[error(transparent)]
    Menu(#[from] muda::Error),
    #[error("tray icon PNG is not 8-bit RGBA")]
    UnexpectedFormat,
}

/// Decodes `bytes` as a PNG into raw RGBA, rejecting any format that isn't the
/// 8-bit RGBA this module assumes everywhere else (no runtime palette/greyscale
/// expansion is implemented). Pure aside from the decode itself — no I/O.
fn decode_icon(bytes: &[u8]) -> Result<DecodedIcon, TrayError> {
    let mut reader = png::Decoder::new(std::io::Cursor::new(bytes)).read_info()?;
    // `output_buffer_size` returns `None` only when the frame doesn't fit this
    // machine's address space; `unwrap_or(0)` lets that case fall through to
    // `next_frame`'s own buffer-too-small `DecodingError` rather than needing a
    // second error path here.
    let mut buffer = vec![0u8; reader.output_buffer_size().unwrap_or(0)];
    let info = reader.next_frame(&mut buffer)?;

    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return Err(TrayError::UnexpectedFormat);
    }

    buffer.truncate(info.buffer_size());
    Ok(DecodedIcon { rgba: buffer, width: info.width, height: info.height })
}

/// Maps a fired `MenuEvent`'s id to the `TrayAction` it represents. `None`
/// covers any id from a menu we didn't build (e.g. the app menu's licenses
/// item, or a foreign event).
fn action_for(id: &MenuId, show_id: &MenuId, quit_id: &MenuId) -> Option<TrayAction> {
    if id == show_id {
        Some(TrayAction::Show)
    } else if id == quit_id {
        Some(TrayAction::Quit)
    } else {
        None
    }
}

/// The tray icon, plus the ids needed to recognize its two menu items.
pub struct AppTray {
    // Never read again after `build_tray` installs it, but must stay alive for
    // the app's lifetime: dropping `TrayIcon` removes the status item from the
    // menu bar. Write-only in every build, so it needs an unconditional
    // `#[allow(dead_code)]` (mirrors `AppMenu::menu` in menu.rs).
    #[allow(dead_code)]
    tray: TrayIcon,
    show_id: MenuId,
    quit_id: MenuId,
}

impl AppTray {
    /// Maps a fired `MenuEvent`'s id to this tray's `TrayAction`, if any.
    pub fn action_for(&self, id: &MenuId) -> Option<TrayAction> {
        action_for(id, &self.show_id, &self.quit_id)
    }
}

/// Builds the menu-bar tray icon with a `Show gemelli` / `Quit gemelli` menu.
/// Left-click opens the menu (the `tray-icon` default) rather than toggling the
/// window directly.
pub fn build_tray() -> Result<AppTray, TrayError> {
    let show_id = MenuId::new("gemelli-tray-show");
    let quit_id = MenuId::new("gemelli-tray-quit");
    let show_item = MenuItem::with_id(show_id.clone(), "Show gemelli", true, None);
    let quit_item = MenuItem::with_id(quit_id.clone(), "Quit gemelli", true, None);
    let menu = Menu::with_items(&[&show_item, &quit_item])?;

    let decoded = decode_icon(TRAY_ICON_BYTES)?;
    let icon = Icon::from_rgba(decoded.rgba, decoded.width, decoded.height)?;

    let tray = TrayIconBuilder::new()
        .with_icon(icon)
        .with_tooltip("gemelli")
        .with_menu(Box::new(menu))
        .build()?;

    Ok(AppTray { tray, show_id, quit_id })
}

#[cfg(test)]
mod tests {
    use muda::MenuId;

    use super::{TRAY_ICON_BYTES, TrayAction, action_for, decode_icon};

    #[test]
    fn decode_icon_reads_the_embedded_asset_as_44x44_rgba() {
        let decoded = decode_icon(TRAY_ICON_BYTES).unwrap();

        assert_eq!(decoded.width, 44);
        assert_eq!(decoded.height, 44);
        assert_eq!(decoded.rgba.len(), 44 * 44 * 4);
    }

    #[test]
    fn action_for_maps_the_show_id_to_show() {
        let show_id = MenuId::new("gemelli-tray-show");
        let quit_id = MenuId::new("gemelli-tray-quit");

        assert_eq!(action_for(&show_id, &show_id, &quit_id), Some(TrayAction::Show));
    }

    #[test]
    fn action_for_maps_the_quit_id_to_quit() {
        let show_id = MenuId::new("gemelli-tray-show");
        let quit_id = MenuId::new("gemelli-tray-quit");

        assert_eq!(action_for(&quit_id, &show_id, &quit_id), Some(TrayAction::Quit));
    }

    #[test]
    fn action_for_ignores_ids_it_does_not_recognize() {
        let show_id = MenuId::new("gemelli-tray-show");
        let quit_id = MenuId::new("gemelli-tray-quit");
        let other_id = MenuId::new("some-other-item");

        assert_eq!(action_for(&other_id, &show_id, &quit_id), None);
    }
}
