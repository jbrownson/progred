//! What any frontend — the native menu, the drawn menu, keyboard
//! shortcuts — asks the app to do. Application commands are meaningful
//! with no window at all; document commands act on one editor.

/// Match only the commands contributed by this component.
pub fn shortcut(
    event: &ui_events::keyboard::KeyboardEvent,
    modifier: puri::keyboard::CommandModifier,
    commands: impl IntoIterator<Item = Command>,
) -> Option<Command> {
    let modifiers = &event.modifiers;
    if !event.state.is_down() || !modifier.pressed(modifiers) || modifiers.alt() {
        return None;
    }
    let ui_events::keyboard::Key::Character(key) = &event.key else {
        return None;
    };
    commands.into_iter().find(|command| {
        spec(*command)
            .shortcut
            .is_some_and(|shortcut| shortcut.matches(key, modifiers.shift()))
    })
}

/// History belongs to editing, independently of whether a menu is installed.
pub(crate) fn history_handler(
    scale: f64,
) -> puri::handler::Handler<crate::Editor, crate::placed::DispatchContext<crate::Editor>> {
    use puri::handler::{Event, EventOutcome, Handler};
    Handler::from_function(
        move |editor: &mut crate::Editor,
              event,
              input: &mut crate::placed::DispatchContext<crate::Editor>| {
            let Event::Key(key) = &event else {
                return EventOutcome::decline(event);
            };
            let Some(Command::Doc(command)) = shortcut(
                key,
                editor.command_modifier,
                [
                    Command::Doc(DocCommand::Undo),
                    Command::Doc(DocCommand::Redo),
                ],
            )
            .filter(|command| editor.menu_availability().enabled(*command)) else {
                return EventOutcome::decline(event);
            };
            editor.run_doc_command(command, input.geometry(scale));
            EventOutcome::accept()
        },
    )
}

/// Meaningful without any window. New and examples replace the current
/// document; desktop Open and NewWindow create windows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppCommand {
    New,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    NewWindow,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    Open,
    /// Close the focused window.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    Close,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    Quit,
    #[cfg(target_os = "macos")]
    Appearance(Option<winit::window::Theme>),
    Example(Example),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Example {
    IopTree,
    Fidget,
    Toolpaths,
    Grap,
    Libraries,
}

impl Example {
    pub const ALL: [Self; 5] = [
        Self::IopTree,
        Self::Fidget,
        Self::Toolpaths,
        Self::Grap,
        Self::Libraries,
    ];

    /// The bundled document's file name, which also names it in a web address.
    #[cfg(any(test, target_arch = "wasm32"))]
    pub fn file_stem(self) -> &'static str {
        match self {
            Self::Grap => "grap-demo",
            Self::IopTree => "iop-tree",
            Self::Fidget => "fidget-shapes",
            Self::Toolpaths => "toolpaths",
            Self::Libraries => "libraries",
        }
    }

    pub fn source(self) -> &'static str {
        match self {
            Self::Grap => include_str!("../../examples/grap-demo.gid"),
            Self::IopTree => include_str!("../../examples/iop-tree.gid"),
            Self::Fidget => include_str!("../../examples/fidget-shapes.gid"),
            Self::Toolpaths => include_str!("../../examples/toolpaths.gid"),
            Self::Libraries => include_str!("../../examples/libraries.gid"),
        }
    }
}

/// Acts on one editor: the focused window's for the native menu, the
/// menu's own window for the drawn one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DocCommand {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    Save,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    SaveAs,
    Undo,
    Redo,
    OpenPaneLeft,
    OpenPaneRight,
    MovePaneUp,
    MovePaneDown,
    MovePaneLeft,
    MovePaneRight,
    Raw,
    /// One library's projection in the selected area. Menus label it with
    /// the library's name.
    Projection(gid::CellId),
    DebugGeometry,
    // On the web, the browser's page zoom does this.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    ActualSize,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    ZoomIn,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    ZoomOut,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    App(AppCommand),
    Doc(DocCommand),
}

/// The logical shortcut for a command; each menu system applies its
/// host's modifier convention (Command on Mac, Ctrl elsewhere).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shortcut {
    pub key: ShortcutKey,
    pub shift: bool,
}

impl Shortcut {
    const fn plain(key: ShortcutKey) -> Self {
        Self { key, shift: false }
    }

    const fn shifted(key: ShortcutKey) -> Self {
        Self { key, shift: true }
    }

    fn matches(self, key: &str, shift: bool) -> bool {
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        if self.key == ShortcutKey::Plus {
            // + is Shift-= on most layouts, and browsers take = alone too.
            return key == "+" || key == "=";
        }
        self.shift == shift && key.eq_ignore_ascii_case(self.key.label())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortcutKey {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    D,
    N,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    O,
    P,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    Q,
    R,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    S,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    W,
    Z,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    Plus,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    Minus,
}

impl ShortcutKey {
    pub const fn label(self) -> &'static str {
        match self {
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            Self::Digit0 => "0",
            Self::Digit1 => "1",
            Self::Digit2 => "2",
            Self::Digit3 => "3",
            Self::Digit4 => "4",
            Self::Digit5 => "5",
            Self::D => "D",
            Self::N => "N",
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            Self::O => "O",
            Self::P => "P",
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            Self::Q => "Q",
            Self::R => "R",
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            Self::S => "S",
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            Self::W => "W",
            Self::Z => "Z",
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            Self::Plus => "+",
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            Self::Minus => "-",
        }
    }
}

/// The command's presentation facts, shared by every menu system:
/// what to call it, its logical key, whether it is a toggle. Menu
/// structure stays each system's own.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spec {
    pub label: &'static str,
    pub shortcut: Option<Shortcut>,
    pub toggle: bool,
}

pub fn spec(command: Command) -> Spec {
    let item = |label, shortcut| Spec {
        label,
        shortcut,
        toggle: false,
    };
    let toggle = |label, shortcut| Spec {
        label,
        shortcut,
        toggle: true,
    };
    match command {
        Command::App(AppCommand::New) => {
            item("New Document", Some(Shortcut::plain(ShortcutKey::N)))
        }
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        Command::App(AppCommand::NewWindow) => {
            item("New Window", Some(Shortcut::shifted(ShortcutKey::N)))
        }
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        Command::App(AppCommand::Open) => item("Open…", Some(Shortcut::plain(ShortcutKey::O))),
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        Command::App(AppCommand::Close) => item("Close", Some(Shortcut::plain(ShortcutKey::W))),
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        Command::App(AppCommand::Quit) => item("Quit", Some(Shortcut::plain(ShortcutKey::Q))),
        #[cfg(target_os = "macos")]
        Command::App(AppCommand::Appearance(theme)) => toggle(
            match theme {
                None => "Follow System Appearance",
                Some(winit::window::Theme::Light) => "Light Mode",
                Some(winit::window::Theme::Dark) => "Dark Mode",
            },
            None,
        ),
        Command::App(AppCommand::Example(Example::IopTree)) => item(
            "Inventing on Principle Tree",
            Some(Shortcut::plain(ShortcutKey::Digit1)),
        ),
        Command::App(AppCommand::Example(Example::Fidget)) => item(
            "Implicit CAD Shapes",
            Some(Shortcut::plain(ShortcutKey::Digit2)),
        ),
        Command::App(AppCommand::Example(Example::Toolpaths)) => {
            item("CAM Toolpaths", Some(Shortcut::plain(ShortcutKey::Digit3)))
        }
        Command::App(AppCommand::Example(Example::Grap)) => {
            item("Grap Language", Some(Shortcut::plain(ShortcutKey::Digit4)))
        }
        Command::App(AppCommand::Example(Example::Libraries)) => item(
            "Document Libraries",
            Some(Shortcut::plain(ShortcutKey::Digit5)),
        ),
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        Command::Doc(DocCommand::Save) => item("Save", Some(Shortcut::plain(ShortcutKey::S))),
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        Command::Doc(DocCommand::SaveAs) => {
            item("Save As…", Some(Shortcut::shifted(ShortcutKey::S)))
        }
        Command::Doc(DocCommand::Undo) => item("Undo", Some(Shortcut::plain(ShortcutKey::Z))),
        Command::Doc(DocCommand::Redo) => item("Redo", Some(Shortcut::shifted(ShortcutKey::Z))),
        Command::Doc(DocCommand::OpenPaneLeft) => {
            item("Open Value on Left", Some(Shortcut::plain(ShortcutKey::P)))
        }
        Command::Doc(DocCommand::OpenPaneRight) => item(
            "Open Value on Right",
            Some(Shortcut::shifted(ShortcutKey::P)),
        ),
        Command::Doc(DocCommand::MovePaneUp) => item("Move Pane Up", None),
        Command::Doc(DocCommand::MovePaneDown) => item("Move Pane Down", None),
        Command::Doc(DocCommand::MovePaneLeft) => item("Move Pane Left", None),
        Command::Doc(DocCommand::MovePaneRight) => item("Move Pane Right", None),
        Command::Doc(DocCommand::Raw) => toggle("Raw", Some(Shortcut::plain(ShortcutKey::R))),
        Command::Doc(DocCommand::Projection(_)) => toggle("Projection", None),
        Command::Doc(DocCommand::DebugGeometry) => {
            toggle("Debug Geometry", Some(Shortcut::plain(ShortcutKey::D)))
        }
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        Command::Doc(DocCommand::ActualSize) => {
            item("Actual Size", Some(Shortcut::plain(ShortcutKey::Digit0)))
        }
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        Command::Doc(DocCommand::ZoomIn) => {
            item("Zoom In", Some(Shortcut::plain(ShortcutKey::Plus)))
        }
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        Command::Doc(DocCommand::ZoomOut) => {
            item("Zoom Out", Some(Shortcut::plain(ShortcutKey::Minus)))
        }
    }
}

/// What the target editor can act on. Application commands are always
/// live.
#[derive(Clone, Copy)]
pub struct Availability {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub save: bool,
    pub undo: bool,
    pub redo: bool,
    pub open_pane: bool,
    pub move_up: bool,
    pub move_down: bool,
    pub move_left: bool,
    pub move_right: bool,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub actual_size: bool,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub zoom_in: bool,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub zoom_out: bool,
}

impl Availability {
    pub fn doc_enabled(self, command: DocCommand) -> bool {
        match command {
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            DocCommand::Save => self.save,
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            DocCommand::SaveAs => true,
            DocCommand::Undo => self.undo,
            DocCommand::Redo => self.redo,
            DocCommand::OpenPaneLeft | DocCommand::OpenPaneRight => self.open_pane,
            DocCommand::MovePaneUp => self.move_up,
            DocCommand::MovePaneDown => self.move_down,
            DocCommand::MovePaneLeft => self.move_left,
            DocCommand::MovePaneRight => self.move_right,
            DocCommand::Raw | DocCommand::Projection(_) | DocCommand::DebugGeometry => true,
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            DocCommand::ActualSize => self.actual_size,
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            DocCommand::ZoomIn => self.zoom_in,
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            DocCommand::ZoomOut => self.zoom_out,
        }
    }

    pub fn enabled(self, command: Command) -> bool {
        match command {
            Command::App(_) => true,
            Command::Doc(command) => self.doc_enabled(command),
        }
    }
}

/// Which toggle commands are on for the target editor. Frontends only
/// display this answer.
#[derive(Clone, Default)]
pub struct Toggles {
    pub raw: bool,
    pub debug_geometry: bool,
    /// Libraries whose projections the selected area leaves off.
    pub hidden: Vec<gid::CellId>,
}

impl Toggles {
    pub fn checked(&self, command: Command) -> bool {
        match command {
            Command::Doc(DocCommand::Raw) => self.raw,
            Command::Doc(DocCommand::Projection(library)) => !self.hidden.contains(&library),
            Command::Doc(DocCommand::DebugGeometry) => self.debug_geometry,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_file_stems_name_their_bundled_documents() {
        for example in Example::ALL {
            let path = format!("../examples/{}.gid", example.file_stem());
            assert_eq!(
                std::fs::read_to_string(&path).unwrap(),
                example.source(),
                "{path}"
            );
        }
    }
}
