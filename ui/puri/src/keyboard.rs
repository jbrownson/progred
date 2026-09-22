use ui_events::keyboard::Modifiers;

/// Supplied by the host, including when its compilation target is WebAssembly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandModifier {
    Control,
    Meta,
}

impl CommandModifier {
    pub fn pressed(self, modifiers: &Modifiers) -> bool {
        self.predicate()(modifiers)
    }

    pub fn predicate(self) -> fn(&Modifiers) -> bool {
        match self {
            Self::Control => Modifiers::ctrl,
            Self::Meta => Modifiers::meta,
        }
    }
}
