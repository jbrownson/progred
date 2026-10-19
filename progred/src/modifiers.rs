use puri::keyboard::CommandModifier;
use ui_events::keyboard::Modifiers;
use ui_events::pointer::PointerButtonEvent;

/// The platform's primary application-command modifier.
pub(crate) fn native() -> CommandModifier {
    if cfg!(target_os = "macos") {
        CommandModifier::Meta
    } else {
        CommandModifier::Control
    }
}

/// Ordinary editing uses a primary contact outside source-picking mode.
pub(crate) fn primary_edit(command: CommandModifier, event: &PointerButtonEvent) -> bool {
    puri::interact::is_primary_contact(event) && !command.pressed(&event.state.modifiers)
}

/// The keys of [`native`]'s modifier, for tests that press it.
#[cfg(test)]
pub(crate) fn command() -> Modifiers {
    match native() {
        CommandModifier::Meta => Modifiers::META,
        CommandModifier::Control => Modifiers::CONTROL,
    }
}

pub(crate) fn plain(modifiers: &Modifiers) -> bool {
    !(modifiers.ctrl() || modifiers.meta() || modifiers.alt() || modifiers.shift())
}

#[cfg(test)]
mod tests;
