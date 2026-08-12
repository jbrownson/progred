//! Platform-neutral menu state and selections, plus Linux's Progred-drawn view.

use ui_events::keyboard::{Key, KeyboardEvent};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    Linux,
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    MacOs,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Selection {
    New,
    Open,
    Save,
    SaveAs,
    Quit,
    Undo,
    Redo,
    Raw,
    Graph,
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

    #[cfg(target_os = "linux")]
    fn linux_label(self) -> String {
        format!(
            "Ctrl+{}{}",
            if self.shift { "Shift+" } else { "" },
            self.key.label()
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortcutKey {
    G,
    N,
    O,
    Q,
    R,
    S,
    Z,
}

impl ShortcutKey {
    pub const fn label(self) -> &'static str {
        match self {
            Self::G => "G",
            Self::N => "N",
            Self::O => "O",
            Self::Q => "Q",
            Self::R => "R",
            Self::S => "S",
            Self::Z => "Z",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Item {
    pub selection: Selection,
    pub label: &'static str,
    pub shortcut: Shortcut,
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
    shortcut: Shortcut::plain(ShortcutKey::N),
    kind: Kind::Command,
};
const OPEN: Item = Item {
    selection: Selection::Open,
    label: "Open…",
    shortcut: Shortcut::plain(ShortcutKey::O),
    kind: Kind::Command,
};
const SAVE: Item = Item {
    selection: Selection::Save,
    label: "Save",
    shortcut: Shortcut::plain(ShortcutKey::S),
    kind: Kind::Command,
};
const SAVE_AS: Item = Item {
    selection: Selection::SaveAs,
    label: "Save As…",
    shortcut: Shortcut::shifted(ShortcutKey::S),
    kind: Kind::Command,
};
const QUIT: Item = Item {
    selection: Selection::Quit,
    label: "Quit",
    shortcut: Shortcut::plain(ShortcutKey::Q),
    kind: Kind::Command,
};
const UNDO: Item = Item {
    selection: Selection::Undo,
    label: "Undo",
    shortcut: Shortcut::plain(ShortcutKey::Z),
    kind: Kind::Command,
};
const REDO: Item = Item {
    selection: Selection::Redo,
    label: "Redo",
    shortcut: Shortcut::shifted(ShortcutKey::Z),
    kind: Kind::Command,
};
const RAW: Item = Item {
    selection: Selection::Raw,
    label: "Raw",
    shortcut: Shortcut::plain(ShortcutKey::R),
    kind: Kind::Check,
};
const GRAPH: Item = Item {
    selection: Selection::Graph,
    label: "Graph",
    shortcut: Shortcut::plain(ShortcutKey::G),
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
    let file = Menu {
        label: "File",
        entries: [
            Entry::Item(NEW),
            Entry::Item(OPEN),
            Entry::Separator,
            Entry::Item(SAVE),
            Entry::Item(SAVE_AS),
        ]
        .into_iter()
        .chain(
            (platform == Platform::Linux)
                .then_some([Entry::Separator, Entry::Item(quit)])
                .into_iter()
                .flatten(),
        )
        .collect(),
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
                label: "Edit",
                entries: vec![Entry::Item(UNDO), Entry::Item(REDO)],
            },
            Menu {
                label: "View",
                entries: vec![Entry::Item(RAW), Entry::Item(GRAPH)],
            },
        ])
        .collect()
}

#[cfg(target_os = "linux")]
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
    pub save: bool,
    pub undo: bool,
    pub redo: bool,
}

impl Availability {
    pub fn enabled(self, selection: Selection) -> bool {
        match selection {
            Selection::Save => self.save,
            Selection::Undo => self.undo,
            Selection::Redo => self.redo,
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
        Key::Character(key) => items(&definition(Platform::Linux))
            .find(|item| {
                item.shortcut.shift == modifiers.shift()
                    && key.as_str().eq_ignore_ascii_case(item.shortcut.key.label())
            })
            .map(|item| item.selection),
        _ => None,
    }
}

#[cfg(target_os = "linux")]
mod view {
    use super::{Availability, Entry, Hover, Item, Kind, Platform, Selection, State, definition};
    use crate::hover::HasHover;
    use crate::layout::{self, Extent, Node};
    use puri::draw::Canvas;
    use puri::handler::HasHandler;
    use puri::text::{TextCtx, TextStyle};
    use std::rc::Rc;
    use vello::kurbo::{Affine, Insets, Rect, Stroke};
    use vello::peniko::{Brush, Color};

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
        pub graph: bool,
        pub hover: Option<Hover>,
        pub scale: f64,
        pub width: f64,
    }

    pub struct View<P> {
        pub bar: Node<P>,
        pub heading_width: f64,
        pub popup: Option<(f64, Node<P>)>,
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

    fn hover_target<P: HasHover<Option<Hover>>>(hover: Hover, content: Node<P>) -> Node<P> {
        layout::before(content, move |p, placement| {
            if p.pointer().is_some_and(|point| placement.contains(point)) {
                p.claim_hover(Some(hover));
            }
        })
    }

    fn heading<C: 'static, P: Canvas + HasHandler<C> + HasHover<Option<Hover>>>(
        tcx: &mut TextCtx,
        style: &TextStyle,
        index: usize,
        label: &'static str,
        active: bool,
        scale: f64,
        toggle: Rc<dyn Fn(&mut C, usize)>,
    ) -> Node<P> {
        let content = layout::pad(
            Insets::new(10.0 * scale, 4.0 * scale, 10.0 * scale, 4.0 * scale),
            layout::text(tcx, label, style),
        );
        let content = layout::decorate(content, move |p: &mut P, rect| {
            if active {
                p.fill(rect, Color::new([0.82, 0.83, 0.86, 1.0]), Affine::IDENTITY);
            }
        });
        layout::on_primary_pointer_down(
            hover_target(Hover::Heading(index), content),
            |_| true,
            move |app, _| {
                toggle(app, index);
                true
            },
        )
    }

    fn separator<P: Canvas>(scale: f64, width: f64) -> Node<P> {
        layout::leaf(
            Extent {
                width,
                ascent: 4.0 * scale,
                descent: 3.0 * scale,
            },
            move |p: &mut P, placement| {
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

    fn item<C: 'static, P: Canvas + HasHandler<C> + HasHover<Option<Hover>>>(
        tcx: &mut TextCtx,
        styles: &Styles,
        item: Item,
        checked: bool,
        enabled: bool,
        hovered: bool,
        scale: f64,
        width: f64,
        select: Rc<dyn Fn(&mut C, Selection)>,
    ) -> Node<P> {
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
        let label = layout::text(
            tcx,
            &format!("{}  {}", if checked { "✓" } else { " " }, item.label),
            style,
        );
        let shortcut = layout::text(tcx, &item.shortcut.linux_label(), shortcut_style);
        let gap =
            (width - 24.0 * scale - label.extent.width - shortcut.extent.width).max(12.0 * scale);
        let content = layout::pad(
            Insets::new(12.0 * scale, 5.0 * scale, 12.0 * scale, 5.0 * scale),
            layout::row(gap, vec![label, shortcut]),
        );
        let content = layout::decorate(content, move |p: &mut P, rect| {
            if enabled && hovered {
                p.fill(rect, Color::new([0.86, 0.89, 0.96, 1.0]), Affine::IDENTITY);
            }
        });
        if enabled {
            layout::on_primary_pointer_down(
                hover_target(Hover::Item(selection), content),
                |_| true,
                move |app, _| {
                    select(app, selection);
                    true
                },
            )
        } else {
            content
        }
    }

    fn popup<C: 'static, P: Canvas + HasHandler<C> + HasHover<Option<Hover>>>(
        tcx: &mut TextCtx,
        styles: &Styles,
        description: &Description,
        menu_entries: &[Entry],
        select: Rc<dyn Fn(&mut C, Selection)>,
    ) -> Node<P> {
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
                            Selection::Graph => description.graph,
                            _ => false,
                        },
                    description.availability.enabled(menu_item.selection),
                    description.hover == Some(Hover::Item(menu_item.selection)),
                    description.scale,
                    width,
                    select.clone(),
                ),
                Entry::About => unreachable!("About is only in the macOS application menu"),
            })
            .collect();
        layout::before(
            layout::decorate(layout::col(0, 0.0, entries), move |p: &mut P, rect| {
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
                if p.pointer().is_some_and(|point| placement.contains(point)) {
                    p.claim_hover(None);
                }
            },
        )
    }

    pub fn view<C: 'static, P: Canvas + HasHandler<C> + HasHover<Option<Hover>>>(
        tcx: &mut TextCtx,
        description: Description,
        hooks: Hooks<C>,
    ) -> View<P> {
        let styles = styles();
        let definition = definition(Platform::Linux);
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
                    description.state.open() == Some(index)
                        || description.hover == Some(Hover::Heading(index)),
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
        let bar = layout::decorate(
            layout::min_width(
                description.width,
                layout::row(
                    0.0,
                    // A zero-width baseline strut gives the bar its fixed height
                    // without shifting the headings; the box algebra has no
                    // minimum-ascent-and-descent wrapper yet.
                    std::iter::once(layout::leaf(
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
            |p: &mut P, rect| {
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

#[cfg(target_os = "linux")]
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
        for platform in [Platform::Linux, Platform::MacOs] {
            let definition = definition(platform);
            let items = items(&definition).collect::<Vec<_>>();
            assert_eq!(items.len(), 9);
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
            vec!["Progred", "File", "Edit", "View"]
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
        let linux = definition(Platform::Linux);
        assert_eq!(
            linux.iter().map(|menu| menu.label).collect::<Vec<_>>(),
            vec!["File", "Edit", "View"]
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
        let definition = definition(Platform::Linux);
        let items = items(&definition).collect::<Vec<_>>();
        for (index, item) in items.iter().enumerate() {
            assert!(
                items[index + 1..]
                    .iter()
                    .all(|other| item.shortcut != other.shortcut)
            );
        }
    }
}
