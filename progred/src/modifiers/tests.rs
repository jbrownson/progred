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
    use crate::libraries::test_widgets::press;
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
        let mut editor = crate::test_editor(gid::Document {
            root: Some(text::value("hello")),
            cells: gid::Cells::new(),
        });
        editor.command_modifier = command;
        let mut driver = crate::driver::Driver::new(editor, kurbo::Size::new(900.0, 600.0));
        // Select all of the text with the page's command key, and type over it.
        driver.select(&[]);
        assert!(driver.keystroke(Key::Character("a".into()), modifiers));
        driver.type_text("changed");
        assert_eq!(text::read(&driver.value(&[])), Some("changed"));
        for (pressed, expected) in [(modifiers, true), (other, false)] {
            let mut event = press(0.0, false);
            event.state.modifiers = pressed;
            assert_eq!(picking(command)(&event), expected);
            assert_eq!(primary_edit(command)(&event), !expected);
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
