//! Raw structural projection as a total `Value → Layout`. The editor
//! supplies collapse, names, and pending state while building the
//! tree; `realize` is the only interpreter.

use super::{Cx, Hooks, select_handler};
use crate::hover::Hover;
use gid::{CellId, Resolution, Step, Value, hex_string};
use progred_display::{
    Delim, Layout, ProjectionInput, activatable, descend, dim, id, on_activate, on_hover, pickable,
    selectable_bracket,
};
use std::rc::Rc;

type View<World> = Layout<World, Hover>;

pub fn of<World: 'static>(
    cx: &Cx,
    path: &[Step],
    value: &Value,
    hooks: &Hooks<World>,
    input: &ProjectionInput<'_, World, Hover>,
) -> View<World> {
    match value {
        Value::Blob(bytes) => selectable(id(blob_text(bytes)), path, value, hooks, true),
        Value::Cell(cell) => cell_layout(cx, *cell),
        Value::List(_) => progred_display::structure::list_layout(input, None).unwrap(),
        Value::Record(_) => progred_display::structure::record_layout(input, |_| None).unwrap(),
    }
}

fn blob_text(bytes: &[u8]) -> String {
    if bytes.len() <= 16 {
        format!("0x{}", hex_string(bytes))
    } else {
        format!("0x{}… ({} bytes)", hex_string(&bytes[..8]), bytes.len())
    }
}

/// The editor-owned folded form shared by raw and custom projections.
/// Active structural editors keep their containing value open.
pub(super) fn collapsed_layout<World: 'static>(
    cx: &Cx,
    path: &[Step],
    value: &Value,
    hooks: &Hooks<World>,
) -> Option<View<World>> {
    let delim = match value {
        Value::Cell(cell) => {
            let value = cx.sources.resolve(*cell)?;
            let mut followed = path.to_vec();
            followed.push(Step::Follow(value.source));
            (cx.pending_child_of(&followed).is_none() && cx.pending_edge_under(&followed).is_none())
                .then_some(Delim::Paren)?
        }
        Value::List(elements) if !elements.is_empty() && cx.pending_child_of(path).is_none() => {
            Delim::Bracket
        }
        Value::Record(fields)
            if !fields.is_empty()
                && cx.pending_child_of(path).is_none()
                && cx.pending_edge_under(path).is_none() =>
        {
            Delim::Brace
        }
        _ => return None,
    };
    Some(selectable(
        selectable_bracket(delim, toggle(dim("…"), path, hooks)),
        path,
        value,
        hooks,
        true,
    ))
}

fn cell_layout<World: 'static>(cx: &Cx, cell: CellId) -> View<World> {
    let source = cx
        .sources
        .resolve(cell)
        .map_or(Resolution::Document, |value| value.source);
    selectable_bracket(Delim::Paren, descend(Step::Follow(source), None, None))
}

fn selectable<World: 'static>(
    child: View<World>,
    path: &[Step],
    value: &Value,
    hooks: &Hooks<World>,
    claim_hover: bool,
) -> View<World> {
    let path: Rc<[Step]> = Rc::from(path);
    let target = Hover::Value(path.clone());
    let clicked = on_activate(
        pickable(child, target.clone(), value.clone()),
        target,
        select_handler(path.clone(), hooks),
    );
    if claim_hover {
        on_hover(clicked, Hover::Value(path))
    } else {
        clicked
    }
}

fn toggle<World: 'static>(child: View<World>, path: &[Step], hooks: &Hooks<World>) -> View<World> {
    let target: Rc<[Step]> = Rc::from(path);
    let toggle = hooks.toggle.clone();
    activatable(
        child,
        Hover::Toggle(target.clone()),
        Rc::new(move |world| {
            toggle(world, target.to_vec());
            true
        }),
    )
}
