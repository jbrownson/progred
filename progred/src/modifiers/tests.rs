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

#[test]
fn website_command_modifier_is_a_host_input_for_editing_and_source_picking() {
    use crate::display::test_support::primary_press;
    use crate::libraries::text;
    use ui_events::keyboard::{Key, KeyState, KeyboardEvent};
    for (command, modifiers, other) in [
        (CommandModifier::Meta, Modifiers::META, Modifiers::CONTROL),
        (
            CommandModifier::Control,
            Modifiers::CONTROL,
            Modifiers::META,
        ),
    ] {
        let mut editor = crate::editor_of(text::value("hello"));
        editor.command_modifier = command;
        let mut driver = crate::driver::Driver::new(editor, crate::driver::Driver::VIEWPORT);
        // Select all of the text with the page's command key, and type over it.
        driver.select::<gid::Step>(&[]);
        assert!(driver.keystroke(Key::Character("a".into()), modifiers));
        driver.type_text("changed");
        assert_eq!(text::read(&driver.value(&[])), Some("changed"));
        for (pressed, expected) in [(modifiers, true), (other, false)] {
            let event = primary_press(kurbo::Point::ZERO, pressed);
            assert_eq!(command.pressed(&event.state.modifiers), expected);
            assert_eq!(primary_edit(command, &event), !expected);
            let undo = KeyboardEvent {
                key: Key::Character("z".into()),
                modifiers: pressed,
                state: KeyState::Down,
                ..Default::default()
            };
            assert_eq!(
                crate::menu::shortcut(&undo, command),
                expected.then_some(crate::command::Command::Doc(
                    crate::command::DocCommand::Undo
                ))
            );
        }
    }
}
