use muda::accelerator::{Accelerator, Code, Modifiers};
use muda::{
    CheckMenuItem, Menu as MudaMenu, MenuEvent, MenuId, MenuItem, PredefinedMenuItem, Submenu,
};
use winit::event_loop::EventLoopProxy;

use crate::UserEvent;

pub struct Event(MenuEvent);

#[derive(Clone, Copy)]
pub enum EventKind {
    NewSelected,
    OpenSelected,
    SaveSelected,
    SaveAsSelected,
    QuitSelected,
    UndoSelected,
    RedoSelected,
    ViewChanged,
}

pub struct Menu {
    root: MudaMenu,
    ids: MenuIds,
    items: MenuItems,
}

struct MenuIds {
    new: MenuId,
    open: MenuId,
    save: MenuId,
    save_as: MenuId,
    quit: MenuId,
    undo: MenuId,
    redo: MenuId,
    graph: MenuId,
    raw: MenuId,
}

struct MenuItems {
    save: MenuItem,
    undo: MenuItem,
    redo: MenuItem,
    graph: CheckMenuItem,
    raw: CheckMenuItem,
}

impl Menu {
    pub fn new() -> Self {
        let accel = Modifiers::META;
        let new = MenuItem::new(
            "New",
            true,
            Some(Accelerator::new(Some(accel), Code::KeyN)),
        );
        let open = MenuItem::new(
            "Open…",
            true,
            Some(Accelerator::new(Some(accel), Code::KeyO)),
        );
        let save = MenuItem::new(
            "Save",
            true,
            Some(Accelerator::new(Some(accel), Code::KeyS)),
        );
        let save_as = MenuItem::new(
            "Save As…",
            true,
            Some(Accelerator::new(
                Some(accel | Modifiers::SHIFT),
                Code::KeyS,
            )),
        );
        let quit = MenuItem::new(
            "Quit Progred",
            true,
            Some(Accelerator::new(Some(accel), Code::KeyQ)),
        );
        let undo = MenuItem::new(
            "Undo",
            true,
            Some(Accelerator::new(Some(accel), Code::KeyZ)),
        );
        let redo = MenuItem::new(
            "Redo",
            true,
            Some(Accelerator::new(
                Some(accel | Modifiers::SHIFT),
                Code::KeyZ,
            )),
        );
        let graph = CheckMenuItem::new(
            "Graph",
            true,
            false,
            Some(Accelerator::new(Some(accel), Code::KeyG)),
        );
        let raw = CheckMenuItem::new(
            "Raw",
            true,
            false,
            Some(Accelerator::new(Some(accel), Code::KeyR)),
        );
        let root = MudaMenu::new();
        let ids = MenuIds {
            new: new.id().clone(),
            open: open.id().clone(),
            save: save.id().clone(),
            save_as: save_as.id().clone(),
            quit: quit.id().clone(),
            undo: undo.id().clone(),
            redo: redo.id().clone(),
            graph: graph.id().clone(),
            raw: raw.id().clone(),
        };
        root.append_items(&[
            &Submenu::with_items(
                "Progred",
                true,
                &[
                    &PredefinedMenuItem::about(None, None),
                    &PredefinedMenuItem::separator(),
                    &quit,
                ],
            )
            .expect("app menu"),
            &Submenu::with_items(
                "File",
                true,
                &[
                    &new,
                    &open,
                    &PredefinedMenuItem::separator(),
                    &save,
                    &save_as,
                ],
            )
            .expect("file menu"),
            &Submenu::with_items("Edit", true, &[&undo, &redo]).expect("edit menu"),
            &Submenu::with_items("View", true, &[&raw, &graph]).expect("view menu"),
        ])
        .expect("menu bar");
        Self {
            root,
            ids,
            items: MenuItems {
                save,
                undo,
                redo,
                graph,
                raw,
            },
        }
    }

    pub fn install(&self) {
        self.root.init_for_nsapp();
    }

    pub fn event_kind(&self, event: &Event) -> Option<EventKind> {
        let id = event.0.id();
        if *id == self.ids.new {
            Some(EventKind::NewSelected)
        } else if *id == self.ids.open {
            Some(EventKind::OpenSelected)
        } else if *id == self.ids.save {
            Some(EventKind::SaveSelected)
        } else if *id == self.ids.save_as {
            Some(EventKind::SaveAsSelected)
        } else if *id == self.ids.quit {
            Some(EventKind::QuitSelected)
        } else if *id == self.ids.undo {
            Some(EventKind::UndoSelected)
        } else if *id == self.ids.redo {
            Some(EventKind::RedoSelected)
        } else if *id == self.ids.graph || *id == self.ids.raw {
            Some(EventKind::ViewChanged)
        } else {
            None
        }
    }

    pub fn sync(&self, save: bool, undo: bool, redo: bool) {
        self.items.save.set_enabled(save);
        self.items.undo.set_enabled(undo);
        self.items.redo.set_enabled(redo);
    }

    pub fn graph(&self) -> bool {
        self.items.graph.is_checked()
    }

    pub fn raw(&self) -> bool {
        self.items.raw.is_checked()
    }
}

pub fn route_events(proxy: EventLoopProxy<UserEvent>) {
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(UserEvent::Menu(Event(event)));
    }));
}
