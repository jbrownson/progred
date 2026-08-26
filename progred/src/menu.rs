//! Platform-neutral menu state and selections, plus the Progred-drawn
//! view for platforms without a native application menu.

use ui_events::keyboard::{Key, KeyboardEvent};

/// The one platform switch: the app draws its own menu bar and popups
/// wherever there is no native application menu; macOS speaks to its
/// own through `macos_menu`.
pub const DRAWN: bool = cfg!(not(target_os = "macos"));

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Drawn,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    MacOs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Selection {
    New,
    #[cfg(not(target_arch = "wasm32"))]
    Open,
    #[cfg(not(target_arch = "wasm32"))]
    Save,
    #[cfg(not(target_arch = "wasm32"))]
    SaveAs,
    Quit,
    ExampleSample,
    ExampleGrap,
    ExampleIopTree,
    Undo,
    Redo,
    OpenPaneLeft,
    OpenPaneRight,
    MovePaneUp,
    MovePaneDown,
    MovePaneLeft,
    MovePaneRight,
    Raw,
    DebugGeometry,
}

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
    D,
    N,
    #[cfg(not(target_arch = "wasm32"))]
    O,
    P,
    Q,
    R,
    #[cfg(not(target_arch = "wasm32"))]
    S,
    Z,
}

impl ShortcutKey {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Digit1 => "1",
            Self::Digit2 => "2",
            Self::Digit3 => "3",
            Self::D => "D",
            Self::N => "N",
            #[cfg(not(target_arch = "wasm32"))]
            Self::O => "O",
            Self::P => "P",
            Self::Q => "Q",
            Self::R => "R",
            #[cfg(not(target_arch = "wasm32"))]
            Self::S => "S",
            Self::Z => "Z",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Item {
    pub selection: Selection,
    pub label: &'static str,
    pub shortcut: Option<Shortcut>,
    pub kind: Kind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    Item(Item),
    Separator,
    About,
}

pub struct Menu {
    pub label: &'static str,
    pub entries: Vec<Entry>,
}

pub fn items(menus: &[Menu]) -> impl Iterator<Item = Item> + '_ {
    menus.iter().flat_map(|menu| {
        menu.entries.iter().filter_map(|entry| match entry {
            Entry::Item(item) => Some(*item),
            Entry::Separator | Entry::About => None,
        })
    })
}

const NEW: Item = Item {
    selection: Selection::New,
    label: "New",
    shortcut: Some(Shortcut::plain(ShortcutKey::N)),
    kind: Kind::Command,
};
#[cfg(not(target_arch = "wasm32"))]
const OPEN: Item = Item {
    selection: Selection::Open,
    label: "Open…",
    shortcut: Some(Shortcut::plain(ShortcutKey::O)),
    kind: Kind::Command,
};
#[cfg(not(target_arch = "wasm32"))]
const SAVE: Item = Item {
    selection: Selection::Save,
    label: "Save",
    shortcut: Some(Shortcut::plain(ShortcutKey::S)),
    kind: Kind::Command,
};
#[cfg(not(target_arch = "wasm32"))]
const SAVE_AS: Item = Item {
    selection: Selection::SaveAs,
    label: "Save As…",
    shortcut: Some(Shortcut::shifted(ShortcutKey::S)),
    kind: Kind::Command,
};
const QUIT: Item = Item {
    selection: Selection::Quit,
    label: "Quit",
    shortcut: Some(Shortcut::plain(ShortcutKey::Q)),
    kind: Kind::Command,
};
const EXAMPLE_SAMPLE: Item = Item {
    selection: Selection::ExampleSample,
    label: "Sample",
    shortcut: Some(Shortcut::plain(ShortcutKey::Digit1)),
    kind: Kind::Command,
};
const EXAMPLE_GRAP: Item = Item {
    selection: Selection::ExampleGrap,
    label: "Grap Demo",
    shortcut: Some(Shortcut::plain(ShortcutKey::Digit2)),
    kind: Kind::Command,
};
const EXAMPLE_IOP_TREE: Item = Item {
    selection: Selection::ExampleIopTree,
    label: "Inventing on Principle Tree",
    shortcut: Some(Shortcut::plain(ShortcutKey::Digit3)),
    kind: Kind::Command,
};
const UNDO: Item = Item {
    selection: Selection::Undo,
    label: "Undo",
    shortcut: Some(Shortcut::plain(ShortcutKey::Z)),
    kind: Kind::Command,
};
const REDO: Item = Item {
    selection: Selection::Redo,
    label: "Redo",
    shortcut: Some(Shortcut::shifted(ShortcutKey::Z)),
    kind: Kind::Command,
};
const OPEN_PANE_LEFT: Item = Item {
    selection: Selection::OpenPaneLeft,
    label: "Open Cell on Left",
    shortcut: Some(Shortcut::plain(ShortcutKey::P)),
    kind: Kind::Command,
};
const OPEN_PANE_RIGHT: Item = Item {
    selection: Selection::OpenPaneRight,
    label: "Open Cell on Right",
    shortcut: Some(Shortcut::shifted(ShortcutKey::P)),
    kind: Kind::Command,
};
const MOVE_PANE_UP: Item = Item {
    selection: Selection::MovePaneUp,
    label: "Move Pane Up",
    shortcut: None,
    kind: Kind::Command,
};
const MOVE_PANE_DOWN: Item = Item {
    selection: Selection::MovePaneDown,
    label: "Move Pane Down",
    shortcut: None,
    kind: Kind::Command,
};
const MOVE_PANE_LEFT: Item = Item {
    selection: Selection::MovePaneLeft,
    label: "Move Pane Left",
    shortcut: None,
    kind: Kind::Command,
};
const MOVE_PANE_RIGHT: Item = Item {
    selection: Selection::MovePaneRight,
    label: "Move Pane Right",
    shortcut: None,
    kind: Kind::Command,
};
const RAW: Item = Item {
    selection: Selection::Raw,
    label: "Raw",
    shortcut: Some(Shortcut::plain(ShortcutKey::R)),
    kind: Kind::Check,
};
const DEBUG_GEOMETRY: Item = Item {
    selection: Selection::DebugGeometry,
    label: "Debug Geometry",
    shortcut: Some(Shortcut::plain(ShortcutKey::D)),
    kind: Kind::Check,
};
pub fn definition(platform: Platform) -> Vec<Menu> {
    let quit = Item {
        label: if platform == Platform::MacOs {
            "Quit Progred"
        } else {
            QUIT.label
        },
        ..QUIT
    };
    #[cfg(target_arch = "wasm32")]
    let file_entries = vec![Entry::Item(NEW)];
    #[cfg(not(target_arch = "wasm32"))]
    let mut file_entries = vec![Entry::Item(NEW)];
    #[cfg(not(target_arch = "wasm32"))]
    file_entries.extend([
        Entry::Item(OPEN),
        Entry::Separator,
        Entry::Item(SAVE),
        Entry::Item(SAVE_AS),
    ]);
    #[cfg(not(target_arch = "wasm32"))]
    file_entries.extend(
        (platform == Platform::Drawn)
            .then_some([Entry::Separator, Entry::Item(quit)])
            .into_iter()
            .flatten(),
    );
    let file = Menu {
        label: "File",
        entries: file_entries,
    };
    (platform == Platform::MacOs)
        .then(|| Menu {
            label: "Progred",
            entries: vec![Entry::About, Entry::Separator, Entry::Item(quit)],
        })
        .into_iter()
        .chain([
            file,
            Menu {
                label: "Examples",
                entries: vec![
                    Entry::Item(EXAMPLE_SAMPLE),
                    Entry::Item(EXAMPLE_GRAP),
                    Entry::Item(EXAMPLE_IOP_TREE),
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
    Item(Selection),
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

#[derive(Clone, Copy)]
pub struct Availability {
    #[cfg(not(target_arch = "wasm32"))]
    pub save: bool,
    pub undo: bool,
    pub redo: bool,
    pub open_pane: bool,
    pub move_up: bool,
    pub move_down: bool,
    pub move_left: bool,
    pub move_right: bool,
}

impl Availability {
    pub fn enabled(self, selection: Selection) -> bool {
        match selection {
            #[cfg(not(target_arch = "wasm32"))]
            Selection::Save => self.save,
            Selection::Undo => self.undo,
            Selection::Redo => self.redo,
            Selection::OpenPaneLeft | Selection::OpenPaneRight => self.open_pane,
            Selection::MovePaneUp => self.move_up,
            Selection::MovePaneDown => self.move_down,
            Selection::MovePaneLeft => self.move_left,
            Selection::MovePaneRight => self.move_right,
            _ => true,
        }
    }
}

pub fn shortcut(event: &KeyboardEvent) -> Option<Selection> {
    let modifiers = &event.modifiers;
    if !event.state.is_down() || !modifiers.ctrl() || modifiers.meta() || modifiers.alt() {
        return None;
    }
    match &event.key {
        Key::Character(key) => items(&definition(Platform::Drawn))
            .find(|item| {
                item.shortcut.is_some_and(|shortcut| {
                    shortcut.shift == modifiers.shift()
                        && key.as_str().eq_ignore_ascii_case(shortcut.key.label())
                })
            })
            .map(|item| item.selection),
        _ => None,
    }
}

mod view {
    use super::{Availability, Entry, Hover, Item, Kind, Platform, Selection, State, definition};
    use crate::frame::Hovered;
    use crate::placed::{self, Placed};
    use measured::{self, Extent, Measured};
    use puri::draw::Canvas;
    use puri::text::{TextCtx, TextStyle};
    use std::rc::Rc;
    use kurbo::{Affine, Insets, Rect, Stroke};
    use peniko::{Brush, Color};

    const BAR_HEIGHT: f64 = 30.0;
    const MENU_WIDTH: f64 = 230.0;

    pub struct Hooks<C> {
        pub toggle: Rc<dyn Fn(&mut C, usize)>,
        pub select: Rc<dyn Fn(&mut C, Selection)>,
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
        activatable(
            Hover::Heading(index),
            content,
            move |app| {
                toggle(app, index);
                true
            },
        )
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
        select: Rc<dyn Fn(&mut C, Selection)>,
    ) -> Measured<Placed<C, Cv>> {
        let selection = item.selection;
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
                    Some(Hovered::Menu(Hover::Item(s))) if *s == selection
                );
                if enabled && hovered {
                    cv.fill(rect, Color::new([0.86, 0.89, 0.96, 1.0]), Affine::IDENTITY);
                }
            });
        });
        if enabled {
            activatable(
                Hover::Item(selection),
                content,
                move |app| {
                    select(app, selection);
                    true
                },
            )
        } else {
            content
        }
    }

    fn popup<C: 'static, Cv: Canvas + 'static>(
        tcx: &mut TextCtx,
        styles: &Styles,
        description: &Description,
        menu_entries: &[Entry],
        select: Rc<dyn Fn(&mut C, Selection)>,
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
                        && match menu_item.selection {
                            Selection::Raw => description.raw,
                            Selection::DebugGeometry => description.debug_geometry,
                            _ => false,
                        },
                    description.availability.enabled(menu_item.selection),
                    description.scale,
                    width,
                    select.clone(),
                ),
                Entry::About => unreachable!("About is only in the macOS application menu"),
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
        let definition = definition(Platform::Drawn);
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
    fn shortcuts_are_linux_application_commands() {
        assert_eq!(
            shortcut(&key("s", Modifiers::CONTROL)),
            Some(Selection::Save)
        );
        assert_eq!(
            shortcut(&key("S", Modifiers::CONTROL | Modifiers::SHIFT)),
            Some(Selection::SaveAs)
        );
        assert_eq!(shortcut(&key("s", Modifiers::META)), None);
        assert_eq!(
            shortcut(&key("s", Modifiers::CONTROL | Modifiers::ALT)),
            None
        );
    }

    #[test]
    fn number_shortcuts_open_examples_in_menu_order() {
        assert_eq!(
            shortcut(&key("1", Modifiers::CONTROL)),
            Some(Selection::ExampleSample)
        );
        assert_eq!(
            shortcut(&key("2", Modifiers::CONTROL)),
            Some(Selection::ExampleGrap)
        );
        assert_eq!(
            shortcut(&key("3", Modifiers::CONTROL)),
            Some(Selection::ExampleIopTree)
        );
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
    fn the_shared_tree_lists_every_selection_once() {
        for platform in [Platform::Drawn, Platform::MacOs] {
            let definition = definition(platform);
            let items = items(&definition).collect::<Vec<_>>();
            assert_eq!(items.len(), 18);
            for (index, item) in items.iter().enumerate() {
                assert!(
                    items[index + 1..]
                        .iter()
                        .all(|other| item.selection != other.selection)
                );
            }
        }
    }

    #[test]
    fn platform_trees_place_quit_where_each_platform_expects_it() {
        let macos = definition(Platform::MacOs);
        assert_eq!(
            macos.iter().map(|menu| menu.label).collect::<Vec<_>>(),
            vec!["Progred", "File", "Examples", "Edit", "View"]
        );
        assert_eq!(
            macos[0].entries,
            vec![
                Entry::About,
                Entry::Separator,
                Entry::Item(Item {
                    label: "Quit Progred",
                    ..QUIT
                }),
            ]
        );
        let linux = definition(Platform::Drawn);
        assert_eq!(
            linux.iter().map(|menu| menu.label).collect::<Vec<_>>(),
            vec!["File", "Examples", "Edit", "View"]
        );
        assert_eq!(
            linux[0].entries,
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
    fn shared_shortcuts_are_unique() {
        let definition = definition(Platform::Drawn);
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
