//! Platform conventions, resolved once into flags, so behavior
//! differences read as policy rather than scattered conditions.

/// Closing the last window quits the process. The macOS convention
/// instead keeps the app resident with only its menu bar, ready to
/// open the next window.
#[cfg_attr(any(target_arch = "wasm32", target_os = "ios"), allow(dead_code))]
pub(crate) const QUITS_ON_LAST_CLOSE: bool = cfg!(not(target_os = "macos"));
