//! The Progred-drawn menu system: an in-window bar and popups, one
//! per editor. It emits [`command::Command`]s; the native macOS menu
//! is its own separate system in `native_menu`.

use crate::command::{self, AppCommand, Command, DocCommand, Example};
use ui_events::keyboard::{Key, KeyboardEvent};

/// The drawn bar's structure: which commands, in which menus, in
/// which order. Labels, keys, and toggle-ness come from the shared
/// [`command::spec`] catalog.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Entry {
    Command(Command),
    Separator,
}

pub struct Menu {
    pub label: &'static str,
    pub entries: Vec<Entry>,
}

pub fn commands(menus: &[Menu]) -> impl Iterator<Item = Command> + '_ {
    menus.iter().flat_map(|menu| {
        menu.entries.iter().filter_map(|entry| match entry {
            Entry::Command(command) => Some(*command),
            Entry::Separator => None,
        })
    })
}

/// The drawn shortcut spelling: the drawn system's modifier is Ctrl.
fn drawn_label(shortcut: command::Shortcut) -> String {
    format!(
        "Ctrl+{}{}",
        if shortcut.shift { "Shift+" } else { "" },
        shortcut.key.label()
    )
}

pub fn definition() -> Vec<Menu> {
    use {AppCommand as A, Command as C, DocCommand as D, Example as E};
    #[cfg(any(target_arch = "wasm32", target_os = "ios"))]
    let file_entries = vec![Entry::Command(C::App(A::New))];
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    let file_entries = vec![
        Entry::Command(C::App(A::New)),
        Entry::Command(C::App(A::NewWindow)),
        Entry::Command(C::App(A::Open)),
        Entry::Separator,
        Entry::Command(C::App(A::Close)),
        Entry::Command(C::Doc(D::Save)),
        Entry::Command(C::Doc(D::SaveAs)),
        Entry::Separator,
        Entry::Command(C::App(A::Quit)),
    ];
    vec![
        Menu {
            label: "File",
            entries: file_entries,
        },
        Menu {
            label: "Examples",
            entries: E::ALL
                .into_iter()
                .map(|example| Entry::Command(C::App(A::Example(example))))
                .collect(),
        },
        Menu {
            label: "Edit",
            entries: vec![
                Entry::Command(C::Doc(D::Undo)),
                Entry::Command(C::Doc(D::Redo)),
            ],
        },
        Menu {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Hover {
    Heading(usize),
    Item(Command),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct State {
    open: Option<usize>,
    /// Keyboard cursor: position among the open menu's command
    /// entries.
    cursor: Option<usize>,
}

impl State {
    pub fn open(&self) -> Option<usize> {
        self.open
    }

    pub fn cursor(&self) -> Option<usize> {
        self.cursor
    }

    pub fn toggle(&mut self, menu: usize) {
        self.open = (self.open != Some(menu)).then_some(menu);
        self.cursor = None;
    }

    pub fn close(&mut self) -> bool {
        self.cursor = None;
        self.open.take().is_some()
    }

    pub fn captures_key(&self, event: &KeyboardEvent) -> bool {
        self.open.is_some() && event.state.is_down()
    }
}

pub enum Navigation {
    Pass,
    Handled,
    Activate(Command),
}

/// Keyboard navigation inside an open drawn menu: vertical arrows walk
/// the enabled items, horizontal arrows switch menus, Enter or Space
/// activates the cursored item. Escape remains the caller's close.
pub fn navigate(
    state: &mut State,
    menus: &[Menu],
    availability: command::Availability,
    event: &KeyboardEvent,
) -> Navigation {
    use ui_events::keyboard::NamedKey;
    let Some(open) = state.open else {
        return Navigation::Pass;
    };
    if !event.state.is_down() {
        return Navigation::Pass;
    }
    let items = menus
        .get(open)
        .map(|menu| {
            menu.entries
                .iter()
                .filter_map(|entry| match entry {
                    Entry::Command(command) => Some((*command, availability.enabled(*command))),
                    Entry::Separator => None,
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let step = |from: Option<usize>, delta: isize| -> Option<usize> {
        if items.iter().all(|(_, enabled)| !enabled) {
            return None;
        }
        let len = items.len() as isize;
        let mut at = match from {
            Some(at) => at as isize + delta,
            None if delta > 0 => 0,
            None => len - 1,
        };
        loop {
            at = at.rem_euclid(len);
            if items[at as usize].1 {
                return Some(at as usize);
            }
            at += delta;
        }
    };
    match &event.key {
        Key::Named(NamedKey::ArrowDown) => {
            state.cursor = step(state.cursor, 1);
            Navigation::Handled
        }
        Key::Named(NamedKey::ArrowUp) => {
            state.cursor = step(state.cursor, -1);
            Navigation::Handled
        }
        Key::Named(NamedKey::ArrowLeft) if !menus.is_empty() => {
            state.open = Some((open + menus.len() - 1) % menus.len());
            state.cursor = None;
            Navigation::Handled
        }
        Key::Named(NamedKey::ArrowRight) if !menus.is_empty() => {
            state.open = Some((open + 1) % menus.len());
            state.cursor = None;
            Navigation::Handled
        }
        Key::Named(NamedKey::Enter) => match state.cursor.and_then(|at| items.get(at)) {
            Some((command, true)) => Navigation::Activate(*command),
            _ => Navigation::Handled,
        },
        Key::Character(c) if c.as_str() == " " => match state.cursor.and_then(|at| items.get(at)) {
            Some((command, true)) => Navigation::Activate(*command),
            _ => Navigation::Handled,
        },
        _ => Navigation::Pass,
    }
}

pub fn shortcut(event: &KeyboardEvent) -> Option<Command> {
    let modifiers = &event.modifiers;
    if !event.state.is_down() || !modifiers.ctrl() || modifiers.meta() || modifiers.alt() {
        return None;
    }
    match &event.key {
        Key::Character(key) => commands(&definition()).find(|command| {
            command::spec(*command).shortcut.is_some_and(|shortcut| {
                shortcut.shift == modifiers.shift()
                    && key.as_str().eq_ignore_ascii_case(shortcut.key.label())
            })
        }),
        _ => None,
    }
}

mod view {
    use super::{Entry, Hover, State, definition, drawn_label};
    use crate::command::{Availability, Command, Spec, Toggles, spec};
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
        pub toggles: Toggles,
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

    fn activatable<C: 'static>(
        hover: Hover,
        content: Measured<Placed<C>>,
        action: impl Fn(&mut C) -> bool + 'static,
    ) -> Measured<Placed<C>> {
        let action = Rc::new(action);
        placed::before(content, move |p, placement| {
            let target = Hovered::Menu(hover);
            p.claim(placement, target.clone());
            let activate = action.clone();
            p.activate(target.clone(), move |world| activate(world));
            p.pick(target, move |world| action(world));
        })
    }

    fn heading<C: 'static>(
        tcx: &mut TextCtx,
        style: &TextStyle,
        index: usize,
        label: &'static str,
        active: bool,
        scale: f64,
        toggle: Rc<dyn Fn(&mut C, usize)>,
    ) -> Measured<Placed<C>> {
        let content = measured::pad(
            Insets::new(10.0 * scale, 4.0 * scale, 10.0 * scale, 4.0 * scale),
            crate::render::text(tcx, label, style),
        );
        let content = placed::decorate(content, move |p, rect| {
            p.ink(move |cv: &mut dyn puri::draw::CanvasSink, ink| {
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

    fn separator<C: 'static>(scale: f64, width: f64) -> Measured<Placed<C>> {
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

    fn item<C: 'static>(
        tcx: &mut TextCtx,
        styles: &Styles,
        command: Command,
        spec: Spec,
        checked: bool,
        enabled: bool,
        cursored: bool,
        scale: f64,
        width: f64,
        select: Rc<dyn Fn(&mut C, Command)>,
    ) -> Measured<Placed<C>> {
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
            &format!("{}  {}", if checked { "✓" } else { " " }, spec.label),
            style,
        );
        let shortcut = crate::render::text(
            tcx,
            &spec.shortcut.map(drawn_label).unwrap_or_default(),
            shortcut_style,
        );
        let gap =
            (width - 24.0 * scale - label.extent.width - shortcut.extent.width).max(12.0 * scale);
        let content = measured::pad(
            Insets::new(12.0 * scale, 5.0 * scale, 12.0 * scale, 5.0 * scale),
            measured::row(gap, vec![label, shortcut]),
        );
        let content = placed::decorate(content, move |p, rect| {
            p.ink(move |cv: &mut dyn puri::draw::CanvasSink, ink| {
                let hovered = matches!(
                    ink.hovered,
                    Some(Hovered::Menu(Hover::Item(c))) if *c == command
                );
                if enabled && (hovered || cursored) {
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

    fn popup<C: 'static>(
        tcx: &mut TextCtx,
        styles: &Styles,
        description: &Description,
        menu_entries: &[Entry],
        select: Rc<dyn Fn(&mut C, Command)>,
    ) -> Measured<Placed<C>> {
        let width = MENU_WIDTH * description.scale;
        let scale = description.scale;
        let mut command_index = 0;
        let entries = menu_entries
            .iter()
            .copied()
            .map(|entry| match entry {
                Entry::Separator => separator(description.scale, width),
                Entry::Command(command) => {
                    let spec = spec(command);
                    let cursored = description.state.cursor() == Some(command_index);
                    command_index += 1;
                    item(
                        tcx,
                        styles,
                        command,
                        spec,
                        spec.toggle && description.toggles.checked(command),
                        description.availability.enabled(command),
                        cursored,
                        description.scale,
                        width,
                        select.clone(),
                    )
                }
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

    pub fn view<C: 'static>(
        tcx: &mut TextCtx,
        description: Description,
        hooks: Hooks<C>,
    ) -> View<Placed<C>> {
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
    use ui_events::keyboard::{KeyState, Modifiers, NamedKey};

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
            shortcut(&key("n", Modifiers::CONTROL)),
            Some(Command::App(AppCommand::New))
        );
        assert_eq!(
            shortcut(&key("N", Modifiers::CONTROL | Modifiers::SHIFT)),
            Some(Command::App(AppCommand::NewWindow))
        );
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
            ("5", Example::Torus),
            ("6", Example::Tanglecube),
            ("7", Example::Gyroid),
            ("8", Example::Cube),
        ] {
            assert_eq!(
                shortcut(&key(digit, Modifiers::CONTROL)),
                Some(Command::App(AppCommand::Example(example)))
            );
        }
    }

    fn named(key: NamedKey) -> KeyboardEvent {
        KeyboardEvent {
            key: Key::Named(key),
            state: KeyState::Down,
            ..Default::default()
        }
    }

    fn all_enabled() -> crate::command::Availability {
        crate::command::Availability {
            save: true,
            undo: true,
            redo: true,
            open_pane: true,
            move_up: true,
            move_down: true,
            move_left: true,
            move_right: true,
        }
    }

    #[test]
    fn arrows_walk_enabled_items_and_wrap() {
        let menus = definition();
        let mut availability = all_enabled();
        availability.undo = false;
        // Edit is [Undo, Redo]; with Undo disabled the cursor lands on
        // Redo from either direction and wraps in place.
        let mut state = State::default();
        let edit = menus
            .iter()
            .position(|menu| menu.label == "Edit")
            .expect("edit menu");
        state.toggle(edit);
        assert!(matches!(
            navigate(
                &mut state,
                &menus,
                availability,
                &named(NamedKey::ArrowDown)
            ),
            Navigation::Handled
        ));
        assert_eq!(state.cursor(), Some(1));
        assert!(matches!(
            navigate(
                &mut state,
                &menus,
                availability,
                &named(NamedKey::ArrowDown)
            ),
            Navigation::Handled
        ));
        assert_eq!(state.cursor(), Some(1));
        assert!(matches!(
            navigate(&mut state, &menus, availability, &named(NamedKey::Enter)),
            Navigation::Activate(Command::Doc(DocCommand::Redo))
        ));
    }

    #[test]
    fn horizontal_arrows_switch_menus_and_reset_the_cursor() {
        let menus = definition();
        let mut state = State::default();
        state.toggle(0);
        assert!(matches!(
            navigate(
                &mut state,
                &menus,
                all_enabled(),
                &named(NamedKey::ArrowDown)
            ),
            Navigation::Handled
        ));
        assert!(state.cursor().is_some());
        assert!(matches!(
            navigate(
                &mut state,
                &menus,
                all_enabled(),
                &named(NamedKey::ArrowRight)
            ),
            Navigation::Handled
        ));
        assert_eq!(state.open(), Some(1));
        assert_eq!(state.cursor(), None);
        assert!(matches!(
            navigate(
                &mut state,
                &menus,
                all_enabled(),
                &named(NamedKey::ArrowLeft)
            ),
            Navigation::Handled
        ));
        assert_eq!(state.open(), Some(0));
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
        let commands = commands(&definition).collect::<Vec<_>>();
        assert_eq!(commands.len(), 17 + Example::ALL.len());
        for (index, command) in commands.iter().enumerate() {
            assert!(commands[index + 1..].iter().all(|other| command != other));
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
                Entry::Command(Command::App(AppCommand::New)),
                Entry::Command(Command::App(AppCommand::NewWindow)),
                Entry::Command(Command::App(AppCommand::Open)),
                Entry::Separator,
                Entry::Command(Command::App(AppCommand::Close)),
                Entry::Command(Command::Doc(DocCommand::Save)),
                Entry::Command(Command::Doc(DocCommand::SaveAs)),
                Entry::Separator,
                Entry::Command(Command::App(AppCommand::Quit)),
            ]
        );
    }

    #[test]
    fn catalog_shortcuts_are_unique_across_the_drawn_tree() {
        let definition = definition();
        let shortcuts = commands(&definition)
            .filter_map(|command| command::spec(command).shortcut)
            .collect::<Vec<_>>();
        for (index, shortcut) in shortcuts.iter().enumerate() {
            assert!(shortcuts[index + 1..].iter().all(|other| shortcut != other));
        }
    }
}
