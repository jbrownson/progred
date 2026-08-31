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
use crate::command::{AppCommand, Availability, Command, DocCommand, Example};
use crate::model::ViewFlags;

pub struct Event(MenuEvent);

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Command,
    Check,
}

#[derive(Clone, Copy)]
struct Item {
    command: Command,
    label: &'static str,
    accelerator: Option<(Code, bool)>,
    kind: Kind,
}

impl Item {
    const fn command(command: Command, label: &'static str, code: Code) -> Self {
        Self {
            command,
            label,
            accelerator: Some((code, false)),
            kind: Kind::Command,
        }
    }

    const fn shifted(command: Command, label: &'static str, code: Code) -> Self {
        Self {
            command,
            label,
            accelerator: Some((code, true)),
            kind: Kind::Command,
        }
    }

    const fn plain(command: Command, label: &'static str) -> Self {
        Self {
            command,
            label,
            accelerator: None,
            kind: Kind::Command,
        }
    }

    const fn check(command: Command, label: &'static str, code: Code) -> Self {
        Self {
            command,
            label,
            accelerator: Some((code, false)),
            kind: Kind::Check,
        }
    }
}

enum Entry {
    Item(Item),
    Separator,
    About,
}

struct Section {
    label: &'static str,
    entries: Vec<Entry>,
}

fn definition() -> Vec<Section> {
    use {AppCommand as A, Command as C, DocCommand as D};
    vec![
        Section {
            label: "Progred",
            entries: vec![
                Entry::About,
                Entry::Separator,
                Entry::Item(Item::command(C::App(A::Quit), "Quit Progred", Code::KeyQ)),
            ],
        },
        Section {
            label: "File",
            entries: vec![
                Entry::Item(Item::command(C::App(A::New), "New", Code::KeyN)),
                Entry::Item(Item::command(C::App(A::Open), "Open…", Code::KeyO)),
                Entry::Separator,
                Entry::Item(Item::command(C::Doc(D::Save), "Save", Code::KeyS)),
                Entry::Item(Item::shifted(C::Doc(D::SaveAs), "Save As…", Code::KeyS)),
            ],
        },
        Section {
            label: "Examples",
            entries: vec![
                Entry::Item(Item::command(
                    C::App(A::Example(Example::Sample)),
                    "Sample",
                    Code::Digit1,
                )),
                Entry::Item(Item::command(
                    C::App(A::Example(Example::Grap)),
                    "Grap Demo",
                    Code::Digit2,
                )),
                Entry::Item(Item::command(
                    C::App(A::Example(Example::IopTree)),
                    "Inventing on Principle Tree",
                    Code::Digit3,
                )),
                Entry::Item(Item::command(
                    C::App(A::Example(Example::Fidget)),
                    "Fidget",
                    Code::Digit4,
                )),
            ],
        },
        Section {
            label: "Edit",
            entries: vec![
                Entry::Item(Item::command(C::Doc(D::Undo), "Undo", Code::KeyZ)),
                Entry::Item(Item::shifted(C::Doc(D::Redo), "Redo", Code::KeyZ)),
            ],
        },
        Section {
            label: "View",
            entries: vec![
                Entry::Item(Item::command(
                    C::Doc(D::OpenPaneLeft),
                    "Open Cell on Left",
                    Code::KeyP,
                )),
                Entry::Item(Item::shifted(
                    C::Doc(D::OpenPaneRight),
                    "Open Cell on Right",
                    Code::KeyP,
                )),
                Entry::Separator,
                Entry::Item(Item::plain(C::Doc(D::MovePaneUp), "Move Pane Up")),
                Entry::Item(Item::plain(C::Doc(D::MovePaneDown), "Move Pane Down")),
                Entry::Item(Item::plain(C::Doc(D::MovePaneLeft), "Move Pane Left")),
                Entry::Item(Item::plain(C::Doc(D::MovePaneRight), "Move Pane Right")),
                Entry::Separator,
                Entry::Item(Item::check(C::Doc(D::Raw), "Raw", Code::KeyR)),
                Entry::Item(Item::check(
                    C::Doc(D::DebugGeometry),
                    "Debug Geometry",
                    Code::KeyD,
                )),
            ],
        },
    ]
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
    fn new(item: Item) -> Self {
        let accelerator = item.accelerator.map(|(code, shift)| {
            Accelerator::new(
                Some(if shift {
                    Modifiers::META | Modifiers::SHIFT
                } else {
                    Modifiers::META
                }),
                code,
            )
        });
        match item.kind {
            Kind::Command => Self::Command(MenuItem::new(item.label, true, accelerator)),
            Kind::Check => Self::Check(CheckMenuItem::new(item.label, true, false, accelerator)),
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
                match entry {
                    Entry::Item(item) => {
                        let native = NativeItem::new(item);
                        submenu
                            .append(native.as_menu_item())
                            .expect("native menu item");
                        items.push((item.command, native));
                    }
                    Entry::Separator => submenu
                        .append(&PredefinedMenuItem::separator())
                        .expect("menu separator"),
                    Entry::About => submenu
                        .append(&PredefinedMenuItem::about(None, None))
                        .expect("about item"),
                }
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

    fn items(sections: &[Section]) -> Vec<Item> {
        sections
            .iter()
            .flat_map(|section| {
                section.entries.iter().filter_map(|entry| match entry {
                    Entry::Item(item) => Some(*item),
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
        let items = items(&definition);
        assert_eq!(items.len(), 19);
        for (index, item) in items.iter().enumerate() {
            assert!(
                items[index + 1..]
                    .iter()
                    .all(|other| item.command != other.command)
            );
        }
    }

    #[test]
    fn native_accelerators_are_unique() {
        let items = items(&definition());
        for (index, item) in items.iter().enumerate() {
            if let Some(accelerator) = item.accelerator {
                assert!(
                    items[index + 1..]
                        .iter()
                        .all(|other| other.accelerator != Some(accelerator))
                );
            }
        }
    }
}
