//! Raw structural projection as a total `Value → Layout`. The editor
//! supplies collapse, names, and pending state while building the
//! tree; `realize` is the only interpreter.

use super::{Cx, Hooks, select_handler};
use crate::hover::Hover;
use crate::selection::writable_at;
use gid::{CellId, Step, Value, hex_string};
use progred_display::{
    Delim, Layout, block_hover, bracket, col, delim, descend, dim, field_label, group, head, hug,
    id, on_click, on_hover, pickable, query, row, slot,
};
use progred_libraries::{name, text};
use std::collections::HashSet;
use std::rc::Rc;

type View<World> = Layout<World, Hover>;

pub fn of<World: 'static>(
    cx: &Cx,
    path: &[Step],
    ancestors: &HashSet<CellId>,
    value: &Value,
    hooks: &Hooks<World>,
) -> View<World> {
    match value {
        Value::Blob(bytes) => selectable(id(blob_text(bytes)), path, value, hooks, true),
        Value::Cell(cell) => cell_layout(cx, path, ancestors, *cell, hooks),
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

fn cell_layout<World: 'static>(
    cx: &Cx,
    path: &[Step],
    ancestors: &HashSet<CellId>,
    cell: CellId,
    hooks: &Hooks<World>,
) -> View<World> {
    let mut followed = path.to_vec();
    followed.push(Step::Follow);
    let value = cx.sources.value(cell);
    let pending_inside = cx.pending_child_of(&followed).is_some()
        || cx.pending_edge_under(&followed).is_some()
        || cx.pending_rename_under(&followed).is_some();
    let elided = value.is_some()
        && !pending_inside
        && cx.collapse.collapsed(path, ancestors.contains(&cell));
    if elided {
        return selectable(
            row(
                4.0,
                [
                    delim(Delim::Paren, true),
                    toggle(dim("…"), path, hooks),
                    delim(Delim::Paren, false),
                ],
            ),
            path,
            &Value::from(cell),
            hooks,
            true,
        );
    }
    let head = selectable(head(cell), path, &Value::from(cell), hooks, true);
    let inner = match value {
        None if !cx.sources.writable(cell) => head,
        None | Some(_) => hug(head, descend(Step::Follow), 4.0, 20.0),
    };
    bracket(Delim::Paren, inner)
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
    let collapsed = !items.is_empty()
        && items.iter().all(|(_, present)| *present)
        && cx.collapse.collapsed(path, false);
    if collapsed {
        return selectable(
            row(
                4.0,
                [
                    delim(Delim::Bracket, true),
                    toggle(dim("…"), path, hooks),
                    delim(Delim::Bracket, false),
                ],
            ),
            path,
            &Value::List(elements.clone()),
            hooks,
            true,
        );
    }
    if items.is_empty() {
        return selectable(
            row(
                0.0,
                [delim(Delim::Bracket, true), delim(Delim::Bracket, false)],
            ),
            path,
            &Value::List(elements.clone()),
            hooks,
            false,
        );
    }
    let writable = writable_at(&cx.sources, path);
    let mut flat = vec![delim(Delim::Bracket, true)];
    for (index, (position, _)) in items.iter().enumerate() {
        if index > 0 {
            let separator = dim(", ");
            flat.push(if writable {
                insert(separator, path, items[index - 1].0.clone(), hooks)
            } else {
                separator
            });
        }
        flat.push(descend(Step::Element(position.clone())));
    }
    flat.push(delim(Delim::Bracket, false));
    let rows: Vec<View<World>> = items
        .iter()
        .map(|(position, _)| descend(Step::Element(position.clone())))
        .collect();
    group(
        selectable(
            row(0.0, flat),
            path,
            &Value::List(elements.clone()),
            hooks,
            false,
        ),
        bracket(Delim::Bracket, col(0, 4.0, rows)),
    )
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
    let renaming = cx.pending_rename_under(path);
    let collapsed = !items.is_empty()
        && !pending_edge
        && renaming.is_none()
        && items.iter().all(|(_, present)| *present)
        && cx.collapse.collapsed(path, false);
    if collapsed {
        return selectable(
            row(
                4.0,
                [
                    delim(Delim::Brace, true),
                    toggle(dim("…"), path, hooks),
                    delim(Delim::Brace, false),
                ],
            ),
            path,
            &Value::Record(fields.clone()),
            hooks,
            true,
        );
    }
    if items.is_empty() && !pending_edge {
        return selectable(
            row(0.0, [delim(Delim::Brace, true), delim(Delim::Brace, false)]),
            path,
            &Value::Record(fields.clone()),
            hooks,
            false,
        );
    }
    let mut flat = vec![delim(Delim::Brace, true)];
    for (index, (key, _)) in items.iter().enumerate() {
        if index > 0 {
            flat.push(dim(", "));
        }
        let name = match cx.pending_rename_under(path) {
            Some((replacing, _, _)) if replacing == key => query(true),
            _ => field_label(*key),
        };
        flat.push(name);
        flat.push(dim(": "));
        flat.push(descend(Step::Key(*key)));
    }
    if pending_edge {
        if !items.is_empty() {
            flat.push(dim(", "));
        }
        flat.push(pending_edge_layout());
    }
    flat.push(delim(Delim::Brace, false));
    let mut rows: Vec<View<World>> = items
        .iter()
        .map(|(key, present)| field_row(cx, path, *key, *present, hooks))
        .collect();
    if pending_edge {
        rows.push(pending_edge_layout());
    }
    group(
        selectable(
            row(0.0, flat),
            path,
            &Value::Record(fields.clone()),
            hooks,
            false,
        ),
        bracket(Delim::Brace, col(0, 4.0, rows)),
    )
}

fn field_head<World: 'static>(
    cx: &Cx,
    path: &[Step],
    key: CellId,
    present: bool,
    hooks: &Hooks<World>,
) -> View<World> {
    let label = match cx.pending_rename_under(path) {
        Some((replacing, _, _)) if replacing == &key => query(true),
        _ => field_label(key),
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

fn field_row<World: 'static>(
    cx: &Cx,
    path: &[Step],
    key: CellId,
    present: bool,
    hooks: &Hooks<World>,
) -> View<World> {
    hug(
        field_head(cx, path, key, present, hooks),
        descend(Step::Key(key)),
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
        row(0.0, [query(true), dim(": "), slot()]),
        Rc::new(|_| true),
    ))
}
