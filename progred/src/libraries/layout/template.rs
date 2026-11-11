//! A template as its author reads and edits it. A template is layout data
//! drawn for some other record, so where it's defined it can't be drawn as
//! the layout it describes: its `descend`s would lead into the template
//! itself. Here each node reads as its kind and what it holds, a `descend` as
//! the name of the part it stands for, and an empty place offers what a
//! template is made of: the kinds of node, the parts of the records it draws,
//! and what's typed as text.

use super::vocabulary::{self as l, kind};
use crate::display::host::{Host, Layout, Partial, ProjectionInput};
use crate::display::structure::{field_name, kind_form};
use crate::display::{
    Completion, CompletionKind, CompletionRequest, CompletionText, Delim, Face, Picker, descend,
    dim, first, picker, ranked, row, runtime_partial, selectable, selectable_bracket,
};
use crate::libraries::completion::{insert, select};
use crate::selection::{at, payload, pending_at};
use ::grap::RuntimeValue;
use gid::{CellId, Step, Value};
use std::rc::Rc;

/// The nodes that arrange a list of `children`.
const CONTAINERS: [CellId; 4] = [kind::ROW, kind::COL, kind::OVERLAY, kind::ALTERNATIVES];

/// The nodes that hold one `child`.
const WRAPPERS: [CellId; 6] = [
    kind::CENTER,
    kind::PAD,
    kind::BORDER,
    kind::BRACKET,
    kind::SELECTABLE,
    kind::HOVER_BLOCK,
];

/// A template for records carrying `keys`, as its author reads and edits it.
/// Anything that isn't one of a template's nodes is left to the projections
/// around it, as a view that's a function is.
pub(crate) fn editor<W: Host>(keys: Rc<[CellId]>) -> Partial<W> {
    runtime_partial(move |input| node(input, &keys))
}

fn node<W: Host>(
    input: &ProjectionInput<'_, W, RuntimeValue>,
    keys: &Rc<[CellId]>,
) -> Option<Layout<W>> {
    let within = || first(input, editor(keys.clone()));
    let Some(value) = input.value else {
        let keys = keys.clone();
        return Some(picker(Picker::new(
            CompletionKind::Value,
            ranked(Rc::new(move |request| Some(offers(request, &keys)))),
        )));
    };
    // A template kept in a cell reads the same, in the cell's parentheses.
    if value.as_cell().is_some() {
        return input.env.resolve(value.as_cell()?).is_some().then(|| {
            selectable_bracket(Delim::Paren, descend([Step::Follow], Some(within()), None))
        });
    }
    let kind = value.kind()?;
    let label = || {
        selectable(
            match input.env.name(kind) {
                Some(name) => crate::display::named(&name, Face::Name),
                None => crate::display::faced(crate::identity::short_id(kind), Face::Id),
            },
            &input.site(),
        )
    };
    match kind {
        kind::DESCEND => kind_form(input, kind, [l::STEP], |[step]| {
            let Step::Key(part) = ::grap::path::read_step(step?.as_value())? else {
                return None;
            };
            Some(selectable(
                row(0.0, [dim("‹"), field_name(input.env, part), dim("›")]),
                &input.site(),
            ))
        }),
        _ if CONTAINERS.contains(&kind) => kind_form(input, kind, [l::CHILDREN], |_| {
            let children = crate::display::structure::list(Some(within()));
            Some(row(
                4.0,
                [
                    label(),
                    descend([Step::Key(l::CHILDREN)], Some(first(input, children)), None),
                ],
            ))
        }),
        _ if WRAPPERS.contains(&kind) => kind_form(input, kind, [l::CHILD], |_| {
            Some(row(
                6.0,
                [
                    label(),
                    descend([Step::Key(l::CHILD)], Some(within()), None),
                ],
            ))
        }),
        kind::TEXT => kind_form(input, kind, [l::CONTENT], |_| {
            Some(row(
                6.0,
                [label(), descend([Step::Key(l::CONTENT)], None, None)],
            ))
        }),
        kind::RULE | kind::SLOT => kind_form(input, kind, [], |[]| Some(label())),
        _ => None,
    }
}

/// The node standing for `part` of the record a template draws.
pub(crate) fn part(part: CellId) -> Value {
    ::grap::isa::record(
        kind::DESCEND,
        [(l::STEP, ::grap::path::step_value(&Step::Key(part)))],
    )
}

/// What an empty place in a template for records carrying `keys` offers.
fn offers(request: &CompletionRequest<'_>, keys: &[CellId]) -> Vec<Completion> {
    let node = |kind: CellId, fields: Vec<(CellId, Value)>| ::grap::isa::record(kind, fields);
    let holding = |kind: CellId, fields: Vec<(CellId, Value)>| {
        insert(
            kind,
            node(kind, fields),
            Some(pending_at(&[Step::Key(l::CHILD)])),
        )
    };
    let arranging = |kind: CellId| {
        let first = gid::position::between(None, None).expect("an empty list has room");
        insert(
            kind,
            node(kind, vec![(l::CHILDREN, Value::list([]))]),
            Some(pending_at(&[Step::Key(l::CHILDREN), Step::Element(first)])),
        )
    };
    let typed = request.query.trim();
    parts(request, keys)
        .into_iter()
        .map(|key| select(key, part(key)).with_detail("part"))
        .chain(CONTAINERS.map(arranging))
        .chain([
            holding(kind::CENTER, vec![]),
            holding(kind::PAD, vec![]),
            holding(kind::BORDER, vec![]),
            holding(kind::BRACKET, vec![(l::DELIM, Value::from(l::PAREN))]),
            select(kind::RULE, node(kind::RULE, vec![])),
            insert(
                kind::TEXT,
                node(
                    kind::TEXT,
                    vec![(l::CONTENT, crate::libraries::text::value(""))],
                ),
                Some(at(&[Step::Key(l::CONTENT)], payload::edge())),
            ),
        ])
        .map(|offer| match offer.detail {
            Some(_) => offer,
            None => offer.with_detail("layout"),
        })
        // What's typed, as the text it would draw.
        .chain((!typed.is_empty()).then(|| {
            select(
                CompletionText::Literal(format!("\"{typed}\"")),
                node(
                    kind::TEXT,
                    vec![(l::CONTENT, crate::libraries::text::value(typed))],
                ),
            )
        }))
        .collect()
}

/// The parts a template for records carrying `keys` can show: the keys
/// themselves, then the other fields of the document's records that carry
/// one, as far as this place can read the document.
fn parts(request: &CompletionRequest<'_>, keys: &[CellId]) -> Vec<CellId> {
    fn visit(value: &Value, keys: &[CellId], found: &mut Vec<CellId>) {
        match value {
            Value::Record(fields) => {
                if keys.iter().any(|key| fields.get(key).is_some()) {
                    for (field, _) in fields.iter() {
                        if *field != ::grap::isa::ISA && !found.contains(field) {
                            found.push(*field);
                        }
                    }
                }
                fields.values().for_each(|value| visit(value, keys, found));
            }
            Value::List(items) => items.values().for_each(|value| visit(value, keys, found)),
            _ => {}
        }
    }
    let mut found = keys.to_vec();
    if let Some(start) = (request.value_at)(&[]) {
        visit(start, keys, &mut found);
    }
    for cell in (request.cells)() {
        if let Some(definition) = (request.resolve)(cell).filter(|cell| cell.library.is_none()) {
            visit(definition.value, keys, &mut found);
        }
    }
    found
}
