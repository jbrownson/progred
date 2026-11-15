//! Light or dark: the appearance chosen for every window, or else the
//! system's. On macOS each window reports the system's appearance; on Linux
//! the desktop's XDG portal reports it to the app.

use crate::display::widget::style;
use winit::window::{Theme, Window};

/// What decides the theme windows draw in.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Appearance {
    /// The menus' choice; `None` follows the system.
    pub(crate) chosen: Option<Theme>,
    /// The desktop's preference, as its portal last reported it: `None`
    /// until it does, and when it states none or there's no portal.
    #[cfg(target_os = "linux")]
    pub(crate) system: Option<Theme>,
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
            let theme = resolve(self.chosen, self.system);
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

/// The theme the portal's `color-scheme` setting prefers: 1 is dark, 2
/// light, and 0, or a value from a later version, states no preference.
#[cfg(target_os = "linux")]
pub(crate) fn color_scheme(value: u32) -> Option<Theme> {
    match value {
        1 => Some(Theme::Dark),
        2 => Some(Theme::Light),
        _ => None,
    }
}

#[cfg(target_os = "linux")]
pub(crate) use portal::follow_system;

/// The XDG desktop portal's Settings interface, which GNOME's, KDE's and
/// GTK's portal backends implement on the session bus.
#[cfg(target_os = "linux")]
mod portal {
    use super::color_scheme;
    use crate::UserEvent;
    use futures_core::Stream;
    use winit::event_loop::EventLoopProxy;

    const NAMESPACE: &str = "org.freedesktop.appearance";
    const KEY: &str = "color-scheme";

    #[zbus::proxy(
        interface = "org.freedesktop.portal.Settings",
        default_service = "org.freedesktop.portal.Desktop",
        default_path = "/org/freedesktop/portal/desktop"
    )]
    trait Settings {
        fn read_one(&self, namespace: &str, key: &str) -> zbus::Result<zbus::OwnedVariant>;

        #[zbus(signal)]
        fn setting_changed(
            &self,
            namespace: &str,
            key: &str,
            value: zbus::Variant<'_>,
        ) -> zbus::Result<()>;
    }

    /// Reports the desktop's light or dark preference as
    /// [`UserEvent::SystemAppearance`], now and whenever it changes, from a
    /// thread of its own that waits on the session bus. Without a session
    /// bus, or a portal that has the setting, it reports nothing.
    pub(crate) fn follow_system(proxy: EventLoopProxy<UserEvent>) {
        let _ = std::thread::Builder::new()
            .name("system appearance".into())
            .spawn(move || zbus::block_on(report(&proxy)));
    }

    /// Ends when the bus or the event loop goes away.
    async fn report(proxy: &EventLoopProxy<UserEvent>) -> zbus::Result<()> {
        let connection = zbus::Connection::session().await?;
        let settings = SettingsProxy::new(&connection).await?;
        // Listening starts before the first read, so no change slips
        // between them.
        let mut changes = std::pin::pin!(
            settings
                .receive_setting_changed_with_args(&[(0, NAMESPACE), (1, KEY)])
                .await?
        );
        let mut value = Some(u32::try_from(settings.read_one(NAMESPACE, KEY).await?)?);
        while let Some(scheme) = value
            && proxy
                .send_event(UserEvent::SystemAppearance(color_scheme(scheme)))
                .is_ok()
        {
            value = std::future::poll_fn(|cx| changes.as_mut().poll_next(cx))
                .await
                .map(|change| change.args().and_then(|args| u32::try_from(args.value)))
                .transpose()?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests/appearance.rs"]
mod tests;
