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
    Completion, CompletionKind, CompletionRequest, CompletionScope, CompletionText, Picker, picker,
    ranked,
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
    /// A library's functions, each a cell holding one.
    pub const FUNCTIONS: CellId = CellId::from_u128(0x6a6a803312ecf3a652470a7cd2a9abcf);
    /// What a place for a field offers, on the field's definition: a Grap
    /// expression evaluating to a list of the values that fit, or to a
    /// function of the record being filled (`value`) that returns one.
    pub const OFFERS: CellId = CellId::from_u128(0x6b258b5bc6d970abc5fb8b6ada32a4b9);
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
            (FUNCTIONS, "functions"),
            (OFFERS, "offers"),
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
        CompletionScope::Everything => field_offers(request, |_| true)
            .chain(std::iter::once(new_library()))
            .chain(new_records(request, |_| true))
            .collect(),
        CompletionScope::Literal => {
            let typed = request.query.trim().to_lowercase();
            let begun = |name: &str| {
                let name = name.to_lowercase();
                !typed.is_empty()
                    && (name.starts_with(&typed) || format!("new {name}").starts_with(&typed))
            };
            // What's read as typed lists every reading, so a field's offers
            // are those whose label holds what's typed.
            field_offers(request, |label| label.to_lowercase().contains(&typed))
                .chain(new_records(request, begun))
                .collect()
        }
    })
}

/// What the field a place is under says fits there, those whose label is
/// `wanted`: its definition's `offers`, evaluated, a list of values, or a
/// function of the record being filled, given as `value`, that returns
/// one. Each is labelled as it reads and says which field offers it.
fn field_offers<'r>(
    request: &'r CompletionRequest<'_>,
    wanted: impl Fn(&str) -> bool + 'r,
) -> impl Iterator<Item = Completion> + 'r {
    let offered = || {
        let (gid::Step::Key(field), record) = request.path.split_last()? else {
            return None;
        };
        let env = request.env?;
        let offers: RuntimeValue = (request.resolve)(*field)?
            .value
            .as_record()?
            .get(&OFFERS)?
            .clone()
            .into();
        let evaluated = env.evaluate_runtime(&offers, crate::sources::DEFAULT_FUEL, &[]);
        let list = match evaluated.list_values() {
            Some(_) => evaluated,
            None => env.apply_expression_runtime(
                &offers,
                &[(VALUE, (request.value_at)(record)?.clone().into())],
            ),
        };
        let values: Vec<Value> = list.list_values()?.map(|value| value.to_value()).collect();
        Some((*field, values))
    };
    offered()
        .into_iter()
        .flat_map(|(field, values)| values.into_iter().map(move |value| (field, value)))
        .filter_map(move |(field, value)| {
            let label = label(request, &value);
            wanted(&label).then(|| {
                crate::libraries::completion::select(CompletionText::Literal(label), value)
                    .with_detail(field)
            })
        })
}

/// How an offered value reads: a cell or a record by its name, text quoted,
/// a number as its digits, a record of a kind by the kind.
fn label(request: &CompletionRequest<'_>, value: &Value) -> String {
    let named = |cell: CellId| {
        (request.name)(cell)
            .map(|name| name.spelling().into_owned())
            .unwrap_or_else(|| crate::identity::short_id(cell))
    };
    if let Some(cell) = value.as_cell() {
        return named(cell);
    }
    if let Some(text) = crate::libraries::text::read(value) {
        return format!("\"{text}\"");
    }
    let number = crate::libraries::f64::read(value)
        .or_else(|| crate::libraries::f32::read(value).map(f64::from))
        .or_else(|| crate::libraries::u64::read(value).map(|n| n as f64));
    if let Some(number) = number {
        return number.to_string();
    }
    match value.as_record() {
        Some(fields) => name::read(value)
            .map(str::to_owned)
            .or_else(|| ::grap::isa::of(fields).map(named))
            .unwrap_or_else(|| "record".to_owned()),
        None => "list".to_owned(),
    }
}

/// A new record of each of the document's kinds whose name is `wanted`.
fn new_records<'r>(
    request: &'r CompletionRequest<'_>,
    wanted: impl Fn(&str) -> bool + 'r,
) -> impl Iterator<Item = Completion> + 'r {
    document_defs(request.root, &|cell| Some((request.resolve)(cell)?.value))
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
}

/// A library with no defs or functions yet.
fn empty_library() -> Value {
    ::grap::isa::record(
        LIBRARY,
        [(DEFS, Value::list([])), (FUNCTIONS, Value::list([]))],
    )
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
}

/// What a document holds where things are looked for: its root, and what
/// the containers reached from it hold, through cells, in the document's
/// order. The containers are named here, each with the field holding what's
/// in it: a notebook's entries, a section's body, a row's or column's parts.
/// Nothing else in the document is entered: a program's insides, say,
/// aren't. `resolve` reads a cell.
pub(crate) fn in_reach<'a>(
    root: Option<&'a Value>,
    resolve: &dyn Fn(CellId) -> Option<&'a Value>,
) -> Vec<&'a Value> {
    use crate::libraries::notebook::vocabulary::{ENTRIES, kind::NOTEBOOK};
    use crate::libraries::section::vocabulary::{BODY, SECTION};
    use crate::workspace::vocabulary::{PARTS, kind::COLUMN, kind::ROW};
    let held = |kind: CellId| match kind {
        NOTEBOOK => Some(ENTRIES),
        SECTION => Some(BODY),
        ROW | COLUMN => Some(PARTS),
        _ => None,
    };
    let mut reached = Vec::new();
    let mut followed = std::collections::HashSet::new();
    let mut pending: Vec<&Value> = root.into_iter().collect();
    while let Some(value) = pending.pop() {
        let value = match value.as_cell() {
            Some(cell) if followed.insert(cell) => match resolve(cell) {
                Some(definition) => definition,
                None => continue,
            },
            Some(_) => continue,
            None => value,
        };
        reached.push(value);
        let inside = value
            .as_record()
            .and_then(|fields| fields.get(&held(::grap::isa::of(fields)?)?));
        match inside {
            Some(Value::List(inside)) => pending.extend(inside.values().rev()),
            Some(inside) => pending.push(inside),
            None => {}
        }
    }
    reached
}

/// The cells listed under `list` by what the document holds in reach
/// ([`in_reach`]): a library, say, or anything else that lists them there.
fn listed<'a>(
    root: Option<&'a Value>,
    resolve: &dyn Fn(CellId) -> Option<&'a Value>,
    list: CellId,
) -> Vec<CellId> {
    in_reach(root, resolve)
        .into_iter()
        .filter_map(|value| value.as_record()?.get(&list)?.as_list())
        .flat_map(|cells| cells.values().filter_map(Value::as_cell))
        .collect()
}

/// The document's own kinds: the defs listed in reach.
pub(crate) fn document_defs<'a>(
    root: Option<&'a Value>,
    resolve: &dyn Fn(CellId) -> Option<&'a Value>,
) -> Vec<CellId> {
    listed(root, resolve, DEFS)
}

/// The document's own functions: those listed in reach.
pub(crate) fn document_functions<'a>(
    root: Option<&'a Value>,
    resolve: &dyn Fn(CellId) -> Option<&'a Value>,
) -> Vec<CellId> {
    listed(root, resolve, FUNCTIONS)
}

/// A library as its author edits it: its defs, then its functions, then
/// whatever else it keeps. A place among its defs offers a new def, and one
/// among its functions a new function, each named as typed.
fn library_display<W: Host>(input: &ProjectionInput<'_, W, RuntimeValue>) -> Option<Layout<W>> {
    (input.value?.kind()? == LIBRARY).then_some(())?;
    // A list whose empty place offers what `new` makes of what's typed.
    let growing = |new: fn(String) -> Completion| {
        crate::display::structure::list(Some(crate::display::runtime_partial(move |input| {
            input.value.is_none().then(|| new_place(new))
        })))
    };
    crate::libraries::isa::form(
        input,
        crate::display::structure::Fields::default()
            .leading([DEFS, FUNCTIONS])
            .slots([DEFS, FUNCTIONS])
            .child(move |key| match key {
                DEFS => Some(growing(|typed| {
                    name::new_cell(
                        typed,
                        "def",
                        vec![(::grap::isa::ISA, DEF.into()), (FIELDS, Value::list([]))],
                    )
                })),
                FUNCTIONS => Some(growing(|typed| {
                    name::new_cell(
                        typed,
                        "function",
                        vec![
                            (
                                ::grap::isa::ISA,
                                Value::from(::grap::vocabulary::kind::LAMBDA),
                            ),
                            (::grap::vocabulary::PARAMS, Value::list([])),
                        ],
                    )
                })),
                _ => None,
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
