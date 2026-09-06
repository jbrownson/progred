//! The native macOS menu bar: one application-wide AppKit menu tree
//! emitting [`command::Command`]s routed to the focused window.
//! Fully separate from the drawn in-window menu system.

use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSControlStateValueOff, NSControlStateValueOn, NSEventModifierFlags, NSMenu,
    NSMenuItem,
};
use objc2_foundation::{MainThreadMarker, NSObject, NSObjectProtocol, NSString};
use winit::event_loop::EventLoopProxy;

use crate::UserEvent;
use crate::command::{self, AppCommand, Availability, Command, DocCommand, Example, Toggles};

pub struct Event(usize);

struct MenuTargetIvars {
    proxy: EventLoopProxy<UserEvent>,
}

define_class!(
    // SAFETY: NSObject has no subclassing requirements and MenuTarget
    // does not implement Drop.
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[ivars = MenuTargetIvars]
    struct MenuTarget;

    impl MenuTarget {
        // SAFETY: This is the action installed on our NSMenuItems.
        #[unsafe(method(performProgredCommand:))]
        fn perform_command(&self, sender: &NSMenuItem) {
            let index = sender.tag();
            if index >= 0 {
                let _ = self
                    .ivars()
                    .proxy
                    .send_event(UserEvent::NativeMenu(Event(index as usize)));
            }
        }
    }

    // SAFETY: NSObjectProtocol has no safety requirements.
    unsafe impl NSObjectProtocol for MenuTarget {}
);

impl MenuTarget {
    fn new(mtm: MainThreadMarker, proxy: EventLoopProxy<UserEvent>) -> Retained<Self> {
        let this = mtm.alloc().set_ivars(MenuTargetIvars { proxy });
        unsafe { msg_send![super(this), init] }
    }
}

/// The native bar's structure: which commands, where, in what order.
/// Labels, keys, and toggle-ness come from the shared
/// [`command::spec`] catalog; entries may override the label for
/// platform idiom ("Quit Progred" in the application menu).
enum Entry {
    Command(Command),
    Labeled(Command, &'static str),
    Separator,
    /// An AppKit-implemented item routed through the responder chain.
    Native(Native),
}

#[derive(Clone, Copy)]
enum Native {
    About,
    Services,
    Hide,
    HideOthers,
    ShowAll,
    Minimize,
    Zoom,
    Fullscreen,
    BringAllToFront,
}

struct Section {
    label: &'static str,
    entries: Vec<Entry>,
    /// AppKit maintains the open-window list at this menu's tail.
    windows_menu: bool,
}

fn definition() -> Vec<Section> {
    use {AppCommand as A, Command as C, DocCommand as D, Example as E};
    let section = |label, entries| Section {
        label,
        entries,
        windows_menu: false,
    };
    vec![
        section(
            "Progred",
            vec![
                Entry::Native(Native::About),
                Entry::Separator,
                Entry::Native(Native::Services),
                Entry::Separator,
                Entry::Native(Native::Hide),
                Entry::Native(Native::HideOthers),
                Entry::Native(Native::ShowAll),
                Entry::Separator,
                Entry::Labeled(C::App(A::Quit), "Quit Progred"),
            ],
        ),
        section(
            "File",
            vec![
                Entry::Command(C::App(A::New)),
                Entry::Command(C::App(A::NewWindow)),
                Entry::Command(C::App(A::Open)),
                Entry::Separator,
                Entry::Command(C::App(A::Close)),
                Entry::Command(C::Doc(D::Save)),
                Entry::Command(C::Doc(D::SaveAs)),
            ],
        ),
        section(
            "Examples",
            E::ALL
                .into_iter()
                .map(|example| Entry::Command(C::App(A::Example(example))))
                .collect(),
        ),
        section(
            "Edit",
            vec![
                Entry::Command(C::Doc(D::Undo)),
                Entry::Command(C::Doc(D::Redo)),
            ],
        ),
        section(
            "View",
            vec![
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
                Entry::Separator,
                Entry::Native(Native::Fullscreen),
            ],
        ),
        Section {
            label: "Window",
            entries: vec![
                Entry::Native(Native::Minimize),
                Entry::Native(Native::Zoom),
                Entry::Separator,
                Entry::Native(Native::BringAllToFront),
            ],
            windows_menu: true,
        },
    ]
}

fn item(
    mtm: MainThreadMarker,
    title: &str,
    action: Option<objc2::runtime::Sel>,
    key: &str,
    modifiers: NSEventModifierFlags,
) -> Retained<NSMenuItem> {
    let item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            mtm.alloc(),
            &NSString::from_str(title),
            action,
            &NSString::from_str(key),
        )
    };
    item.setKeyEquivalentModifierMask(modifiers);
    item.setEnabled(true);
    item
}

fn shortcut(shortcut: command::Shortcut) -> (String, NSEventModifierFlags) {
    (
        shortcut.key.label().to_ascii_lowercase(),
        NSEventModifierFlags::Command
            | if shortcut.shift {
                NSEventModifierFlags::Shift
            } else {
                NSEventModifierFlags::empty()
            },
    )
}

fn add_command(
    mtm: MainThreadMarker,
    menu: &NSMenu,
    target: &MenuTarget,
    items: &mut Vec<(Command, Retained<NSMenuItem>)>,
    command: Command,
    label: Option<&str>,
) {
    let spec = command::spec(command);
    let (key, modifiers) = spec
        .shortcut
        .map(shortcut)
        .unwrap_or_else(|| (String::new(), NSEventModifierFlags::empty()));
    let native = item(
        mtm,
        label.unwrap_or(spec.label),
        Some(sel!(performProgredCommand:)),
        &key,
        modifiers,
    );
    native.setTag(items.len() as isize);
    unsafe { native.setTarget(Some(target)) };
    menu.addItem(&native);
    items.push((command, native));
}

fn native_item(
    mtm: MainThreadMarker,
    native: Native,
) -> (Retained<NSMenuItem>, Option<Retained<NSMenu>>) {
    let plain = NSEventModifierFlags::empty();
    match native {
        Native::About => (
            item(
                mtm,
                "About Progred",
                Some(sel!(orderFrontStandardAboutPanel:)),
                "",
                plain,
            ),
            None,
        ),
        Native::Services => {
            let services = NSMenu::new(mtm);
            let item = item(mtm, "Services", None, "", plain);
            item.setSubmenu(Some(&services));
            (item, Some(services))
        }
        Native::Hide => (
            item(
                mtm,
                "Hide Progred",
                Some(sel!(hide:)),
                "h",
                NSEventModifierFlags::Command,
            ),
            None,
        ),
        Native::HideOthers => (
            item(
                mtm,
                "Hide Others",
                Some(sel!(hideOtherApplications:)),
                "h",
                NSEventModifierFlags::Command | NSEventModifierFlags::Option,
            ),
            None,
        ),
        Native::ShowAll => (
            item(
                mtm,
                "Show All",
                Some(sel!(unhideAllApplications:)),
                "",
                plain,
            ),
            None,
        ),
        Native::Minimize => (
            item(
                mtm,
                "Minimize",
                Some(sel!(performMiniaturize:)),
                "m",
                NSEventModifierFlags::Command,
            ),
            None,
        ),
        Native::Zoom => (item(mtm, "Zoom", Some(sel!(performZoom:)), "", plain), None),
        Native::Fullscreen => (
            item(
                mtm,
                "Enter Full Screen",
                Some(sel!(toggleFullScreen:)),
                "f",
                NSEventModifierFlags::Command | NSEventModifierFlags::Control,
            ),
            None,
        ),
        Native::BringAllToFront => (
            item(
                mtm,
                "Bring All to Front",
                Some(sel!(arrangeInFront:)),
                "",
                plain,
            ),
            None,
        ),
    }
}

pub struct Menu {
    root: Retained<NSMenu>,
    items: Vec<(Command, Retained<NSMenuItem>)>,
    windows: Retained<NSMenu>,
    services: Retained<NSMenu>,
    /// NSMenuItem's target is weak, so the menu adapter owns the bridge.
    _target: Retained<MenuTarget>,
}

impl Menu {
    pub fn new(proxy: EventLoopProxy<UserEvent>) -> Self {
        let mtm = MainThreadMarker::new().expect("menus are created on the main thread");
        let target = MenuTarget::new(mtm, proxy);
        let root = NSMenu::new(mtm);
        root.setAutoenablesItems(false);
        let mut items = Vec::new();
        let mut windows = None;
        let mut services = None;

        for section in definition() {
            let submenu = NSMenu::new(mtm);
            submenu.setTitle(&NSString::from_str(section.label));
            submenu.setAutoenablesItems(false);

            for entry in section.entries {
                match entry {
                    Entry::Command(command) => {
                        add_command(mtm, &submenu, &target, &mut items, command, None)
                    }
                    Entry::Labeled(command, label) => {
                        add_command(mtm, &submenu, &target, &mut items, command, Some(label))
                    }
                    Entry::Separator => submenu.addItem(&NSMenuItem::separatorItem(mtm)),
                    Entry::Native(native) => {
                        let (item, service_menu) = native_item(mtm, native);
                        services = services.or(service_menu);
                        submenu.addItem(&item);
                    }
                }
            }

            let section_item = item(mtm, section.label, None, "", NSEventModifierFlags::empty());
            section_item.setSubmenu(Some(&submenu));
            root.addItem(&section_item);
            if section.windows_menu {
                windows = Some(submenu);
            }
        }

        Self {
            root,
            items,
            windows: windows.expect("native menu has a Window section"),
            services: services.expect("native menu has a Services item"),
            _target: target,
        }
    }

    pub fn install(&self) {
        let mtm = MainThreadMarker::new().expect("menus install on the main thread");
        let app = NSApplication::sharedApplication(mtm);
        app.setMainMenu(Some(&self.root));
        app.setWindowsMenu(Some(&self.windows));
        app.setServicesMenu(Some(&self.services));
    }

    pub fn command(&self, event: &Event) -> Option<Command> {
        self.items.get(event.0).map(|(command, _)| *command)
    }

    /// `None` is the windowless state: every document command grays,
    /// application commands stay live.
    pub fn sync(&self, doc: Option<(Availability, Toggles)>) {
        for (command, item) in &self.items {
            item.setEnabled(match command {
                Command::App(AppCommand::Close) => doc.is_some(),
                Command::App(_) => true,
                Command::Doc(command) => {
                    doc.is_some_and(|(availability, _)| availability.doc_enabled(*command))
                }
            });
            if command::spec(*command).toggle {
                item.setState(
                    if doc.is_some_and(|(_, toggles)| toggles.checked(*command)) {
                        NSControlStateValueOn
                    } else {
                        NSControlStateValueOff
                    },
                );
            }
        }
    }
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
                    Entry::Separator | Entry::Native(_) => None,
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
            vec!["Progred", "File", "Examples", "Edit", "View", "Window"]
        );
        let commands = commands(&definition);
        assert_eq!(commands.len(), 24);
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
