use ui_events::keyboard::Modifiers;

/// Supplied by the host, including when its compilation target is WebAssembly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandModifier {
    Control,
    Meta,
}

impl CommandModifier {
    pub fn pressed(self, modifiers: &Modifiers) -> bool {
        match self {
            Self::Control => modifiers.ctrl(),
            Self::Meta => modifiers.meta(),
        }
    }
}
