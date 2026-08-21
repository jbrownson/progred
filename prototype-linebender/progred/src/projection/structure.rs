//! Raw structural projection as a total `Value → Layout`. The editor
//! supplies collapse, names, and pending state while building the
//! tree; `realize` is the only interpreter.

use super::{Cx, Hooks, select_handler};
use crate::hover::Hover;
use crate::selection::writable_at;
use gid::{CellId, Step, Value, hex_string};
use crate::identity::short_id;
use progred_display::{
    Delim, Face, Layout, alternatives, at, block_hover, bracket, col, descend, dim, faced, hug, id,
    on_click, on_hover, pickable, query, row, shared, slot,
};
use progred_libraries::{name, text};
use std::rc::Rc;

type View<World> = Layout<World, Hover>;

pub fn of<World: 'static>(
    cx: &Cx,
    path: &[Step],
    value: &Value,
    hooks: &Hooks<World>,
) -> View<World> {
    match value {
        Value::Blob(bytes) => selectable(id(blob_text(bytes)), path, value, hooks, true),
        Value::Cell(cell) => cell_layout(cx, path, *cell, hooks),
        Value::List(elements) => list_layout(cx, path, elements, hooks),
        Value::Record(fields) => record_layout(cx, path, fields, hooks),
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
        Value::Cell(cell) if cx.sources.value(*cell).is_some() => {
            let mut followed = path.to_vec();
            followed.push(Step::Follow);
            let pending_inside = cx.pending_child_of(&followed).is_some()
                || cx.pending_edge_under(&followed).is_some()
                || cx.pending_rename_under(&followed).is_some();
            (!pending_inside).then_some(Delim::Paren)?
        }
        Value::List(elements)
            if !elements.is_empty() && cx.pending_child_of(path).is_none() =>
        {
            Delim::Bracket
        }
        Value::Record(fields)
            if !fields.is_empty()
                && cx.pending_child_of(path).is_none()
                && cx.pending_edge_under(path).is_none()
                && cx.pending_rename_under(path).is_none() =>
        {
            Delim::Brace
        }
        _ => return None,
    };
    Some(selectable(
        bracket(delim, toggle(dim("…"), path, hooks)),
        path,
        value,
        hooks,
        true,
    ))
}

fn cell_layout<World: 'static>(
    cx: &Cx,
    path: &[Step],
    cell: CellId,
    hooks: &Hooks<World>,
) -> View<World> {
    let value = cx.sources.value(cell);
    let head = selectable(cell_head(cx, cell), path, &Value::from(cell), hooks, true);
    let inner = match value {
        None if !cx.sources.writable(cell) => head,
        None | Some(_) => hug(head, descend(Step::Follow), 4.0, 20.0),
    };
    bracket(Delim::Paren, inner)
}

fn cell_head<World>(cx: &Cx, cell: CellId) -> View<World> {
    match cx
        .sources
        .value(cell)
        .and_then(Value::as_record)
        .and_then(|fields| fields.get(&name::vocabulary::NAME))
    {
        Some(name) => at(
            [Step::Follow, Step::Key(name::vocabulary::NAME)],
            name,
        ),
        None => id(short_id(cell)),
    }
}

fn list_layout<World: 'static>(
    cx: &Cx,
    path: &[Step],
    elements: &im::OrdMap<gid::Position, Value>,
    hooks: &Hooks<World>,
) -> View<World> {
    let mut items: Vec<(gid::Position, bool)> = elements
        .iter()
        .map(|(position, _)| (position.clone(), true))
        .collect();
    if let Some(Step::Element(position)) = cx.pending_child_of(path) {
        items.push((position, false));
        items.sort_by(|a, b| a.0.cmp(&b.0));
    }
    if items.is_empty() {
        return selectable(
            bracket(Delim::Bracket, row(0.0, Vec::new())),
            path,
            &Value::List(elements.clone()),
            hooks,
            false,
        );
    }
    let writable = writable_at(&cx.sources, path);
    let children: Vec<View<World>> = items
        .iter()
        .map(|(position, _)| shared(descend(Step::Element(position.clone()))))
        .collect();
    let mut flat = Vec::new();
    for (index, _) in items.iter().enumerate() {
        if index > 0 {
            let separator = dim(", ");
            flat.push(if writable {
                insert(separator, path, items[index - 1].0.clone(), hooks)
            } else {
                separator
            });
        }
        flat.push(children[index].clone());
    }
    alternatives([
        selectable(
            bracket(Delim::Bracket, row(0.0, flat)),
            path,
            &Value::List(elements.clone()),
            hooks,
            false,
        ),
        bracket(Delim::Bracket, col(0, 4.0, children)),
    ])
}

fn record_layout<World: 'static>(
    cx: &Cx,
    path: &[Step],
    fields: &im::OrdMap<CellId, Value>,
    hooks: &Hooks<World>,
) -> View<World> {
    let consumes_simple_name = !cx.raw
        && path
            .split_last()
            .filter(|(step, _)| matches!(step, Step::Follow))
            .and_then(|(_, parent)| cx.sources.resolve(parent))
            .and_then(Value::as_cell)
            .and_then(|cell| cx.name(cell))
            .is_some()
        && fields
            .get(&name::vocabulary::NAME)
            .and_then(text::read)
            .is_some_and(|name| !name.is_empty());
    let mut items: Vec<(CellId, bool)> = fields
        .iter()
        .filter(|(key, _)| !consumes_simple_name || **key != name::vocabulary::NAME)
        .map(|(key, _)| (*key, true))
        .collect();
    if let Some(Step::Key(key)) = cx.pending_child_of(path) {
        items.push((key, false));
    }
    items.sort_by(
        |(left, _), (right, _)| match (cx.name(*left), cx.name(*right)) {
            (Some(left_name), Some(right_name)) => left_name.cmp(&right_name).then(left.cmp(right)),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => left.cmp(right),
        },
    );
    let pending_edge = cx.pending_edge_under(path).is_some();
    if items.is_empty() && !pending_edge {
        return selectable(
            bracket(Delim::Brace, row(0.0, Vec::new())),
            path,
            &Value::Record(fields.clone()),
            hooks,
            false,
        );
    }
    let children: Vec<View<World>> = items
        .iter()
        .map(|(key, _)| shared(descend(Step::Key(*key))))
        .collect();
    let mut flat = Vec::new();
    for (index, (key, _)) in items.iter().enumerate() {
        if index > 0 {
            flat.push(dim(", "));
        }
        let name = match cx.pending_rename_under(path) {
            Some((replacing, _, _)) if replacing == *key => query(),
            _ => field_label(cx, path, *key, hooks),
        };
        flat.push(name);
        flat.push(dim(": "));
        flat.push(children[index].clone());
    }
    if pending_edge {
        if !items.is_empty() {
            flat.push(dim(", "));
        }
        flat.push(pending_edge_layout());
    }
    let mut rows: Vec<View<World>> = items
        .iter()
        .zip(children)
        .map(|((key, present), child)| field_row(cx, path, *key, *present, child, hooks))
        .collect();
    if pending_edge {
        rows.push(pending_edge_layout());
    }
    alternatives([
        selectable(
            bracket(Delim::Brace, row(0.0, flat)),
            path,
            &Value::Record(fields.clone()),
            hooks,
            false,
        ),
        bracket(Delim::Brace, col(0, 4.0, rows)),
    ])
}

fn field_head<World: 'static>(
    cx: &Cx,
    path: &[Step],
    key: CellId,
    present: bool,
    hooks: &Hooks<World>,
) -> View<World> {
    let label = match cx.pending_rename_under(path) {
        Some((replacing, _, _)) if replacing == key => query(),
        _ => field_label(cx, path, key, hooks),
    };
    let head = row(0.0, [label, dim(":")]);
    if present {
        let mut child = path.to_vec();
        child.push(Step::Key(key));
        selectable(head, &child, &Value::from(key), hooks, true)
    } else {
        pickable(head, Value::Cell(key))
    }
}

fn field_label<World: 'static>(
    cx: &Cx,
    path: &[Step],
    key: CellId,
    hooks: &Hooks<World>,
) -> View<World> {
    let (spelling, face) = match cx.name(key) {
        Some(name) => (name.to_string(), Face::Label),
        None => (short_id(key), Face::Id),
    };
    let label = faced(spelling.clone(), face);
    if !writable_at(&cx.sources, path) || cx.source.transient() {
        label
    } else {
        let mut target = path.to_vec();
        target.push(Step::Key(key));
        let handler_target = target.clone();
        let rename = hooks.rename.clone();
        let caret = spelling.len();
        on_hover(
            on_click(
                label,
                Rc::new(move |world| {
                    rename(world, handler_target.clone(), caret);
                    true
                }),
            ),
            Hover::Label(target),
        )
    }
}

fn field_row<World: 'static>(
    cx: &Cx,
    path: &[Step],
    key: CellId,
    present: bool,
    child: View<World>,
    hooks: &Hooks<World>,
) -> View<World> {
    hug(
        field_head(cx, path, key, present, hooks),
        child,
        6.0,
        20.0,
    )
}

fn selectable<World: 'static>(
    child: View<World>,
    path: &[Step],
    value: &Value,
    hooks: &Hooks<World>,
    claim_hover: bool,
) -> View<World> {
    let path = path.to_vec();
    let clicked = on_click(
        pickable(child, value.clone()),
        select_handler(path.clone(), hooks),
    );
    if claim_hover {
        on_hover(clicked, Hover::Value(path))
    } else {
        clicked
    }
}

fn toggle<World: 'static>(child: View<World>, path: &[Step], hooks: &Hooks<World>) -> View<World> {
    let target = path.to_vec();
    let toggle = hooks.toggle.clone();
    on_hover(
        on_click(
            child,
            Rc::new(move |world| {
                toggle(world, target.clone());
                true
            }),
        ),
        Hover::Toggle(path.to_vec()),
    )
}

fn insert<World: 'static>(
    child: View<World>,
    path: &[Step],
    after: gid::Position,
    hooks: &Hooks<World>,
) -> View<World> {
    let mut target = path.to_vec();
    target.push(Step::Element(after));
    let insert = hooks.insert.clone();
    let handler_target = target.clone();
    on_hover(
        on_click(
            child,
            Rc::new(move |world| {
                insert(world, handler_target.clone());
                true
            }),
        ),
        Hover::Insert(target),
    )
}

fn pending_edge_layout<World: 'static>() -> View<World> {
    block_hover(on_click(
        row(0.0, [query(), dim(": "), slot()]),
        Rc::new(|_| true),
    ))
}
