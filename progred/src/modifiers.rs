use ui_events::keyboard::Modifiers;

/// The platform's primary application-command modifier.
pub(crate) fn command(modifiers: &Modifiers) -> bool {
    if cfg!(target_os = "macos") {
        modifiers.meta()
    } else {
        modifiers.ctrl()
    }
}

/// Nonlocal graph relationships become visible while this pointer
/// modifier is held.
pub(crate) fn link(modifiers: &Modifiers) -> bool {
    command(modifiers)
}

/// Picking is the activating gesture of the same link mode.
pub(crate) fn pick(modifiers: &Modifiers) -> bool {
    link(modifiers)
}

/// Direct manipulation of a projected value uses the same nonlocal
/// inspection mode as picking it.
pub(crate) fn scrub(modifiers: &Modifiers) -> bool {
    link(modifiers)
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

        assert!(link(&platform));
        assert!(pick(&platform));
        assert!(scrub(&platform));
        assert!(!link(&Modifiers::empty()));
        assert!(!pick(&Modifiers::empty()));
        assert!(!scrub(&Modifiers::empty()));
    }
}
