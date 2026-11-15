//! Defs: what a kind is. A record says its kind with `isa`; the kind's cell is
//! defined by a def, `{isa: def, name, fields, projection}`, which says what
//! such a record has and how it's drawn, connecting what's known about a kind
//! to the kind itself. A record is drawn by its kind's def's projection
//! before anything else draws it; where that declines, it's passed on. One
//! projection, since projections compose: several are one that tries each.
//! A kind with no projection is drawn as any record is, by its kind and name
//! and then its fields in the def's order, each shown where the record lacks
//! it as a place to fill (see [`crate::libraries::isa`]), so a new record of
//! a kind needs only to say the kind: an empty place offers a new record of
//! each kind the document's library defines.

use crate::display::host::{Host, Layout, Partial, ProjectionInput};
use crate::display::{
    Completion, CompletionKind, CompletionRequest, CompletionScope, Picker, picker, ranked,
};
use crate::libraries::presentation::vocabulary::VALUE;
use crate::libraries::{Library, layout, name};
use ::grap::RuntimeValue;
use gid::{CellId, Value};
use std::rc::Rc;

pub const ID: CellId = CellId::from_u128(0x5a0fcad83599b7a4bd80398988ba22cb);

pub mod vocabulary {
    use gid::CellId;

    /// A def's kind: `{isa: def, name, fields, projection}`.
    pub const DEF: CellId = CellId::from_u128(0xa9a53af4187f4715f1eebdf58e0c65c0);
    /// The fields a kind's records have, in the order they're read.
    pub const FIELDS: CellId = CellId::from_u128(0x003bb68cfb673326a483cd31b03ba27f);
    /// How a kind's records are drawn, as a pane's value is: a template, or
    /// a function of `value`.
    pub use crate::workspace::vocabulary::PROJECTION;
    /// A library's kind: `{isa: library, defs, …}`, where a document's own
    /// defs live.
    pub const LIBRARY: CellId = CellId::from_u128(0x6a2a379c9d9553cedafcb3987e7891b7);
    /// A library's defs, each a kind's cell.
    pub const DEFS: CellId = CellId::from_u128(0x9c1a4dd291087eef39ac1d4020380ed0);
}
use vocabulary::*;

pub fn library<W: Host>() -> Library<W> {
    name::named_library(
        ID,
        "def",
        [
            (DEF, "def"),
            (FIELDS, "fields"),
            (LIBRARY, "library"),
            (DEFS, "defs"),
        ],
        ::grap::ForeignFunctions::default(),
    )
    .with_projections_for([
        (DEF, crate::display::runtime_partial(def_display)),
        (LIBRARY, crate::display::runtime_partial(library_display)),
    ])
    .with_completions(completions)
}

/// For an empty place: a new library, and a new record of each kind the
/// document's libraries define, selected as it's made; where what's typed is
/// read, those kinds whose name it begins. An empty document can start as a
/// notebook of a library.
fn completions(request: &CompletionRequest<'_>) -> Option<Vec<Completion>> {
    (request.kind == CompletionKind::Value && request.value().is_none()).then_some(())?;
    Some(match request.scope {
        CompletionScope::Start => vec![starter()],
        CompletionScope::Everything => std::iter::once(new_library())
            .chain(new_records(request, |_| true))
            .collect(),
        CompletionScope::Literal => {
            let typed = request.query.trim().to_lowercase();
            new_records(request, |name| {
                let name = name.to_lowercase();
                !typed.is_empty()
                    && (name.starts_with(&typed) || format!("new {name}").starts_with(&typed))
            })
            .collect()
        }
    })
}

/// A new record of each of the document's kinds whose name is `wanted`.
fn new_records<'r>(
    request: &'r CompletionRequest<'_>,
    wanted: impl Fn(&str) -> bool + 'r,
) -> impl Iterator<Item = Completion> + 'r {
    document_defs(request.root, |cell| Some((request.resolve)(cell)?.value))
        .into_iter()
        .filter_map(move |kind| {
            let name = (request.name)(kind)
                .map(|name| name.spelling().into_owned())
                .unwrap_or_else(|| crate::identity::short_id(kind));
            wanted(&name).then(|| {
                crate::libraries::completion::select(
                    format!("new {name}"),
                    ::grap::isa::record(kind, []),
                )
                .with_aliases([name])
            })
        })
}

/// A new library, its first def made next.
fn new_library() -> Completion {
    crate::libraries::completion::insert("new library", empty_library(), Some(first_def()))
        .with_aliases(["library"])
        .with_detail(ID)
}

/// A library with no defs yet.
fn empty_library() -> Value {
    ::grap::isa::record(LIBRARY, [(DEFS, Value::list([]))])
}

/// Where a new library's first def is made, from the library.
fn first_def() -> crate::site::Continuation {
    crate::selection::pending_at(&[
        gid::Step::Key(DEFS),
        gid::Step::Element(gid::position::between(None, None).expect("an empty list has room")),
    ])
}

/// Starts an empty document as a notebook whose first entry is a library,
/// its first def made next.
fn starter() -> Completion {
    use crate::libraries::notebook::vocabulary::{ENTRIES, kind::NOTEBOOK};
    let first = || gid::position::between(None, None).expect("an empty list has room");
    crate::libraries::completion::insert(
        LIBRARY,
        ::grap::isa::record(
            NOTEBOOK,
            [(
                ENTRIES,
                Value::List([(first(), empty_library())].into_iter().collect()),
            )],
        ),
        Some(crate::selection::pending_at(&[
            gid::Step::Key(ENTRIES),
            gid::Step::Element(first()),
            gid::Step::Key(DEFS),
            gid::Step::Element(first()),
        ])),
    )
    .with_detail(ID)
}

/// The document's own defs: those of the library its root is, or where its
/// root is a notebook, of the libraries among its entries, through the
/// sections that name them. `resolve` reads a cell, as an entry may be one.
/// Nothing else in the document is searched.
pub(crate) fn document_defs<'a>(
    root: Option<&'a Value>,
    resolve: impl Fn(CellId) -> Option<&'a Value>,
) -> Vec<CellId> {
    use crate::libraries::notebook::vocabulary::{ENTRIES, kind::NOTEBOOK};
    use crate::libraries::section::vocabulary::{BODY, SECTION};
    let read = |value: &'a Value| match value.as_cell() {
        Some(cell) => resolve(cell),
        None => Some(value),
    };
    let kind = |value: &Value| value.as_record().and_then(::grap::isa::of);
    // What an entry holds, under the sections that name it.
    let held = |entry: &'a Value| {
        std::iter::successors(read(entry), |value| match kind(value) {
            Some(SECTION) => read(value.as_record()?.get(&BODY)?),
            _ => None,
        })
        .last()
    };
    let libraries: Vec<&Value> = match root.and_then(kind) {
        Some(LIBRARY) => root.into_iter().collect(),
        Some(NOTEBOOK) => root
            .and_then(Value::as_record)
            .and_then(|fields| fields.get(&ENTRIES)?.as_list())
            .into_iter()
            .flat_map(|entries| entries.values())
            .filter_map(held)
            .filter(|value| kind(value) == Some(LIBRARY))
            .collect(),
        _ => Vec::new(),
    };
    libraries
        .into_iter()
        .filter_map(|library| library.as_record()?.get(&DEFS)?.as_list())
        .flat_map(|defs| defs.values().filter_map(Value::as_cell))
        .collect()
}

/// A library as its author edits it: its defs first, then whatever else the
/// document keeps beside them. A place among its defs offers a new one.
fn library_display<W: Host>(input: &ProjectionInput<'_, W, RuntimeValue>) -> Option<Layout<W>> {
    (input.value?.kind()? == LIBRARY).then_some(())?;
    crate::libraries::isa::form(
        input,
        crate::display::structure::Fields::default()
            .leading([DEFS])
            .slots([DEFS])
            .child(|key| {
                (key == DEFS).then(|| {
                    crate::display::structure::list(Some(crate::display::runtime_partial(
                        |input| {
                            input.value.is_none().then(|| {
                                new_place(|typed| {
                                    name::new_cell(
                                        typed,
                                        "def",
                                        vec![
                                            (::grap::isa::ISA, DEF.into()),
                                            (FIELDS, Value::list([])),
                                        ],
                                    )
                                })
                            })
                        },
                    )))
                })
            }),
        false,
    )
}

/// A def's field: a cell declared there, by its name. A place for one offers
/// a new one.
fn field<W: Host>(input: &ProjectionInput<'_, W, RuntimeValue>) -> Option<Layout<W>> {
    match input.value {
        Some(_) => crate::libraries::grap::declaration_cell(input, None),
        None => Some(new_place(|typed| name::new_cell(typed, "field", vec![]))),
    }
}

/// An empty place offering what `offer` makes of what's typed.
fn new_place<W: Host>(offer: fn(String) -> Completion) -> Layout<W> {
    picker(Picker::new(
        CompletionKind::Value,
        ranked(Rc::new(move |request| {
            Some(vec![offer(request.query.trim().to_owned())])
        })),
    ))
}

/// What a def says about a kind.
pub(crate) struct Def {
    pub fields: Vec<CellId>,
    pub projection: Option<RuntimeValue>,
}

impl Def {
    pub fn read(value: &RuntimeValue) -> Option<Self> {
        (value.kind()? == DEF).then_some(())?;
        Some(Self {
            fields: value
                .field(FIELDS)
                .and_then(|list| {
                    Some(
                        list.list_values()?
                            .filter_map(|field| field.as_cell())
                            .collect(),
                    )
                })
                .unwrap_or_default(),
            projection: value.field(PROJECTION),
        })
    }
}

/// A record drawn by its kind's def's projection, where its kind has one.
pub fn by_def<W: Host>() -> Partial<W> {
    crate::display::runtime_partial(|input| {
        let kind = input.value?.kind()?;
        let projection = Def::read(&input.env.resolve(kind)?.value.into())?.projection?;
        drawn(input, &projection)
    })
}

/// A def as its author edits it, `def fraction {fields, projection}`: its
/// name, its fields declared by name, and its projection, which where it's a
/// template is edited as a template for the def's own fields. Each is a
/// place to fill until it's there.
fn def_display<W: Host>(input: &ProjectionInput<'_, W, RuntimeValue>) -> Option<Layout<W>> {
    let parts: Rc<[CellId]> = Def::read(input.value?)?.fields.into();
    let fields = [FIELDS, PROJECTION];
    crate::libraries::isa::form(
        input,
        crate::display::structure::Fields::default()
            .leading(fields)
            .slots(fields)
            .child(move |key| match key {
                FIELDS => Some(crate::display::structure::list_flowing(Some(
                    crate::display::runtime_partial(field),
                ))),
                PROJECTION => Some(layout::template::editor(parts.clone())),
                _ => None,
            }),
        // A def names its kind.
        true,
    )
}

thread_local! {
    /// Where views are drawing right now, and what. One steps aside where
    /// it's already drawing, or for a value it's already drawing, so a view
    /// can show the value's ordinary projection inside itself, in place or as
    /// a result.
    static DRAWING: std::cell::RefCell<Vec<(Option<crate::occurrence::Trail>, RuntimeValue)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// How deeply views may nest before the loaded libraries take over, so a
/// view that keeps drawing itself still ends.
const NESTING: usize = 64;

struct Drawing;

impl Drawing {
    fn at(occurrence: Option<crate::occurrence::Trail>, value: RuntimeValue) -> Self {
        DRAWING.with(|drawing| drawing.borrow_mut().push((occurrence, value)));
        Self
    }
}

impl Drop for Drawing {
    fn drop(&mut self) {
        DRAWING.with(|drawing| drawing.borrow_mut().pop());
    }
}

/// The value `input` draws, drawn by `projection`: a template, layout data
/// drawn for it as it stands and selecting it where it's pointed at; or a
/// function of `value`, whose layout draws it in place and whose returned
/// value is drawn with the area's projections. None where it declines, or
/// where a view is already drawing this place or this value.
pub(crate) fn drawn<W: Host>(
    input: &ProjectionInput<'_, W, RuntimeValue>,
    projection: &RuntimeValue,
) -> Option<Layout<W>> {
    let value = input.value?;
    let occurrence = input.env.occurrence().cloned();
    DRAWING
        .with(|drawing| {
            let drawing = drawing.borrow();
            drawing.len() < NESTING
                && !drawing
                    .iter()
                    .any(|(at, drawn)| *at == occurrence || drawn.as_value() == value.as_value())
        })
        .then_some(())?;
    let stored = match projection.as_cell() {
        Some(cell) => input.env.resolve(cell).map(|found| found.value.into()),
        None => Some(projection.clone()),
    };
    let layout = match stored.and_then(|node| layout::template(&node, &|| input.site())) {
        Some(template) => crate::display::selectable(template, &input.site()),
        None => {
            let (evaluation, layout) = layout::scope::run(
                || input.site(),
                |scope| {
                    input
                        .env
                        .apply_runtime_scoped(projection, &[(VALUE, value.clone())], scope)
                },
            );
            match layout {
                Some(layout) => layout,
                None => {
                    return Some(evaluation.result)
                        .filter(|result| result.absent_reason().is_none())
                        .map(crate::libraries::presentation::result_layout)
                        .map(|layout| nested(&occurrence, value, layout));
                }
            }
        }
    };
    // The view chose what to show; only a field being added follows it.
    Some(nested(
        &occurrence,
        value,
        crate::display::structure::Fields::default().after(input, |_| true, layout),
    ))
}

/// `layout`, drawn as a view's drawing of `value` at `occurrence`.
fn nested<W: Host>(
    occurrence: &Option<crate::occurrence::Trail>,
    value: &RuntimeValue,
    layout: Layout<W>,
) -> Layout<W> {
    let (occurrence, value) = (occurrence.clone(), value.clone());
    Layout::new(move |builder| {
        let _drawing = Drawing::at(occurrence.clone(), value.clone());
        layout.run(builder)
    })
}

#[cfg(test)]
#[path = "tests/def.rs"]
mod tests;
