//! Raw structural projection as a total `Value → Layout`. The editor
//! supplies collapse, names, and pending state while building the
//! tree; `realize` is the only interpreter.

use super::{Cx, select_handler};
use crate::display::{
    Delim, Layout, ProjectionInput, activatable, bracket, descend, dim, id, on_activate, on_hover,
    pickable_runtime, row, selectable_bracket,
};
use crate::frame::Hovered;
use crate::hover::Hover;
use crate::libraries::name;
use gid::{CellId, Resolution, Step, hex_string};
use std::rc::Rc;

type View = Layout<crate::Editor, Hovered>;

pub fn of(
    cx: &Cx,
    path: &[Step],
    value: &grap::RuntimeValue,
    input: &ProjectionInput<'_, crate::Editor, Hovered, grap::RuntimeValue>,
) -> View {
    match value.shape() {
        grap::Shape::Blob(bytes) => selectable(cx, id(blob_text(bytes)), path, value),
        grap::Shape::Cell(cell) => cell_layout(cx, cell),
        grap::Shape::List(positions) => {
            crate::display::structure::list_layout_at(input, positions, None)
        }
        // A named record reads as its name, then its other fields.
        grap::Shape::Record(keys) if !cx.raw && name::named(value) => name::with_name(
            input,
            crate::display::structure::record_layout_at(
                input,
                keys.into_iter()
                    .filter(|key| *key != name::vocabulary::NAME)
                    .collect(),
                |_| None,
            ),
        ),
        grap::Shape::Record(keys) => {
            crate::display::structure::record_layout_at(input, keys, |_| None)
        }
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
/// Active structural editors keep their containing value open. A named
/// value keeps its name in front of what it hides.
pub(super) fn collapsed_layout(
    cx: &Cx,
    path: &[Step],
    value: &grap::RuntimeValue,
    default: bool,
) -> Option<View> {
    let (delim, label) = match value.as_cell() {
        Some(cell) => {
            let source = cx.sources.resolve(cell)?.source;
            let mut followed = path.to_vec();
            followed.push(Step::Follow(source));
            (cx.pending_child_of(&followed).is_none()
                && cx.pending_edge_under(&followed).is_none())
            .then_some(())?;
            (
                Delim::Paren,
                cx.name(cell)
                    .is_some()
                    .then(|| vec![Step::Follow(source), Step::Key(name::vocabulary::NAME)]),
            )
        }
        None if value.list_len().is_some_and(|len| len != 0)
            && cx.pending_child_of(path).is_none() =>
        {
            (Delim::Bracket, None)
        }
        None if value.record_len().is_some_and(|len| len != 0)
            && cx.pending_child_of(path).is_none()
            && cx.pending_edge_under(path).is_none() =>
        {
            (
                Delim::Brace,
                (!cx.raw && name::named(value)).then(|| vec![Step::Key(name::vocabulary::NAME)]),
            )
        }
        _ => return None,
    };
    let folded = toggle(dim("…"), path, cx, default);
    Some(match (delim, label) {
        // The cell stays a cell around the named record it hides.
        (Delim::Paren, Some(label)) => selectable(
            cx,
            selectable_bracket(
                Delim::Paren,
                row(6.0, [name::label(label), bracket(Delim::Brace, folded)]),
            ),
            path,
            value,
        ),
        (delim, Some(label)) => row(
            6.0,
            [
                name::label(label),
                selectable(cx, selectable_bracket(delim, folded), path, value),
            ],
        ),
        (delim, None) => selectable(cx, selectable_bracket(delim, folded), path, value),
    })
}

fn cell_layout(cx: &Cx, cell: CellId) -> View {
    let source = cx
        .sources
        .resolve(cell)
        .map_or(Resolution::Document, |value| value.source);
    crate::display::projection::group(selectable_bracket(
        Delim::Paren,
        descend(Step::Follow(source), None, None),
    ))
}

fn selectable(cx: &Cx, child: View, path: &[Step], value: &grap::RuntimeValue) -> View {
    let path: Rc<[Step]> = Rc::from(path);
    let target = Hovered::Tree(Hover::Value(path.clone()));
    let child = crate::display::projection::target(child);
    let clicked = on_activate(
        pickable_runtime(child, target.clone(), value.clone()),
        target,
        select_handler(path.clone(), cx),
    );
    on_hover(clicked, Hovered::Tree(Hover::Value(path)))
}

fn toggle(child: View, path: &[Step], cx: &Cx, default: bool) -> View {
    let target: Rc<[Step]> = Rc::from(path);
    let root = cx.view.clone();
    activatable(
        crate::display::hover_highlight(child, Hovered::Tree(Hover::Toggle(target.clone()))),
        Hovered::Tree(Hover::Toggle(target.clone())),
        Rc::new(move |world| {
            world.set_collapsed(&root, &target, default, None);
            true
        }),
    )
}
