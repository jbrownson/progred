use super::*;

fn commands(sections: &[Section]) -> Vec<Command> {
    sections
        .iter()
        .flat_map(|section| {
            section.entries.iter().filter_map(|entry| match entry {
                Entry::Command(command) | Entry::Labeled(command, _) => Some(*command),
                Entry::Separator | Entry::Native(_) => None,
            })
        })
        .collect()
}

#[test]
fn the_native_tree_lists_every_command_once() {
    let stack = crate::stack::load();
    let definition = definition(stack.switchable());
    assert_eq!(
        definition
            .iter()
            .map(|section| section.label)
            .collect::<Vec<_>>(),
        vec![
            "Progred",
            "File",
            "Examples",
            "Edit",
            "View",
            "Projections",
            "Window",
            "Help"
        ]
    );
    let commands = commands(&definition);
    assert_eq!(
        commands.len(),
        27 + Example::ALL.len() + stack.switchable().count()
    );
    for (index, command) in commands.iter().enumerate() {
        assert!(commands[index + 1..].iter().all(|other| command != other));
    }
}

#[test]
fn both_menu_systems_expose_the_same_commands_except_native_appearance() {
    let stack = crate::stack::load();
    let mut native = commands(&definition(stack.switchable()));
    native.retain(|command| !matches!(command, Command::App(AppCommand::Appearance(_))));
    let drawn_definition = crate::menu::definition(stack.switchable().map(|(library, _)| library));
    let mut drawn = crate::menu::commands(&drawn_definition).collect::<Vec<_>>();
    let key = |command: &Command| format!("{command:?}");
    native.sort_by_key(key);
    drawn.sort_by_key(key);
    assert_eq!(native, drawn);
}

#[test]
fn appearance_choices_are_exclusive_even_without_a_document() {
    let choices = [
        None,
        Some(winit::window::Theme::Light),
        Some(winit::window::Theme::Dark),
    ];
    for selected in choices {
        for doc in [None, Some(Toggles::default())] {
            for choice in choices {
                let command = Command::App(AppCommand::Appearance(choice));
                assert!(command::spec(command).toggle);
                assert_eq!(checked(command, doc.as_ref(), selected), choice == selected);
            }
        }
    }
}
