//! Raw structural projection as a total `Value → Layout`. The editor
//! supplies collapse, names, and pending state while building the
//! tree; `realize` is the only interpreter.

use super::{Cx, Hooks, select_handler};
use crate::hover::Hover;
use crate::identity::short_id;
use crate::selection::writable_at;
use crate::sources::DefinitionSource;
use gid::{CellId, Resolution, Step, Value, hex_string};
use progred_display::{
    CompletionKind, CompletionProvider, Delim, Face, Layout, activatable, alternatives,
    block_hover, bracket, col, completion, descend, dim, faced, hug, id, on_activate, on_click,
    on_hover, pickable, row, shared, slot,
};
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
        Value::Cell(cell) => cell_layout(cx, path, *cell),
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
        Value::Cell(cell) if cx.sources.values(*cell).next().is_some() => {
            let pending_inside = cx.sources.values(*cell).any(|value| {
                let mut followed = path.to_vec();
                followed.push(Step::Follow(value.source));
                cx.pending_child_of(&followed).is_some()
                    || cx.pending_edge_under(&followed).is_some()
            });
            (!pending_inside).then_some(Delim::Paren)?
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
        bracket(delim, toggle(dim("…"), path, hooks)),
        path,
        value,
        hooks,
        true,
    ))
}

fn cell_layout<World: 'static>(cx: &Cx, _path: &[Step], cell: CellId) -> View<World> {
    let definitions: Vec<_> = cx.sources.definitions(cell).collect();
    match definitions.as_slice() {
        [] => {
            return bracket(
                Delim::Paren,
                descend(Step::Follow(Resolution::Document), None, None),
            );
        }
        [
            crate::sources::LocatedDefinition {
                definition: progred_libraries::DefinitionRef::Value(_),
                source,
                ..
            },
        ] => {
            return bracket(Delim::Paren, descend(Step::Follow(*source), None, None));
        }
        [
            crate::sources::LocatedDefinition {
                definition: progred_libraries::DefinitionRef::ForeignFunction(_),
                ..
            },
        ] => return bracket(Delim::Paren, dim("foreign function")),
        _ => {}
    }
    bracket(
        Delim::Paren,
        col(
            0,
            4.0,
            definitions.into_iter().map(|resolved| {
                let source = match resolved.source {
                    DefinitionSource::Document => "document".to_string(),
                    DefinitionSource::Library(library) => cx
                        .sources
                        .library_name(library)
                        .map(|name| format!("library {name}"))
                        .unwrap_or_else(|| format!("library {}", short_id(library))),
                };
                let definition = match resolved.definition {
                    progred_libraries::DefinitionRef::Value(_) => {
                        descend(Step::Follow(resolved.source), None, None)
                    }
                    progred_libraries::DefinitionRef::ForeignFunction(_) => dim("foreign function"),
                };
                row(6.0, [dim(format!("{source}:")), definition])
            }),
        ),
    )
}

fn list_layout<World: 'static>(
    cx: &Cx,
    path: &[Step],
    elements: &gid::List,
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
        .map(|(position, _)| shared(descend(Step::Element(position.clone()), None, None)))
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
    fields: &gid::Record,
    hooks: &Hooks<World>,
) -> View<World> {
    let mut items: Vec<(CellId, bool)> = fields.iter().map(|(key, _)| (*key, true)).collect();
    if let Some(Step::Key(key)) = cx.pending_child_of(path) {
        items.push((key, false));
    }
    items.sort_by(
        |(left, _), (right, _)| match (cx.names(*left), cx.names(*right)) {
            (Some(left_name), Some(right_name)) => left_name.cmp(&right_name).then(left.cmp(right)),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => left.cmp(right),
        },
    );
    let pending_edge = cx.pending_edge_under(path).is_some();
    let pending_completions = path
        .is_empty()
        .then(|| cx.root_field_completions.cloned())
        .flatten();
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
        .map(|(key, _)| shared(descend(Step::Key(*key), None, None)))
        .collect();
    let mut flat = Vec::new();
    for (index, (key, present)) in items.iter().enumerate() {
        if index > 0 {
            flat.push(dim(", "));
        }
        flat.push(field_head(cx, path, *key, *present, hooks));
        flat.push(dim(" "));
        flat.push(children[index].clone());
    }
    if pending_edge {
        if !items.is_empty() {
            flat.push(dim(", "));
        }
        flat.push(pending_edge_layout(pending_completions.clone()));
    }
    let mut rows: Vec<View<World>> = items
        .iter()
        .zip(children)
        .map(|((key, present), child)| field_row(cx, path, *key, *present, child, hooks))
        .collect();
    if pending_edge {
        rows.push(pending_edge_layout(pending_completions));
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
    let label = field_label(cx, key);
    let head = row(0.0, [label, dim(":")]);
    if present {
        let mut child = path.to_vec();
        child.push(Step::Key(key));
        selectable(head, &child, &Value::from(key), hooks, true)
    } else {
        let mut child = path.to_vec();
        child.push(Step::Key(key));
        pickable(head, Hover::Value(Rc::from(child)), Value::Cell(key))
    }
}

fn field_label<World>(cx: &Cx, key: CellId) -> View<World> {
    let (spelling, face) = match cx.names(key) {
        Some(names) => (names, Face::Label),
        None => (short_id(key), Face::Id),
    };
    faced(spelling, face)
}

fn field_row<World: 'static>(
    cx: &Cx,
    path: &[Step],
    key: CellId,
    present: bool,
    child: View<World>,
    hooks: &Hooks<World>,
) -> View<World> {
    hug(field_head(cx, path, key, present, hooks), child, 6.0, 20.0)
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

fn insert<World: 'static>(
    child: View<World>,
    path: &[Step],
    after: gid::Position,
    hooks: &Hooks<World>,
) -> View<World> {
    let mut target = path.to_vec();
    target.push(Step::Element(after));
    let insert = hooks.insert.clone();
    let target: Rc<[Step]> = Rc::from(target);
    let handler_target = target.clone();
    activatable(
        child,
        Hover::Insert(target),
        Rc::new(move |world| {
            insert(world, handler_target.to_vec());
            true
        }),
    )
}

fn pending_edge_layout<World: 'static>(completions: Option<CompletionProvider>) -> View<World> {
    block_hover(on_click(
        row(
            0.0,
            [
                completion(CompletionKind::Field, completions),
                dim(": "),
                slot(),
            ],
        ),
        Rc::new(|_| true),
    ))
}
