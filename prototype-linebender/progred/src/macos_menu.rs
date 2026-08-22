use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{
    CheckMenuItem, IsMenuItem, Menu as MudaMenu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem,
    Submenu,
};
use winit::event_loop::EventLoopProxy;

use crate::menu::{self, Entry, Item, Kind, Platform, Selection, ShortcutKey};
use crate::model::ViewFlags;
use crate::UserEvent;

pub struct Event(MenuEvent);

pub struct Menu {
    root: MudaMenu,
    items: Vec<(Selection, NativeItem)>,
}

enum NativeItem {
    Command(MenuItem),
    Check(CheckMenuItem),
}

impl NativeItem {
    fn new(item: Item) -> Self {
        match item.kind {
            Kind::Command => {
                Self::Command(MenuItem::new(item.label, true, Some(accelerator(item))))
            }
            Kind::Check => Self::Check(CheckMenuItem::new(
                item.label,
                true,
                false,
                Some(accelerator(item)),
            )),
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

fn accelerator(item: Item) -> Accelerator {
    let shortcut = item.shortcut;
    Accelerator::new(
        Some(if shortcut.shift {
            Modifiers::META | Modifiers::SHIFT
        } else {
            Modifiers::META
        }),
        match shortcut.key {
            ShortcutKey::D => Code::KeyD,
            ShortcutKey::N => Code::KeyN,
            ShortcutKey::O => Code::KeyO,
            ShortcutKey::Q => Code::KeyQ,
            ShortcutKey::R => Code::KeyR,
            ShortcutKey::S => Code::KeyS,
            ShortcutKey::Z => Code::KeyZ,
        },
    )
}

fn item(items: &[(Selection, NativeItem)], selection: Selection) -> &NativeItem {
    &items
        .iter()
        .find(|(candidate, _)| *candidate == selection)
        .expect("shared menu item")
        .1
}

fn section_menu(menu: &menu::Menu, items: &[(Selection, NativeItem)]) -> Submenu {
    let submenu = Submenu::new(menu.label, true);
    for entry in &menu.entries {
        match entry {
            Entry::Item(selection) => submenu
                .append(item(items, selection.selection).as_menu_item())
                .expect("menu item"),
            Entry::Separator => submenu
                .append(&PredefinedMenuItem::separator())
                .expect("menu separator"),
            Entry::About => submenu
                .append(&PredefinedMenuItem::about(None, None))
                .expect("about item"),
        }
    }
    submenu
}

impl Menu {
    pub fn new() -> Self {
        let definition = menu::definition(Platform::MacOs);
        let items = menu::items(&definition)
            .map(|item| (item.selection, NativeItem::new(item)))
            .collect::<Vec<_>>();
        let root = MudaMenu::new();
        for menu in &definition {
            root.append(&section_menu(menu, &items))
                .expect("menu section");
        }
        Self { root, items }
    }

    pub fn install(&self) {
        self.root.init_for_nsapp();
    }

    pub fn selection(&self, event: &Event) -> Option<Selection> {
        self.items
            .iter()
            .find(|(_, item)| item.id() == event.0.id())
            .map(|(selection, _)| *selection)
    }

    pub fn sync(&self, availability: menu::Availability, view: ViewFlags) {
        for (selection, item) in &self.items {
            item.set_enabled(availability.enabled(*selection));
            item.set_checked(match selection {
                Selection::Raw => view.raw,
                Selection::DebugGeometry => view.debug_geometry,
                _ => false,
            });
        }
    }
}

pub fn route_events(proxy: EventLoopProxy<UserEvent>) {
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(UserEvent::MacMenu(Event(event)));
    }));
}
