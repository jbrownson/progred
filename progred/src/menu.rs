//! The Progred-drawn menu system: an in-window bar and popups, one
//! per editor. It emits [`command::Command`]s; the native macOS menu
//! is its own separate system in `native_menu`.

use crate::command::{AppCommand, Command, DocCommand, Example};
use ui_events::keyboard::{Key, KeyboardEvent};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Command,
    Check,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Shortcut {
    pub key: ShortcutKey,
    pub shift: bool,
}

impl Shortcut {
    const fn plain(key: ShortcutKey) -> Self {
        Self { key, shift: false }
    }

    const fn shifted(key: ShortcutKey) -> Self {
        Self { key, shift: true }
    }

    fn drawn_label(self) -> String {
        format!(
            "Ctrl+{}{}",
            if self.shift { "Shift+" } else { "" },
            self.key.label()
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortcutKey {
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    D,
    N,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    O,
    P,
    Q,
    R,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    S,
    Z,
}

impl ShortcutKey {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Digit1 => "1",
            Self::Digit2 => "2",
            Self::Digit3 => "3",
            Self::Digit4 => "4",
            Self::D => "D",
            Self::N => "N",
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            Self::O => "O",
            Self::P => "P",
            Self::Q => "Q",
            Self::R => "R",
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            Self::S => "S",
            Self::Z => "Z",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Item {
    pub command: Command,
    pub label: &'static str,
    pub shortcut: Option<Shortcut>,
    pub kind: Kind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    Item(Item),
    Separator,
}

pub struct Menu {
    pub label: &'static str,
    pub entries: Vec<Entry>,
}

pub fn items(menus: &[Menu]) -> impl Iterator<Item = Item> + '_ {
    menus.iter().flat_map(|menu| {
        menu.entries.iter().filter_map(|entry| match entry {
            Entry::Item(item) => Some(*item),
            Entry::Separator => None,
        })
    })
}

const NEW: Item = Item {
    command: Command::App(AppCommand::New),
    label: "New",
    shortcut: Some(Shortcut::plain(ShortcutKey::N)),
    kind: Kind::Command,
};
#[cfg(any(target_os = "macos", target_os = "linux"))]
const OPEN: Item = Item {
    command: Command::App(AppCommand::Open),
    label: "Open…",
    shortcut: Some(Shortcut::plain(ShortcutKey::O)),
    kind: Kind::Command,
};
#[cfg(any(target_os = "macos", target_os = "linux"))]
const SAVE: Item = Item {
    command: Command::Doc(DocCommand::Save),
    label: "Save",
    shortcut: Some(Shortcut::plain(ShortcutKey::S)),
    kind: Kind::Command,
};
#[cfg(any(target_os = "macos", target_os = "linux"))]
const SAVE_AS: Item = Item {
    command: Command::Doc(DocCommand::SaveAs),
    label: "Save As…",
    shortcut: Some(Shortcut::shifted(ShortcutKey::S)),
    kind: Kind::Command,
};
#[cfg_attr(any(target_arch = "wasm32", target_os = "ios"), allow(dead_code))]
const QUIT: Item = Item {
    command: Command::App(AppCommand::Quit),
    label: "Quit",
    shortcut: Some(Shortcut::plain(ShortcutKey::Q)),
    kind: Kind::Command,
};
const EXAMPLE_SAMPLE: Item = Item {
    command: Command::App(AppCommand::Example(Example::Sample)),
    label: "Sample",
    shortcut: Some(Shortcut::plain(ShortcutKey::Digit1)),
    kind: Kind::Command,
};
const EXAMPLE_GRAP: Item = Item {
    command: Command::App(AppCommand::Example(Example::Grap)),
    label: "Grap Demo",
    shortcut: Some(Shortcut::plain(ShortcutKey::Digit2)),
    kind: Kind::Command,
};
const EXAMPLE_IOP_TREE: Item = Item {
    command: Command::App(AppCommand::Example(Example::IopTree)),
    label: "Inventing on Principle Tree",
    shortcut: Some(Shortcut::plain(ShortcutKey::Digit3)),
    kind: Kind::Command,
};
const EXAMPLE_FIDGET: Item = Item {
    command: Command::App(AppCommand::Example(Example::Fidget)),
    label: "Fidget",
    shortcut: Some(Shortcut::plain(ShortcutKey::Digit4)),
    kind: Kind::Command,
};
const UNDO: Item = Item {
    command: Command::Doc(DocCommand::Undo),
    label: "Undo",
    shortcut: Some(Shortcut::plain(ShortcutKey::Z)),
    kind: Kind::Command,
};
const REDO: Item = Item {
    command: Command::Doc(DocCommand::Redo),
    label: "Redo",
    shortcut: Some(Shortcut::shifted(ShortcutKey::Z)),
    kind: Kind::Command,
};
const OPEN_PANE_LEFT: Item = Item {
    command: Command::Doc(DocCommand::OpenPaneLeft),
    label: "Open Cell on Left",
    shortcut: Some(Shortcut::plain(ShortcutKey::P)),
    kind: Kind::Command,
};
const OPEN_PANE_RIGHT: Item = Item {
    command: Command::Doc(DocCommand::OpenPaneRight),
    label: "Open Cell on Right",
    shortcut: Some(Shortcut::shifted(ShortcutKey::P)),
    kind: Kind::Command,
};
const MOVE_PANE_UP: Item = Item {
    command: Command::Doc(DocCommand::MovePaneUp),
    label: "Move Pane Up",
    shortcut: None,
    kind: Kind::Command,
};
const MOVE_PANE_DOWN: Item = Item {
    command: Command::Doc(DocCommand::MovePaneDown),
    label: "Move Pane Down",
    shortcut: None,
    kind: Kind::Command,
};
const MOVE_PANE_LEFT: Item = Item {
    command: Command::Doc(DocCommand::MovePaneLeft),
    label: "Move Pane Left",
    shortcut: None,
    kind: Kind::Command,
};
const MOVE_PANE_RIGHT: Item = Item {
    command: Command::Doc(DocCommand::MovePaneRight),
    label: "Move Pane Right",
    shortcut: None,
    kind: Kind::Command,
};
const RAW: Item = Item {
    command: Command::Doc(DocCommand::Raw),
    label: "Raw",
    shortcut: Some(Shortcut::plain(ShortcutKey::R)),
    kind: Kind::Check,
};
const DEBUG_GEOMETRY: Item = Item {
    command: Command::Doc(DocCommand::DebugGeometry),
    label: "Debug Geometry",
    shortcut: Some(Shortcut::plain(ShortcutKey::D)),
    kind: Kind::Check,
};
pub fn definition() -> Vec<Menu> {
    #[cfg(any(target_arch = "wasm32", target_os = "ios"))]
    let file_entries = vec![Entry::Item(NEW)];
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    let file_entries = vec![
        Entry::Item(NEW),
        Entry::Item(OPEN),
        Entry::Separator,
        Entry::Item(SAVE),
        Entry::Item(SAVE_AS),
        Entry::Separator,
        Entry::Item(QUIT),
    ];
    let file = Menu {
        label: "File",
        entries: file_entries,
    };
    [file]
        .into_iter()
        .chain([
            Menu {
                label: "Examples",
                entries: vec![
                    Entry::Item(EXAMPLE_SAMPLE),
                    Entry::Item(EXAMPLE_GRAP),
                    Entry::Item(EXAMPLE_IOP_TREE),
                    Entry::Item(EXAMPLE_FIDGET),
                ],
            },
            Menu {
                label: "Edit",
                entries: vec![Entry::Item(UNDO), Entry::Item(REDO)],
            },
            Menu {
                label: "View",
                entries: vec![
                    Entry::Item(OPEN_PANE_LEFT),
                    Entry::Item(OPEN_PANE_RIGHT),
                    Entry::Separator,
                    Entry::Item(MOVE_PANE_UP),
                    Entry::Item(MOVE_PANE_DOWN),
                    Entry::Item(MOVE_PANE_LEFT),
                    Entry::Item(MOVE_PANE_RIGHT),
                    Entry::Separator,
                    Entry::Item(RAW),
                    Entry::Item(DEBUG_GEOMETRY),
                ],
            },
        ])
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hover {
    Heading(usize),
    Item(Command),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct State {
    open: Option<usize>,
}

impl State {
    pub fn open(&self) -> Option<usize> {
        self.open
    }

    pub fn toggle(&mut self, menu: usize) {
        self.open = (self.open != Some(menu)).then_some(menu);
    }

    pub fn close(&mut self) -> bool {
        self.open.take().is_some()
    }

    pub fn captures_key(&self, event: &KeyboardEvent) -> bool {
        self.open.is_some() && event.state.is_down()
    }
}

pub fn shortcut(event: &KeyboardEvent) -> Option<Command> {
    let modifiers = &event.modifiers;
    if !event.state.is_down() || !modifiers.ctrl() || modifiers.meta() || modifiers.alt() {
        return None;
    }
    match &event.key {
        Key::Character(key) => items(&definition())
            .find(|item| {
                item.shortcut.is_some_and(|shortcut| {
                    shortcut.shift == modifiers.shift()
                        && key.as_str().eq_ignore_ascii_case(shortcut.key.label())
                })
            })
            .map(|item| item.command),
        _ => None,
    }
}

mod view {
    use super::{Entry, Hover, Item, Kind, State, definition};
    use crate::command::{Availability, Command, DocCommand};
    use crate::frame::Hovered;
    use crate::placed::{self, Placed};
    use kurbo::{Affine, Insets, Rect, Stroke};
    use measured::{self, Extent, Measured};
    use peniko::{Brush, Color};
    use puri::draw::Canvas;
    use puri::text::{TextCtx, TextStyle};
    use std::rc::Rc;

    const BAR_HEIGHT: f64 = 30.0;
    const MENU_WIDTH: f64 = 230.0;

    pub struct Hooks<C> {
        pub toggle: Rc<dyn Fn(&mut C, usize)>,
        pub select: Rc<dyn Fn(&mut C, Command)>,
    }

    pub struct Description {
        pub state: State,
        pub availability: Availability,
        pub raw: bool,
        pub debug_geometry: bool,
        pub scale: f64,
        pub width: f64,
    }

    pub struct View<P> {
        pub bar: Measured<P>,
        pub heading_width: f64,
        pub popup: Option<(f64, Measured<P>)>,
    }

    struct Styles {
        text: TextStyle,
        dim: TextStyle,
        disabled: TextStyle,
    }

    fn styles() -> Styles {
        let text = |color| TextStyle {
            size: 14.0,
            brush: Brush::from(Color::new(color)),
            weight: None,
            family: parley::style::GenericFamily::SystemUi,
        };
        Styles {
            text: text([0.13, 0.14, 0.16, 1.0]),
            dim: text([0.42, 0.44, 0.49, 1.0]),
            disabled: text([0.58, 0.59, 0.62, 1.0]),
        }
    }

    pub fn bar_height(scale: f64) -> f64 {
        BAR_HEIGHT * scale
    }

    fn activatable<C: 'static, Cv: Canvas + 'static>(
        hover: Hover,
        content: Measured<Placed<C, Cv>>,
        action: impl Fn(&mut C) -> bool + 'static,
    ) -> Measured<Placed<C, Cv>> {
        let action = Rc::new(action);
        placed::before(content, move |p, placement| {
            let target = Hovered::Menu(hover);
            p.claim(placement, target.clone());
            let activate = action.clone();
            p.activate(target.clone(), move |world| activate(world));
            p.pick(target, move |world| action(world));
        })
    }

    fn heading<C: 'static, Cv: Canvas + 'static>(
        tcx: &mut TextCtx,
        style: &TextStyle,
        index: usize,
        label: &'static str,
        active: bool,
        scale: f64,
        toggle: Rc<dyn Fn(&mut C, usize)>,
    ) -> Measured<Placed<C, Cv>> {
        let content = measured::pad(
            Insets::new(10.0 * scale, 4.0 * scale, 10.0 * scale, 4.0 * scale),
            crate::render::text(tcx, label, style),
        );
        let content = placed::decorate(content, move |p, rect| {
            p.ink(move |cv: &mut Cv, ink| {
                let hovered =
                    matches!(ink.hovered, Some(Hovered::Menu(Hover::Heading(i))) if *i == index);
                if active || hovered {
                    cv.fill(rect, Color::new([0.82, 0.83, 0.86, 1.0]), Affine::IDENTITY);
                }
            });
        });
        activatable(Hover::Heading(index), content, move |app| {
            toggle(app, index);
            true
        })
    }

    fn separator<C: 'static, Cv: Canvas + 'static>(
        scale: f64,
        width: f64,
    ) -> Measured<Placed<C, Cv>> {
        placed::leaf(
            Extent {
                width,
                ascent: 4.0 * scale,
                descent: 3.0 * scale,
            },
            move |p, placement| {
                let y = placement.rect.center().y;
                p.fill(
                    Rect::new(
                        placement.rect.x0 + 9.0 * scale,
                        y,
                        placement.rect.x1 - 9.0 * scale,
                        y + scale.max(1.0),
                    ),
                    Color::new([0.84, 0.85, 0.87, 1.0]),
                    Affine::IDENTITY,
                );
            },
        )
    }

    fn item<C: 'static, Cv: Canvas + 'static>(
        tcx: &mut TextCtx,
        styles: &Styles,
        item: Item,
        checked: bool,
        enabled: bool,
        scale: f64,
        width: f64,
        select: Rc<dyn Fn(&mut C, Command)>,
    ) -> Measured<Placed<C, Cv>> {
        let command = item.command;
        let style = if enabled {
            &styles.text
        } else {
            &styles.disabled
        };
        let shortcut_style = if enabled {
            &styles.dim
        } else {
            &styles.disabled
        };
        let label = crate::render::text(
            tcx,
            &format!("{}  {}", if checked { "✓" } else { " " }, item.label),
            style,
        );
        let shortcut = crate::render::text(
            tcx,
            &item
                .shortcut
                .map(|shortcut| shortcut.drawn_label())
                .unwrap_or_default(),
            shortcut_style,
        );
        let gap =
            (width - 24.0 * scale - label.extent.width - shortcut.extent.width).max(12.0 * scale);
        let content = measured::pad(
            Insets::new(12.0 * scale, 5.0 * scale, 12.0 * scale, 5.0 * scale),
            measured::row(gap, vec![label, shortcut]),
        );
        let content = placed::decorate(content, move |p, rect| {
            p.ink(move |cv: &mut Cv, ink| {
                let hovered = matches!(
                    ink.hovered,
                    Some(Hovered::Menu(Hover::Item(c))) if *c == command
                );
                if enabled && hovered {
                    cv.fill(rect, Color::new([0.86, 0.89, 0.96, 1.0]), Affine::IDENTITY);
                }
            });
        });
        if enabled {
            activatable(Hover::Item(command), content, move |app| {
                select(app, command);
                true
            })
        } else {
            content
        }
    }

    fn popup<C: 'static, Cv: Canvas + 'static>(
        tcx: &mut TextCtx,
        styles: &Styles,
        description: &Description,
        menu_entries: &[Entry],
        select: Rc<dyn Fn(&mut C, Command)>,
    ) -> Measured<Placed<C, Cv>> {
        let width = MENU_WIDTH * description.scale;
        let scale = description.scale;
        let entries = menu_entries
            .iter()
            .copied()
            .map(|entry| match entry {
                Entry::Separator => separator(description.scale, width),
                Entry::Item(menu_item) => item(
                    tcx,
                    styles,
                    menu_item,
                    menu_item.kind == Kind::Check
                        && match menu_item.command {
                            Command::Doc(DocCommand::Raw) => description.raw,
                            Command::Doc(DocCommand::DebugGeometry) => description.debug_geometry,
                            _ => false,
                        },
                    description.availability.enabled(menu_item.command),
                    description.scale,
                    width,
                    select.clone(),
                ),
            })
            .collect();
        placed::before(
            placed::decorate(measured::col(0, 0.0, entries), move |p, rect| {
                p.fill(
                    rect,
                    Color::new([0.975, 0.975, 0.982, 1.0]),
                    Affine::IDENTITY,
                );
                p.stroke(
                    rect,
                    Stroke::new(scale.max(1.0)),
                    Color::new([0.70, 0.71, 0.74, 1.0]),
                    Affine::IDENTITY,
                );
            }),
            |p, placement| {
                p.occlude(placement);
            },
        )
    }

    pub fn view<C: 'static, Cv: Canvas + 'static>(
        tcx: &mut TextCtx,
        description: Description,
        hooks: Hooks<C>,
    ) -> View<Placed<C, Cv>> {
        let styles = styles();
        let definition = definition();
        let mut x = 0.0;
        let mut popup_x = 0.0;
        let headings = definition
            .iter()
            .enumerate()
            .map(|(index, menu)| {
                if description.state.open() == Some(index) {
                    popup_x = x;
                }
                let node = heading(
                    tcx,
                    &styles.text,
                    index,
                    menu.label,
                    description.state.open() == Some(index),
                    description.scale,
                    hooks.toggle.clone(),
                );
                x += node.extent.width;
                node
            })
            .collect::<Vec<_>>();
        let heading_width = x;
        let popup = description.state.open().and_then(|index| {
            definition
                .get(index)
                .map(|menu| popup(tcx, &styles, &description, &menu.entries, hooks.select))
        });
        let height = bar_height(description.scale);
        let bar = placed::decorate(
            measured::min_width(
                description.width,
                measured::row(
                    0.0,
                    // A zero-width baseline strut gives the bar its fixed height
                    // without shifting the headings; the box algebra has no
                    // minimum-ascent-and-descent wrapper yet.
                    std::iter::once(placed::leaf(
                        Extent {
                            width: 0.0,
                            ascent: height * 0.7,
                            descent: height * 0.3,
                        },
                        |_, _| {},
                    ))
                    .chain(headings)
                    .collect(),
                ),
            ),
            |p, rect| {
                p.fill(rect, Color::new([0.93, 0.93, 0.945, 1.0]), Affine::IDENTITY);
                p.fill(
                    Rect::new(rect.x0, rect.y1 - 1.0, rect.x1, rect.y1),
                    Color::new([0.78, 0.79, 0.82, 1.0]),
                    Affine::IDENTITY,
                );
            },
        );
        View {
            bar,
            heading_width,
            popup: popup.map(|popup| (popup_x, popup)),
        }
    }
}

pub use view::{Description, Hooks, bar_height, view};

#[cfg(test)]
mod tests {
    use super::*;
    use ui_events::keyboard::{KeyState, Modifiers};

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
            shortcut(&key("s", Modifiers::CONTROL)),
            Some(Command::Doc(DocCommand::Save))
        );
        assert_eq!(
            shortcut(&key("S", Modifiers::CONTROL | Modifiers::SHIFT)),
            Some(Command::Doc(DocCommand::SaveAs))
        );
        assert_eq!(shortcut(&key("s", Modifiers::META)), None);
        assert_eq!(
            shortcut(&key("s", Modifiers::CONTROL | Modifiers::ALT)),
            None
        );
    }

    #[test]
    fn number_shortcuts_open_examples_in_menu_order() {
        for (digit, example) in [
            ("1", Example::Sample),
            ("2", Example::Grap),
            ("3", Example::IopTree),
            ("4", Example::Fidget),
        ] {
            assert_eq!(
                shortcut(&key(digit, Modifiers::CONTROL)),
                Some(Command::App(AppCommand::Example(example)))
            );
        }
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
        let definition = definition();
        let items = items(&definition).collect::<Vec<_>>();
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
    fn the_file_menu_carries_the_application_lifecycle() {
        let definition = definition();
        assert_eq!(
            definition.iter().map(|menu| menu.label).collect::<Vec<_>>(),
            vec!["File", "Examples", "Edit", "View"]
        );
        assert_eq!(
            definition[0].entries,
            vec![
                Entry::Item(NEW),
                Entry::Item(OPEN),
                Entry::Separator,
                Entry::Item(SAVE),
                Entry::Item(SAVE_AS),
                Entry::Separator,
                Entry::Item(QUIT),
            ]
        );
    }

    #[test]
    fn drawn_shortcuts_are_unique() {
        let definition = definition();
        let items = items(&definition).collect::<Vec<_>>();
        for (index, item) in items.iter().enumerate() {
            if let Some(shortcut) = item.shortcut {
                assert!(
                    items[index + 1..]
                        .iter()
                        .all(|other| other.shortcut != Some(shortcut))
                );
            }
        }
    }
}
