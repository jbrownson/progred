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
