//! Structural projection as a total `Value → Layout`. The editor
//! supplies collapse, names, and pending state while building the
//! tree; `realize` is the only interpreter.

use super::Cx;
use crate::document::short_id;
use crate::selection::writable_at;
use progred_display::{
    Click, Delim, Layout, bracket, col, delim, descend, dim, group, head, hug, id, label, on_click,
    query, row, slot,
};
use progred_graph::{CellId, Step, Value, hex_string};
use std::collections::HashSet;

pub fn of(cx: &Cx, path: &[Step], ancestors: &HashSet<CellId>, value: &Value) -> Layout {
    match value {
        Value::Blob(bytes) => on_click(id(blob_text(bytes)), Click::Select),
        Value::Cell(cell) => cell_layout(cx, path, ancestors, *cell),
        Value::List(elements) => list_layout(cx, path, elements),
        Value::Record(fields) => record_layout(cx, path, fields),
    }
}

fn blob_text(bytes: &[u8]) -> String {
    if bytes.len() <= 16 {
        format!("0x{}", hex_string(bytes))
    } else {
        format!("0x{}… ({} bytes)", hex_string(&bytes[..8]), bytes.len())
    }
}

fn cell_layout(
    cx: &Cx,
    path: &[Step],
    ancestors: &HashSet<CellId>,
    cell: CellId,
) -> Layout {
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
        return on_click(
            row(
                4.0,
                [
                    delim(Delim::Paren, true),
                    on_click(dim("…"), Click::Toggle),
                    delim(Delim::Paren, false),
                ],
            ),
            Click::Select,
        );
    }
    let head = on_click(head(cell), Click::Select);
    let inner = match value {
        None if !cx.sources.writable(cell) => head,
        None | Some(_) => hug(head, descend(Step::Follow), 4.0, 20.0),
    };
    bracket(Delim::Paren, inner)
}

fn list_layout(
    cx: &Cx,
    path: &[Step],
    elements: &im::OrdMap<progred_graph::Position, Value>,
) -> Layout {
    let mut items: Vec<(progred_graph::Position, bool)> = elements
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
        return on_click(
            row(
                4.0,
                [
                    delim(Delim::Bracket, true),
                    on_click(dim("…"), Click::Toggle),
                    delim(Delim::Bracket, false),
                ],
            ),
            Click::Select,
        );
    }
    if items.is_empty() {
        return on_click(
            row(0.0, [delim(Delim::Bracket, true), delim(Delim::Bracket, false)]),
            Click::Quiet,
        );
    }
    let writable = writable_at(&cx.sources, path);
    let mut flat = vec![delim(Delim::Bracket, true)];
    for (index, (position, _)) in items.iter().enumerate() {
        if index > 0 {
            let separator = dim(", ");
            flat.push(if writable {
                on_click(
                    separator,
                    Click::Insert {
                        after: items[index - 1].0.clone(),
                    },
                )
            } else {
                separator
            });
        }
        flat.push(descend(Step::Element(position.clone())));
    }
    flat.push(delim(Delim::Bracket, false));
    let rows: Vec<Layout> = items
        .iter()
        .map(|(position, _)| descend(Step::Element(position.clone())))
        .collect();
    group(
        on_click(row(0.0, flat), Click::Quiet),
        bracket(Delim::Bracket, col(0, 4.0, rows)),
    )
}

fn record_layout(
    cx: &Cx,
    path: &[Step],
    fields: &im::OrdMap<CellId, Value>,
) -> Layout {
    let consumes_simple_name = !cx.raw
        && path
            .split_last()
            .filter(|(step, _)| matches!(step, Step::Follow))
            .and_then(|(_, parent)| cx.sources.resolve(parent))
            .and_then(Value::as_cell)
            .and_then(|cell| cx.name(cell))
            .is_some()
        && fields
            .get(&progred_name::vocabulary::NAME)
            .and_then(progred_text::read)
            .is_some_and(|name| !name.is_empty());
    let mut items: Vec<(CellId, bool)> = fields
        .iter()
        .filter(|(key, _)| !consumes_simple_name || **key != progred_name::vocabulary::NAME)
        .map(|(key, _)| (*key, true))
        .collect();
    if let Some(Step::Key(key)) = cx.pending_child_of(path) {
        items.push((key, false));
    }
    items.sort_by(|(left, _), (right, _)| match (cx.name(*left), cx.name(*right)) {
        (Some(left_name), Some(right_name)) => left_name.cmp(&right_name).then(left.cmp(right)),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => left.cmp(right),
    });
    let pending_edge = cx.pending_edge_under(path).is_some();
    let renaming = cx.pending_rename_under(path);
    let collapsed = !items.is_empty()
        && !pending_edge
        && renaming.is_none()
        && items.iter().all(|(_, present)| *present)
        && cx.collapse.collapsed(path, false);
    if collapsed {
        return on_click(
            row(
                4.0,
                [
                    delim(Delim::Brace, true),
                    on_click(dim("…"), Click::Toggle),
                    delim(Delim::Brace, false),
                ],
            ),
            Click::Select,
        );
    }
    if items.is_empty() && !pending_edge {
        return on_click(
            row(0.0, [delim(Delim::Brace, true), delim(Delim::Brace, false)]),
            Click::Quiet,
        );
    }
    let mut flat = vec![delim(Delim::Brace, true)];
    for (index, (key, _)) in items.iter().enumerate() {
        if index > 0 {
            flat.push(dim(", "));
        }
        let name = match cx.pending_rename_under(path) {
            Some((replacing, _, _)) if replacing == key => query(true),
            _ => field_label(cx, path, *key),
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
    let mut rows: Vec<Layout> = items
        .iter()
        .map(|(key, present)| field_row(cx, path, *key, *present))
        .collect();
    if pending_edge {
        rows.push(pending_edge_layout());
    }
    group(
        on_click(row(0.0, flat), Click::Quiet),
        bracket(Delim::Brace, col(0, 4.0, rows)),
    )
}

fn field_head(cx: &Cx, path: &[Step], key: CellId, present: bool) -> Layout {
    let label = match cx.pending_rename_under(path) {
        Some((replacing, _, _)) if replacing == &key => query(true),
        _ => field_label(cx, path, key),
    };
    let head = row(0.0, [label, dim(":")]);
    if present {
        on_click(head, Click::Field { key })
    } else {
        on_click(head, Click::Pick { key })
    }
}

fn field_row(cx: &Cx, path: &[Step], key: CellId, present: bool) -> Layout {
    hug(field_head(cx, path, key, present), descend(Step::Key(key)), 6.0, 20.0)
}

fn field_label(cx: &Cx, path: &[Step], key: CellId) -> Layout {
    let shown = match cx.name(key) {
        Some(name) => label(name),
        None => id(short_id(key)),
    };
    if writable_at(&cx.sources, path) {
        on_click(shown, Click::Rename { key })
    } else {
        shown
    }
}

fn pending_edge_layout() -> Layout {
    on_click(
        row(0.0, [query(true), dim(": "), slot()]),
        Click::Absorb,
    )
}
