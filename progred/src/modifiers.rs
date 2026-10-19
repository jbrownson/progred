use puri::keyboard::CommandModifier;
use ui_events::keyboard::{Key, KeyboardEvent, Modifiers};
use ui_events::pointer::PointerButtonEvent;

/// The platform's primary application-command modifier. A browser page
/// says which its platform uses; only the orbit profile asks there.
#[cfg(any(not(target_arch = "wasm32"), feature = "cam-profile"))]
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

/// Whether `event` presses `key` with the `command` modifier held.
pub(crate) fn chord(command: CommandModifier, event: &KeyboardEvent, key: &str) -> bool {
    event.state.is_down()
        && command.pressed(&event.modifiers)
        && matches!(&event.key, Key::Character(typed) if typed.eq_ignore_ascii_case(key))
}

pub(crate) fn plain(modifiers: &Modifiers) -> bool {
    !(modifiers.ctrl() || modifiers.meta() || modifiers.alt() || modifiers.shift())
}

#[cfg(test)]
mod tests;
