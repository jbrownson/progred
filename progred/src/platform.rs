//! Platform conventions, each resolved in one place, so behavior
//! differences read as policy rather than scattered conditions.

/// Closing the last window quits the process. The macOS convention
/// instead keeps the app resident with only its menu bar, ready to
/// open the next window.
pub(crate) const QUITS_ON_LAST_CLOSE: bool = cfg!(not(target_os = "macos"));

/// Whether windows draw the in-window menu system: wherever there is no
/// native application menu. Setting `PROGRED_DRAWN_MENU` turns it on
/// anywhere, for developing the drawn system on macOS.
pub(crate) fn drawn_menu() -> bool {
    cfg!(not(target_os = "macos")) || std::env::var_os("PROGRED_DRAWN_MENU").is_some()
}
