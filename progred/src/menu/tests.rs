use super::*;
use ui_events::keyboard::{KeyState, Modifiers, NamedKey};

fn key(key: &str, modifiers: Modifiers) -> KeyboardEvent {
    KeyboardEvent {
        key: Key::Character(key.into()),
        modifiers,
        state: KeyState::Down,
        ..Default::default()
    }
}

#[test]
fn shortcuts_are_drawn_application_commands() {
    assert_eq!(
        shortcut(
            &key("n", Modifiers::CONTROL),
            puri::keyboard::CommandModifier::Control
        ),
        Some(Command::App(AppCommand::New))
    );
    assert_eq!(
        shortcut(
            &key("N", Modifiers::CONTROL | Modifiers::SHIFT),
            puri::keyboard::CommandModifier::Control
        ),
        Some(Command::App(AppCommand::NewWindow))
    );
    assert_eq!(
        shortcut(
            &key("s", Modifiers::CONTROL),
            puri::keyboard::CommandModifier::Control
        ),
        Some(Command::Doc(DocCommand::Save))
    );
    assert_eq!(
        shortcut(
            &key("S", Modifiers::CONTROL | Modifiers::SHIFT),
            puri::keyboard::CommandModifier::Control
        ),
        Some(Command::Doc(DocCommand::SaveAs))
    );
    assert_eq!(
        shortcut(
            &key("s", Modifiers::META),
            puri::keyboard::CommandModifier::Control
        ),
        None
    );
    assert_eq!(
        shortcut(
            &key("s", Modifiers::CONTROL | Modifiers::ALT),
            puri::keyboard::CommandModifier::Control
        ),
        None
    );
}

#[test]
fn example_shortcuts_follow_menu_order() {
    for (digit, example) in [
        ("1", Example::IopTree),
        ("2", Example::Fidget),
        ("3", Example::Toolpaths),
        ("4", Example::Grap),
        ("5", Example::Libraries),
    ] {
        assert_eq!(
            shortcut(
                &key(digit, Modifiers::CONTROL),
                puri::keyboard::CommandModifier::Control
            ),
            Some(Command::App(AppCommand::Example(example)))
        );
    }
}

#[test]
fn removed_example_shortcuts_are_unclaimed() {
    for digit in ["6", "7", "8", "9"] {
        assert_eq!(
            shortcut(
                &key(digit, Modifiers::CONTROL),
                puri::keyboard::CommandModifier::Control
            ),
            None
        );
    }
}

fn named(key: NamedKey) -> KeyboardEvent {
    KeyboardEvent {
        key: Key::Named(key),
        state: KeyState::Down,
        ..Default::default()
    }
}

fn all_enabled() -> crate::command::Availability {
    crate::command::Availability {
        save: true,
        undo: true,
        redo: true,
        open_pane: true,
        move_up: true,
        move_down: true,
        move_left: true,
        move_right: true,
        actual_size: true,
        zoom_in: true,
        zoom_out: true,
    }
}

#[test]
fn arrows_walk_enabled_items_and_wrap() {
    let menus = definition([]);
    let mut availability = all_enabled();
    availability.undo = false;
    // Edit is [Undo, Redo]; with Undo disabled the cursor lands on
    // Redo from either direction and wraps in place.
    let mut state = State::default();
    let edit = menus
        .iter()
        .position(|menu| menu.label == "Edit")
        .expect("edit menu");
    state.toggle(edit);
    assert!(matches!(
        navigate(
            &mut state,
            &menus,
            availability,
            &named(NamedKey::ArrowDown)
        ),
        Navigation::Handled
    ));
    assert_eq!(state.cursor(), Some(1));
    assert!(matches!(
        navigate(
            &mut state,
            &menus,
            availability,
            &named(NamedKey::ArrowDown)
        ),
        Navigation::Handled
    ));
    assert_eq!(state.cursor(), Some(1));
    assert!(matches!(
        navigate(&mut state, &menus, availability, &named(NamedKey::Enter)),
        Navigation::Activate(Command::Doc(DocCommand::Redo))
    ));
}

#[test]
fn horizontal_arrows_switch_menus_and_select_the_first_enabled_item() {
    let menus = definition([]);
    let mut state = State::default();
    state.toggle(0);
    assert!(matches!(
        navigate(
            &mut state,
            &menus,
            all_enabled(),
            &named(NamedKey::ArrowDown)
        ),
        Navigation::Handled
    ));
    assert!(state.cursor().is_some());
    assert!(matches!(
        navigate(
            &mut state,
            &menus,
            all_enabled(),
            &named(NamedKey::ArrowRight)
        ),
        Navigation::Handled
    ));
    assert_eq!(state.open(), Some(1));
    assert_eq!(state.cursor(), Some(0));
    assert!(matches!(
        navigate(
            &mut state,
            &menus,
            all_enabled(),
            &named(NamedKey::ArrowLeft)
        ),
        Navigation::Handled
    ));
    assert_eq!(state.open(), Some(0));
    assert_eq!(state.cursor(), Some(0));
}

#[test]
fn keyboard_entry_endpoints_and_dismissal() {
    let menus = definition([]);
    let mut state = State::default();
    navigate(&mut state, &menus, all_enabled(), &named(NamedKey::F10));
    assert_eq!(state.open(), Some(0));
    assert_eq!(state.cursor(), Some(0));
    navigate(&mut state, &menus, all_enabled(), &named(NamedKey::End));
    assert_eq!(state.cursor(), Some(commands(&menus[..1]).count() - 1));
    navigate(&mut state, &menus, all_enabled(), &named(NamedKey::Home));
    assert_eq!(state.cursor(), Some(0));
    for key in [NamedKey::Escape, NamedKey::Tab, NamedKey::F10] {
        navigate(&mut state, &menus, all_enabled(), &named(key));
        assert_eq!(state.open(), None);
        assert_eq!(state.cursor(), None);
        navigate(&mut state, &menus, all_enabled(), &named(NamedKey::F10));
    }
}

#[test]
fn a_tap_of_alt_on_its_own_enters_and_leaves_the_bar() {
    let menus = definition([]);
    let mut state = State::default();
    let alt = |state: &mut State, key: NamedKey, down: bool, modifiers: Modifiers| {
        let event = KeyboardEvent {
            key: Key::Named(key),
            state: if down { KeyState::Down } else { KeyState::Up },
            modifiers,
            ..Default::default()
        };
        matches!(
            navigate(state, &menus, all_enabled(), &event),
            Navigation::Handled
        )
    };
    // Its repeats while held are still the one press.
    assert!(!alt(&mut state, NamedKey::Alt, true, Modifiers::ALT));
    let mut repeat = named(NamedKey::Alt);
    repeat.modifiers = Modifiers::ALT;
    repeat.repeat = true;
    navigate(&mut state, &menus, all_enabled(), &repeat);
    assert_eq!(state.open(), None, "pressing it opens nothing yet");
    assert!(alt(&mut state, NamedKey::Alt, false, Modifiers::empty()));
    assert_eq!(state.open(), Some(0));
    assert_eq!(state.cursor(), Some(0));
    alt(&mut state, NamedKey::Alt, true, Modifiers::ALT);
    assert!(alt(&mut state, NamedKey::Alt, false, Modifiers::empty()));
    assert_eq!(state.open(), None, "a second tap leaves");

    // Chords aren't taps: another key between, Alt pressed with Shift (a
    // layout switch on many desktops) or Ctrl, AltGr, or a click.
    alt(&mut state, NamedKey::Alt, true, Modifiers::ALT);
    navigate(&mut state, &menus, all_enabled(), &key("x", Modifiers::ALT));
    assert!(!alt(&mut state, NamedKey::Alt, false, Modifiers::empty()));
    for held in [Modifiers::SHIFT, Modifiers::CONTROL] {
        alt(&mut state, NamedKey::Alt, true, held | Modifiers::ALT);
        assert!(!alt(&mut state, NamedKey::Alt, false, held));
    }
    alt(&mut state, NamedKey::AltGraph, true, Modifiers::ALT_GRAPH);
    assert!(!alt(
        &mut state,
        NamedKey::AltGraph,
        false,
        Modifiers::empty()
    ));
    alt(&mut state, NamedKey::Alt, true, Modifiers::ALT);
    state.interrupt();
    assert!(!alt(&mut state, NamedKey::Alt, false, Modifiers::empty()));
    // Nor is a release whose press went elsewhere.
    assert!(!alt(&mut state, NamedKey::Alt, false, Modifiers::empty()));
    assert_eq!(state.open(), None);
}

#[test]
fn menu_sections_toggle_and_switch() {
    let mut state = State::default();
    state.toggle(0);
    assert_eq!(state.open(), Some(0));
    state.toggle(1);
    assert_eq!(state.open(), Some(1));
    state.toggle(1);
    assert_eq!(state.open(), None);
    assert!(!state.close());
}

#[test]
fn an_open_menu_captures_other_key_downs() {
    let mut state = State::default();
    let mut event = key("x", Modifiers::empty());
    assert!(!state.captures_key(&event));
    state.toggle(0);
    assert!(state.captures_key(&event));
    event.state = KeyState::Up;
    assert!(!state.captures_key(&event));
}

#[test]
fn the_drawn_tree_lists_every_command_once() {
    let definition = definition([]);
    let commands = commands(&definition).collect::<Vec<_>>();
    assert_eq!(commands.len(), 20 + Example::ALL.len());
    for (index, command) in commands.iter().enumerate() {
        assert!(commands[index + 1..].iter().all(|other| command != other));
    }
}

#[test]
fn the_file_menu_carries_the_application_lifecycle() {
    let definition = definition([]);
    assert_eq!(
        definition.iter().map(|menu| menu.label).collect::<Vec<_>>(),
        vec!["File", "Examples", "Edit", "View"]
    );
    assert_eq!(
        definition[0].entries,
        vec![
            Entry::Command(Command::App(AppCommand::New)),
            Entry::Command(Command::App(AppCommand::NewWindow)),
            Entry::Command(Command::App(AppCommand::Open)),
            Entry::Separator,
            Entry::Command(Command::App(AppCommand::Close)),
            Entry::Command(Command::Doc(DocCommand::Save)),
            Entry::Command(Command::Doc(DocCommand::SaveAs)),
            Entry::Separator,
            Entry::Command(Command::App(AppCommand::Quit)),
        ]
    );
}

#[test]
fn catalog_shortcuts_are_unique_across_the_drawn_tree() {
    let definition = definition([]);
    let shortcuts = commands(&definition)
        .filter_map(|command| command::spec(command).shortcut)
        .collect::<Vec<_>>();
    for (index, shortcut) in shortcuts.iter().enumerate() {
        assert!(shortcuts[index + 1..].iter().all(|other| shortcut != other));
    }
}
