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

/// Which pointer presses count, given the platform's command key.
pub(crate) type PressRule = fn(CommandModifier) -> fn(&PointerButtonEvent) -> bool;

/// Picking activates a nonlocal graph relationship while the command
/// modifier is held.
pub(crate) fn picking(command: CommandModifier) -> fn(&PointerButtonEvent) -> bool {
    match command {
        CommandModifier::Meta => |event| event.state.modifiers.meta(),
        CommandModifier::Control => |event| event.state.modifiers.ctrl(),
    }
}

/// Ordinary editing uses a primary contact outside source-picking mode.
pub(crate) fn primary_edit(command: CommandModifier) -> fn(&PointerButtonEvent) -> bool {
    match command {
        CommandModifier::Meta => {
            |event| puri::interact::is_primary_contact(event) && !event.state.modifiers.meta()
        }
        CommandModifier::Control => {
            |event| puri::interact::is_primary_contact(event) && !event.state.modifiers.ctrl()
        }
    }
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
