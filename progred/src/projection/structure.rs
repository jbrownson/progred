//! Raw structural projection as a total `Value → Layout`. The editor
//! supplies collapse, names, and pending state while building the
//! tree; `realize` is the only interpreter.

use super::{Cx, select_handler};
use crate::display::{
    Delim, Layout, ProjectionInput, activatable, descend, dim, id, on_activate, on_hover, pickable,
    selectable_bracket,
};
use crate::frame::Hovered;
use crate::hover::Hover;
use gid::{CellId, Resolution, Step, Value, hex_string};
use std::rc::Rc;

type View = Layout<crate::Editor, Hovered>;

pub fn of(
    cx: &Cx,
    path: &[Step],
    value: &Value,
    input: &ProjectionInput<'_, crate::Editor, Hovered>,
) -> View {
    match value {
        Value::Blob(bytes) => selectable(cx, id(blob_text(bytes)), path, value, true),
        Value::Cell(cell) => cell_layout(cx, *cell),
        Value::List(_) => crate::display::structure::list_layout(input, None).unwrap(),
        Value::Record(_) => crate::display::structure::record_layout(input, |_| None).unwrap(),
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
pub(super) fn collapsed_layout(cx: &Cx, path: &[Step], value: &Value) -> Option<View> {
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
        cx,
        selectable_bracket(delim, toggle(dim("…"), path, cx)),
        path,
        value,
        true,
    ))
}

fn cell_layout(cx: &Cx, cell: CellId) -> View {
    let source = cx
        .sources
        .resolve(cell)
        .map_or(Resolution::Document, |value| value.source);
    selectable_bracket(Delim::Paren, descend(Step::Follow(source), None, None))
}

fn selectable(cx: &Cx, child: View, path: &[Step], value: &Value, claim_hover: bool) -> View {
    let path: Rc<[Step]> = Rc::from(path);
    let target = Hovered::Tree(Hover::Value(path.clone()));
    let clicked = on_activate(
        pickable(child, target.clone(), value.clone()),
        target,
        select_handler(path.clone(), cx),
    );
    if claim_hover {
        on_hover(clicked, Hovered::Tree(Hover::Value(path)))
    } else {
        clicked
    }
}

fn toggle(child: View, path: &[Step], cx: &Cx) -> View {
    let target: Rc<[Step]> = Rc::from(path);
    let root = cx.view.clone();
    let writable = !cx.source.transient();
    activatable(
        crate::display::hover_highlight(child, Hovered::Tree(Hover::Toggle(target.clone()))),
        Hovered::Tree(Hover::Toggle(target.clone())),
        Rc::new(move |world| {
            if writable {
                world.collapse(&root, &target, None);
            }
            true
        }),
    )
}
