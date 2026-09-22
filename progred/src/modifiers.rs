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

pub(crate) fn plain(modifiers: &Modifiers) -> bool {
    !(modifiers.ctrl() || modifiers.meta() || modifiers.alt() || modifiers.shift())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linking_and_picking_share_the_platform_command_modifier() {
        let platform = if cfg!(target_os = "macos") {
            Modifiers::META
        } else {
            Modifiers::CONTROL
        };

        assert!(native().pressed(&platform));
        assert!(!native().pressed(&Modifiers::empty()));
    }
}
