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
        Entry::Command(C::App(A::Open)),
        Entry::Separator,
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
            entries: vec![
                Entry::Command(C::App(A::Example(E::Sample))),
                Entry::Command(C::App(A::Example(E::Grap))),
                Entry::Command(C::App(A::Example(E::IopTree))),
                Entry::Command(C::App(A::Example(E::Fidget))),
            ],
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
    use crate::command::{Availability, Command, DocCommand, Spec, spec};
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
        command: Command,
        spec: Spec,
        checked: bool,
        enabled: bool,
        scale: f64,
        width: f64,
        select: Rc<dyn Fn(&mut C, Command)>,
    ) -> Measured<Placed<C, Cv>> {
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
                Entry::Command(command) => {
                    let spec = spec(command);
                    item(
                        tcx,
                        styles,
                        command,
                        spec,
                        spec.toggle
                            && match command {
                                Command::Doc(DocCommand::Raw) => description.raw,
                                Command::Doc(DocCommand::DebugGeometry) => {
                                    description.debug_geometry
                                }
                                _ => false,
                            },
                        description.availability.enabled(command),
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
        let commands = commands(&definition).collect::<Vec<_>>();
        assert_eq!(commands.len(), 19);
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
                Entry::Command(Command::App(AppCommand::Open)),
                Entry::Separator,
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
