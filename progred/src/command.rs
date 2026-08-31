//! What any frontend — the native menu, the drawn menu, keyboard
//! shortcuts — asks the app to do. Application commands are meaningful
//! with no window at all; document commands act on one editor.

/// Meaningful without any window. On the desktop these create or
/// drain windows; the single-canvas shells replace in place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppCommand {
    New,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    Open,
    Quit,
    Example(Example),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Example {
    Sample,
    Grap,
    IopTree,
    Fidget,
}

impl Example {
    pub fn source(self) -> &'static str {
        match self {
            Self::Sample => include_str!("../../examples/sample.gid"),
            Self::Grap => include_str!("../../examples/grap-demo.gid"),
            Self::IopTree => include_str!("../../examples/iop-tree.gid"),
            Self::Fidget => include_str!("../../examples/fidget.gid"),
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
    DebugGeometry,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    App(AppCommand),
    Doc(DocCommand),
}

/// The logical shortcut for a command; each menu system applies its
/// own modifier convention (Ctrl drawn, Command native).
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortcutKey {
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    D,
    N,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    O,
    P,
    Q,
    R,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    S,
    Z,
}

impl ShortcutKey {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Digit1 => "1",
            Self::Digit2 => "2",
            Self::Digit3 => "3",
            Self::Digit4 => "4",
            Self::D => "D",
            Self::N => "N",
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            Self::O => "O",
            Self::P => "P",
            Self::Q => "Q",
            Self::R => "R",
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            Self::S => "S",
            Self::Z => "Z",
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
        Command::App(AppCommand::New) => item("New", Some(Shortcut::plain(ShortcutKey::N))),
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        Command::App(AppCommand::Open) => item("Open…", Some(Shortcut::plain(ShortcutKey::O))),
        Command::App(AppCommand::Quit) => item("Quit", Some(Shortcut::plain(ShortcutKey::Q))),
        Command::App(AppCommand::Example(Example::Sample)) => {
            item("Sample", Some(Shortcut::plain(ShortcutKey::Digit1)))
        }
        Command::App(AppCommand::Example(Example::Grap)) => {
            item("Grap Demo", Some(Shortcut::plain(ShortcutKey::Digit2)))
        }
        Command::App(AppCommand::Example(Example::IopTree)) => item(
            "Inventing on Principle Tree",
            Some(Shortcut::plain(ShortcutKey::Digit3)),
        ),
        Command::App(AppCommand::Example(Example::Fidget)) => {
            item("Fidget", Some(Shortcut::plain(ShortcutKey::Digit4)))
        }
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        Command::Doc(DocCommand::Save) => item("Save", Some(Shortcut::plain(ShortcutKey::S))),
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        Command::Doc(DocCommand::SaveAs) => {
            item("Save As…", Some(Shortcut::shifted(ShortcutKey::S)))
        }
        Command::Doc(DocCommand::Undo) => item("Undo", Some(Shortcut::plain(ShortcutKey::Z))),
        Command::Doc(DocCommand::Redo) => item("Redo", Some(Shortcut::shifted(ShortcutKey::Z))),
        Command::Doc(DocCommand::OpenPaneLeft) => {
            item("Open Cell on Left", Some(Shortcut::plain(ShortcutKey::P)))
        }
        Command::Doc(DocCommand::OpenPaneRight) => item(
            "Open Cell on Right",
            Some(Shortcut::shifted(ShortcutKey::P)),
        ),
        Command::Doc(DocCommand::MovePaneUp) => item("Move Pane Up", None),
        Command::Doc(DocCommand::MovePaneDown) => item("Move Pane Down", None),
        Command::Doc(DocCommand::MovePaneLeft) => item("Move Pane Left", None),
        Command::Doc(DocCommand::MovePaneRight) => item("Move Pane Right", None),
        Command::Doc(DocCommand::Raw) => toggle("Raw", Some(Shortcut::plain(ShortcutKey::R))),
        Command::Doc(DocCommand::DebugGeometry) => {
            toggle("Debug Geometry", Some(Shortcut::plain(ShortcutKey::D)))
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
}

impl Availability {
    /// Nothing document-scoped can act — the windowless menu bar.
    #[cfg(target_os = "macos")]
    pub fn disabled() -> Self {
        Self {
            save: false,
            undo: false,
            redo: false,
            open_pane: false,
            move_up: false,
            move_down: false,
            move_left: false,
            move_right: false,
        }
    }

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
            DocCommand::Raw | DocCommand::DebugGeometry => true,
        }
    }

    pub fn enabled(self, command: Command) -> bool {
        match command {
            Command::App(_) => true,
            Command::Doc(command) => self.doc_enabled(command),
        }
    }
}
