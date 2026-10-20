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
    Labeled(Command, String),
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

/// `Projections` before the Window menu: the presets, and an entry for each
/// library an area can switch off.
fn definition<'a>(projections: impl IntoIterator<Item = (gid::CellId, &'a str)>) -> Vec<Section> {
    use crate::workspace::{Move, Side};
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
                Entry::Labeled(C::App(A::Quit), "Quit Progred".into()),
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
                Entry::Command(C::Doc(D::OpenPane(Side::Left))),
                Entry::Command(C::Doc(D::OpenPane(Side::Right))),
                Entry::Command(C::Doc(D::ClosePane)),
                Entry::Separator,
                Entry::Command(C::Doc(D::MovePane(Move::Up))),
                Entry::Command(C::Doc(D::MovePane(Move::Down))),
                Entry::Command(C::Doc(D::MovePane(Move::Left))),
                Entry::Command(C::Doc(D::MovePane(Move::Right))),
                Entry::Separator,
                Entry::Command(C::Doc(D::ActualSize)),
                Entry::Command(C::Doc(D::ZoomIn)),
                Entry::Command(C::Doc(D::ZoomOut)),
                Entry::Separator,
                Entry::Command(C::App(A::Appearance(None))),
                Entry::Command(C::App(A::Appearance(Some(winit::window::Theme::Light)))),
                Entry::Command(C::App(A::Appearance(Some(winit::window::Theme::Dark)))),
                Entry::Separator,
                Entry::Native(Native::Fullscreen),
            ],
        ),
        section(
            "Projections",
            crate::stack::Preset::ALL
                .into_iter()
                .map(|preset| Entry::Command(C::Doc(D::Preset(preset))))
                .chain([Entry::Separator])
                .chain(projections.into_iter().map(|(library, name)| {
                    Entry::Labeled(C::Doc(D::Projection(library)), name.into())
                }))
                .collect(),
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
    let mut add = |key: &str, modifiers| {
        let native = item(
            mtm,
            label.unwrap_or(spec.label),
            Some(sel!(performProgredCommand:)),
            key,
            modifiers,
        );
        native.setTag(items.len() as isize);
        unsafe { native.setTarget(Some(target)) };
        menu.addItem(&native);
        items.push((command, native.clone()));
        native
    };
    add(&key, modifiers);
    // ⌘+ is typed with Shift; a hidden twin takes ⌘= as well, as browsers do.
    if spec
        .shortcut
        .is_some_and(|shortcut| shortcut.key == command::ShortcutKey::Plus)
    {
        let twin = add("=", NSEventModifierFlags::Command);
        twin.setHidden(true);
        twin.setAllowsKeyEquivalentWhenHidden(true);
    }
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
    /// The bar, with a `Projections` entry for each of `projections`.
    pub fn new<'a>(
        proxy: EventLoopProxy<UserEvent>,
        projections: impl IntoIterator<Item = (gid::CellId, &'a str)>,
    ) -> Self {
        let mtm = MainThreadMarker::new().expect("menus are created on the main thread");
        let target = MenuTarget::new(mtm, proxy);
        let root = NSMenu::new(mtm);
        root.setAutoenablesItems(false);
        let mut items = Vec::new();
        let mut windows = None;
        let mut services = None;

        for section in definition(projections) {
            let submenu = NSMenu::new(mtm);
            submenu.setTitle(&NSString::from_str(section.label));
            submenu.setAutoenablesItems(false);

            for entry in section.entries {
                match entry {
                    Entry::Command(command) => {
                        add_command(mtm, &submenu, &target, &mut items, command, None)
                    }
                    Entry::Labeled(command, label) => {
                        add_command(mtm, &submenu, &target, &mut items, command, Some(&label))
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
    pub fn sync(
        &self,
        doc: Option<(Availability, Toggles)>,
        appearance: Option<winit::window::Theme>,
    ) {
        for (command, item) in &self.items {
            item.setEnabled(match command {
                Command::App(AppCommand::Close) => doc.is_some(),
                Command::App(_) => true,
                Command::Doc(command) => doc
                    .as_ref()
                    .is_some_and(|(availability, _)| availability.doc_enabled(*command)),
            });
            if command::spec(*command).toggle {
                item.setState(
                    if checked(
                        *command,
                        doc.as_ref().map(|(_, toggles)| toggles),
                        appearance,
                    ) {
                        NSControlStateValueOn
                    } else {
                        NSControlStateValueOff
                    },
                );
            }
        }
    }
}

fn checked(
    command: Command,
    doc: Option<&Toggles>,
    appearance: Option<winit::window::Theme>,
) -> bool {
    match command {
        Command::App(AppCommand::Appearance(theme)) => theme == appearance,
        _ => doc.is_some_and(|toggles| toggles.checked(command)),
    }
}

#[cfg(test)]
mod tests;
