//! The native menu system: one application-wide menu bar built with
//! muda, emitting [`command::Command`]s routed to the focused window.
//! Fully separate from the drawn in-window menu system. Only enabled
//! on macOS today, but muda itself also speaks Windows and GTK.

use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{
    CheckMenuItem, IsMenuItem, Menu as MudaMenu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem,
    Submenu,
};
use winit::event_loop::EventLoopProxy;

use crate::UserEvent;
use crate::command::{self, AppCommand, Availability, Command, DocCommand, Example};
use crate::model::ViewFlags;

pub struct Event(MenuEvent);

/// The native bar's structure: which commands, where, in what order.
/// Labels, keys, and toggle-ness come from the shared
/// [`command::spec`] catalog; entries may override the label for
/// platform idiom ("Quit Progred" in the application menu).
enum Entry {
    Command(Command),
    Labeled(Command, &'static str),
    Separator,
    About,
}

struct Section {
    label: &'static str,
    entries: Vec<Entry>,
}

fn definition() -> Vec<Section> {
    use {AppCommand as A, Command as C, DocCommand as D, Example as E};
    vec![
        Section {
            label: "Progred",
            entries: vec![
                Entry::About,
                Entry::Separator,
                Entry::Labeled(C::App(A::Quit), "Quit Progred"),
            ],
        },
        Section {
            label: "File",
            entries: vec![
                Entry::Command(C::App(A::New)),
                Entry::Command(C::App(A::Open)),
                Entry::Separator,
                Entry::Command(C::Doc(D::Save)),
                Entry::Command(C::Doc(D::SaveAs)),
            ],
        },
        Section {
            label: "Examples",
            entries: vec![
                Entry::Command(C::App(A::Example(E::Sample))),
                Entry::Command(C::App(A::Example(E::Grap))),
                Entry::Command(C::App(A::Example(E::IopTree))),
                Entry::Command(C::App(A::Example(E::Fidget))),
            ],
        },
        Section {
            label: "Edit",
            entries: vec![
                Entry::Command(C::Doc(D::Undo)),
                Entry::Command(C::Doc(D::Redo)),
            ],
        },
        Section {
            label: "View",
            entries: vec![
                Entry::Command(C::Doc(D::OpenPaneLeft)),
                Entry::Command(C::Doc(D::OpenPaneRight)),
                Entry::Separator,
                Entry::Command(C::Doc(D::MovePaneUp)),
                Entry::Command(C::Doc(D::MovePaneDown)),
                Entry::Command(C::Doc(D::MovePaneLeft)),
                Entry::Command(C::Doc(D::MovePaneRight)),
                Entry::Separator,
                Entry::Command(C::Doc(D::Raw)),
                Entry::Command(C::Doc(D::DebugGeometry)),
            ],
        },
    ]
}

/// The native modifier convention is Command.
fn accelerator(shortcut: command::Shortcut) -> Accelerator {
    use command::ShortcutKey;
    Accelerator::new(
        Some(if shortcut.shift {
            Modifiers::META | Modifiers::SHIFT
        } else {
            Modifiers::META
        }),
        match shortcut.key {
            ShortcutKey::Digit1 => Code::Digit1,
            ShortcutKey::Digit2 => Code::Digit2,
            ShortcutKey::Digit3 => Code::Digit3,
            ShortcutKey::Digit4 => Code::Digit4,
            ShortcutKey::D => Code::KeyD,
            ShortcutKey::N => Code::KeyN,
            ShortcutKey::O => Code::KeyO,
            ShortcutKey::P => Code::KeyP,
            ShortcutKey::Q => Code::KeyQ,
            ShortcutKey::R => Code::KeyR,
            ShortcutKey::S => Code::KeyS,
            ShortcutKey::Z => Code::KeyZ,
        },
    )
}

pub struct Menu {
    root: MudaMenu,
    items: Vec<(Command, NativeItem)>,
}

enum NativeItem {
    Command(MenuItem),
    Check(CheckMenuItem),
}

impl NativeItem {
    fn new(command: Command, label: Option<&'static str>) -> Self {
        let spec = command::spec(command);
        let label = label.unwrap_or(spec.label);
        let accelerator = spec.shortcut.map(accelerator);
        if spec.toggle {
            Self::Check(CheckMenuItem::new(label, true, false, accelerator))
        } else {
            Self::Command(MenuItem::new(label, true, accelerator))
        }
    }

    fn as_menu_item(&self) -> &dyn IsMenuItem {
        match self {
            Self::Command(item) => item,
            Self::Check(item) => item,
        }
    }

    fn id(&self) -> &MenuId {
        self.as_menu_item().id()
    }

    fn set_enabled(&self, enabled: bool) {
        match self {
            Self::Command(item) => item.set_enabled(enabled),
            Self::Check(item) => item.set_enabled(enabled),
        }
    }

    fn set_checked(&self, checked: bool) {
        if let Self::Check(item) = self {
            item.set_checked(checked);
        }
    }
}

impl Menu {
    pub fn new() -> Self {
        let mut items = Vec::new();
        let root = MudaMenu::new();
        for section in definition() {
            let submenu = Submenu::new(section.label, true);
            for entry in section.entries {
                let (command, label) = match entry {
                    Entry::Command(command) => (command, None),
                    Entry::Labeled(command, label) => (command, Some(label)),
                    Entry::Separator => {
                        submenu
                            .append(&PredefinedMenuItem::separator())
                            .expect("menu separator");
                        continue;
                    }
                    Entry::About => {
                        submenu
                            .append(&PredefinedMenuItem::about(None, None))
                            .expect("about item");
                        continue;
                    }
                };
                let native = NativeItem::new(command, label);
                submenu
                    .append(native.as_menu_item())
                    .expect("native menu item");
                items.push((command, native));
            }
            root.append(&submenu).expect("menu section");
        }
        Self { root, items }
    }

    pub fn install(&self) {
        self.root.init_for_nsapp();
    }

    pub fn command(&self, event: &Event) -> Option<Command> {
        self.items
            .iter()
            .find(|(_, item)| item.id() == event.0.id())
            .map(|(command, _)| *command)
    }

    pub fn sync(&self, availability: Availability, view: ViewFlags, raw: bool) {
        for (command, item) in &self.items {
            item.set_enabled(availability.enabled(*command));
            item.set_checked(match command {
                Command::Doc(DocCommand::Raw) => raw,
                Command::Doc(DocCommand::DebugGeometry) => view.debug_geometry,
                _ => false,
            });
        }
    }
}

pub fn route_events(proxy: EventLoopProxy<UserEvent>) {
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(UserEvent::NativeMenu(Event(event)));
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commands(sections: &[Section]) -> Vec<Command> {
        sections
            .iter()
            .flat_map(|section| {
                section.entries.iter().filter_map(|entry| match entry {
                    Entry::Command(command) | Entry::Labeled(command, _) => Some(*command),
                    Entry::Separator | Entry::About => None,
                })
            })
            .collect()
    }

    #[test]
    fn the_native_tree_lists_every_command_once() {
        let definition = definition();
        assert_eq!(
            definition
                .iter()
                .map(|section| section.label)
                .collect::<Vec<_>>(),
            vec!["Progred", "File", "Examples", "Edit", "View"]
        );
        let commands = commands(&definition);
        assert_eq!(commands.len(), 19);
        for (index, command) in commands.iter().enumerate() {
            assert!(commands[index + 1..].iter().all(|other| command != other));
        }
    }

    #[test]
    fn both_menu_systems_expose_the_same_commands() {
        let mut native = commands(&definition());
        let drawn_definition = crate::menu::definition();
        let mut drawn = crate::menu::commands(&drawn_definition).collect::<Vec<_>>();
        let key = |command: &Command| format!("{command:?}");
        native.sort_by_key(key);
        drawn.sort_by_key(key);
        assert_eq!(native, drawn);
    }
}
