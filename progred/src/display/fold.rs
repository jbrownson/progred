//! Folding, which a projection offers for what it draws: the occurrence shut,
//! as its brackets around an ellipsis, while its state says so. The state is
//! the occurrence's own, kept with its view; how it starts is the
//! projection's call.

use crate::display::host::{Host, Layout, Partial};
use crate::display::{Delim, Viewing, dim, on_activate, on_hover, row, selectable_bracket};
use gid::{CellId, Value};
use puri::handler::HasHandler;
use std::rc::Rc;
use ui_events::keyboard::{Key, KeyboardEvent, NamedKey};

/// Whether a cell's fold starts closed at the occurrence `viewing` reads,
/// `value` being the cell's own. It does where drawing it would draw that
/// value again inside itself, and where it sits inside another cell's value
/// and holds cells itself, which could each open more in turn. The cells a
/// view reaches directly open, so their projections get going, and so do
/// cells holding none, which can't cascade.
pub fn cell_starts_closed(viewing: &dyn Viewing, cell: CellId, value: &Value) -> bool {
    viewing.inside(cell) || viewing.inside_a_cell() && holds_cells(value)
}

/// Whether a cell sits anywhere in `value`.
fn holds_cells(value: &Value) -> bool {
    match value {
        Value::Cell(_) => true,
        Value::Blob(_) => false,
        Value::List(list) => list.values().any(holds_cells),
        Value::Record(record) => record.values().any(holds_cells),
    }
}

/// `open`, the occurrence drawn open, or while its fold is closed, `delim`
/// around `name` and an ellipsis that opens it. `starts_closed` says which
/// until it's toggled. It stays open around a selection inside it and while a
/// field is being added to it. Selected, Space toggles it, and the command
/// modifier with Up closes it and with Down opens it.
pub fn foldable<W: Host>(
    delim: Delim,
    name: Option<Layout<W>>,
    starts_closed: impl Fn(&dyn Viewing) -> bool + 'static,
    open: Layout<W>,
) -> Layout<W> {
    Layout::program(Rc::new(move |context, build| {
        let toolkit = context.toolkit;
        let default = starts_closed(context.viewing());
        let path = toolkit.path();
        let held = toolkit.label_query().is_some()
            || toolkit.selection().is_some_and(|selection| {
                selection.path().len() > path.len() && path.leads(selection.path())
            });
        let layout = match context.value {
            Some(_) if !held && crate::annotations::folded(toolkit.state_below(&[]), default) => {
                closed(delim, name.clone(), context.site(), default)
            }
            _ => open.clone(),
        };
        let layout = if toolkit.selected() {
            fold_keys(layout, context.site(), default)
        } else {
            layout
        };
        layout.measure(context, build)
    }))
}

/// The closed form: `delim` around `name` and an ellipsis, which opens the
/// fold, selecting the occurrence first as a click on its brackets would, so
/// Space closes it again.
fn closed<W: Host>(
    delim: Delim,
    name: Option<Layout<W>>,
    site: crate::editing::Site,
    default: bool,
) -> Layout<W> {
    let hover = W::Hover::from(site.toggle_hover());
    let toggle = {
        let site = site.clone();
        on_hover(
            on_activate(
                crate::display::hover_highlight(dim("…"), hover.clone()),
                hover.clone(),
                Rc::new(move |session| {
                    let mut edit = site.open(session);
                    edit.select();
                    edit.fold(default, None);
                    true
                }),
            ),
            hover,
        )
    };
    let content = match name {
        Some(name) => row(4.0, [name, toggle]),
        None => toggle,
    };
    crate::display::activatable(selectable_bracket(delim, content), &site)
}

/// `child` with the fold keys, acting while the occurrence at `site` is
/// selected as a whole.
fn fold_keys<W: Host>(child: Layout<W>, site: crate::editing::Site, default: bool) -> Layout<W> {
    crate::display::widget::around(
        child,
        Rc::new(move |_| {
            let site = site.clone();
            Box::new(move |child| {
                crate::display::widget::before_place(child, move |_, output| {
                    let command = output.input.command_modifier;
                    output
                        .handler()
                        .on_key_with(move |world: &mut W, event, _| {
                            let Some(closed) =
                                event.state.is_down().then(|| key(command, event)).flatten()
                            else {
                                return false;
                            };
                            let mut session = world.session();
                            let whole = session.model.selection.as_ref().is_some_and(|selection| {
                                site.holds(selection)
                                    && selection.stage(&session.sources())
                                        == crate::selection::Stage::Edge
                            });
                            whole && site.open(&mut session).fold(default, closed)
                        });
                })
            })
        }),
    )
}

/// The fold a key asks for: Space toggles it, and the command modifier with
/// Up closes it and with Down opens it.
fn key(command: puri::keyboard::CommandModifier, event: &KeyboardEvent) -> Option<Option<bool>> {
    match &event.key {
        Key::Character(key) if key.as_str() == " " => Some(None),
        Key::Named(NamedKey::ArrowUp) if command.pressed(&event.modifiers) => Some(Some(true)),
        Key::Named(NamedKey::ArrowDown) if command.pressed(&event.modifiers) => Some(Some(false)),
        _ => None,
    }
}

/// `partial`, folding what it draws between braces, as the record it is,
/// starting open.
pub fn folding<W: Host>(partial: Partial<W>) -> Partial<W> {
    crate::display::runtime_partial(move |input| {
        partial(input).map(|open| foldable(Delim::Brace, None, |_| false, open))
    })
}
