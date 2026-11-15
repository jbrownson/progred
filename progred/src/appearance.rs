//! Light or dark: the appearance chosen for every window, or else the
//! system's. On macOS each window reports the system's appearance; on Linux
//! the app doesn't see it yet, so following it means light.

use crate::display::widget::style;
use winit::window::{Theme, Window};

/// What decides the theme windows draw in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Appearance {
    /// The menus' choice; `None` follows the system.
    pub(crate) chosen: Option<Theme>,
}

impl Appearance {
    /// Gives `window` this appearance, for what the platform draws around
    /// its content, and returns the palette its editor draws in.
    pub(crate) fn apply(self, window: &Window) -> style::Palette {
        // A window without a theme of its own follows the system's, which
        // AppKit then reports.
        #[cfg(target_os = "macos")]
        let theme = {
            window.set_theme(self.chosen);
            resolve(self.chosen, window.theme())
        };
        // Winit can't see the desktop's preference, so the window is told
        // the theme in full: Wayland's drawn decorations take it, and X11
        // passes it to the window manager as a hint.
        #[cfg(target_os = "linux")]
        let theme = {
            let theme = resolve(self.chosen, None);
            window.set_theme(Some(theme));
            theme
        };
        palette(theme)
    }
}

/// `chosen`, or where that's `None` the system's theme, or light when the
/// system states none.
pub(crate) fn resolve(chosen: Option<Theme>, system: Option<Theme>) -> Theme {
    chosen.or(system).unwrap_or(Theme::Light)
}

pub(crate) fn palette(theme: Theme) -> style::Palette {
    match theme {
        Theme::Light => style::Theme::Light,
        Theme::Dark => style::Theme::Dark,
    }
    .palette()
}

#[cfg(test)]
#[path = "tests/appearance.rs"]
mod tests;
