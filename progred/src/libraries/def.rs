//! Defs: what a kind is. A record says its kind with `isa`; the kind's cell is
//! defined by a def, `{isa: def, name, fields, projection}`, which says what
//! such a record has and how it's drawn, connecting what's known about a kind
//! to the kind itself. A record is drawn by its kind's def's projection
//! before anything else draws it; where that declines, it's passed on. One
//! projection, since projections compose: several are one that tries each.
//! A kind with no projection is drawn as a record of its fields, each shown
//! where the record lacks it as a place to fill, so a new record of a kind
//! needs only to say the kind: an empty place offers a new record of each
//! kind the document's library defines.

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

/// A new record of each kind the document's library defines, for an empty
/// place, selected as it's made: all of them where a projection asks for
/// everything, and where what's typed is read, those whose name it begins.
fn completions(request: &CompletionRequest<'_>) -> Option<Vec<Completion>> {
    (request.kind == CompletionKind::Value && request.value().is_none()).then_some(())?;
    let typed = request.query.trim().to_lowercase();
    let wanted = |name: &str| match request.scope {
        CompletionScope::Everything => true,
        CompletionScope::Literal => {
            !typed.is_empty()
                && [name.to_lowercase(), format!("new {}", name.to_lowercase())]
                    .iter()
                    .any(|spelling| spelling.starts_with(&typed))
        }
        CompletionScope::Start => false,
    };
    Some(
        document_defs(request.root)
            .into_iter()
            .filter_map(|kind| {
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
            .collect(),
    )
}

/// The document's own defs: those of the library its root is, where its root
/// is one. Nothing else in the document is searched.
pub(crate) fn document_defs(root: Option<&Value>) -> Vec<CellId> {
    root.and_then(Value::as_record)
        .filter(|fields| ::grap::isa::of(fields) == Some(LIBRARY))
        .and_then(|fields| fields.get(&DEFS)?.as_list())
        .map(|defs| defs.values().filter_map(Value::as_cell).collect())
        .unwrap_or_default()
}

/// A library as its author edits it: its defs first, then whatever else the
/// document keeps beside them. A place among its defs offers a new one.
fn library_display<W: Host>(input: &ProjectionInput<'_, W, RuntimeValue>) -> Option<Layout<W>> {
    (input.value?.kind()? == LIBRARY).then_some(())?;
    crate::display::structure::Fields::default()
        .leading([::grap::isa::ISA, name::vocabulary::NAME, DEFS])
        .slots([DEFS])
        .child(|key| {
            (key == DEFS).then(|| {
                crate::display::structure::list(Some(crate::display::runtime_partial(|input| {
                    input.value.is_none().then(|| {
                        new_place(|typed| {
                            name::new_cell(
                                typed,
                                "def",
                                vec![(::grap::isa::ISA, DEF.into()), (FIELDS, Value::list([]))],
                            )
                        })
                    })
                })))
            })
        })
        .record(input)
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

/// A record drawn by its kind's def, where its kind has one.
pub fn by_def<W: Host>() -> Partial<W> {
    crate::display::runtime_partial(|input| {
        let kind = input.value?.kind()?;
        let Def { fields, projection } = Def::read(&input.env.resolve(kind)?.value.into())?;
        match projection {
            Some(projection) => drawn(input, &projection),
            // A kind with no projection of its own is a record of its
            // fields, in order, each drawn even where the record lacks it,
            // as a place to fill.
            None => crate::display::structure::Fields::default()
                .leading([::grap::isa::ISA].into_iter().chain(fields.iter().copied()))
                .slots(fields)
                .record(input),
        }
    })
}

/// A def as its author edits it: its name, its fields declared by name, and
/// its projection, which where it's a template is edited as a template for
/// the def's own fields. Each is a place to fill until it's there.
fn def_display<W: Host>(input: &ProjectionInput<'_, W, RuntimeValue>) -> Option<Layout<W>> {
    let parts: Rc<[CellId]> = Def::read(input.value?)?.fields.into();
    let fields = [name::vocabulary::NAME, FIELDS, PROJECTION];
    crate::display::structure::Fields::default()
        .leading([::grap::isa::ISA].into_iter().chain(fields))
        .slots(fields)
        .child(move |key| match key {
            FIELDS => Some(crate::display::structure::list_flowing(Some(
                crate::display::runtime_partial(field),
            ))),
            PROJECTION => Some(layout::template::editor(parts.clone())),
            _ => None,
        })
        .record(input)
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
