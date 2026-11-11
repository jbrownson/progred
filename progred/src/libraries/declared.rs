//! The libraries a document declares: one library whose projection finds
//! them at the document's root and behaves as them, so switching it off
//! switches them all off, like any other library.

use crate::display::host::{Host, Layout, Partial};
use crate::libraries::presentation::vocabulary;
use crate::libraries::{Library, layout, name};
use gid::CellId;

pub const ID: CellId = CellId::from_u128(0x0ab7c6ce75b78c779291147f380cb6c4);

pub fn library<W: Host>() -> Library<W> {
    name::named_library(
        ID,
        "document libraries",
        [],
        ::grap::ForeignFunctions::default(),
    )
    .with_projection(projection())
}

thread_local! {
    /// Where document libraries are drawing right now, and what. One steps
    /// aside where it's already drawing, or for a value it's already drawing,
    /// so its view can show the value's ordinary projection inside itself,
    /// in place or as a result.
    static DRAWING: std::cell::RefCell<
        Vec<(Option<crate::occurrence::Trail>, ::grap::RuntimeValue)>,
    > =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// How deeply document libraries' drawings may nest before the loaded
/// libraries take over, so a view that keeps drawing itself still ends.
const NESTING: usize = 64;

struct Drawing;

impl Drawing {
    fn at(occurrence: Option<crate::occurrence::Trail>, value: ::grap::RuntimeValue) -> Self {
        DRAWING.with(|drawing| drawing.borrow_mut().push((occurrence, value)));
        Self
    }
}

impl Drop for Drawing {
    fn drop(&mut self) {
        DRAWING.with(|drawing| drawing.borrow_mut().pop());
    }
}

/// The libraries a document declares at its root, tried in order on a record
/// carrying a key one owns. A projection that emits a layout draws the record
/// in place, so whatever it descends into stays editable; one that returns a
/// value draws that value with the area's projections; an absent passes the
/// record on.
fn projection<W: Host>() -> Partial<W> {
    crate::display::runtime_partial(|input| {
        let value = input.value?;
        if let Some(declaration) = declaration(input) {
            return Some(declaration);
        }
        let occurrence = input.env.occurrence().cloned();
        DRAWING
            .with(|drawing| {
                let drawing = drawing.borrow();
                drawing.len() < NESTING
                    && !drawing.iter().any(|(at, drawn)| {
                        *at == occurrence || drawn.as_value() == value.as_value()
                    })
            })
            .then_some(())?;
        input
            .env
            .libraries()
            .into_iter()
            .filter(|library| library.keys.iter().any(|key| value.field(*key).is_some()))
            .find_map(|library| {
                let projection = library.projection;
                // A view that is layout data is a template: drawn for the
                // record as it stands, selecting the record where it's
                // pointed at, its `descend`s leading into the record.
                let stored = match projection.as_cell() {
                    Some(cell) => input.env.resolve(cell).map(|found| found.value.into()),
                    None => Some(projection.clone()),
                };
                let template = stored
                    .and_then(|node| layout::template(&node, &|| input.site()))
                    .map(|layout| crate::display::selectable(layout, &input.site()));
                let drawn = match template {
                    Some(layout) => Some(layout),
                    None => {
                        let (evaluation, layout) = layout::scope::run(
                            || input.site(),
                            |scope| {
                                input.env.apply_runtime_scoped(
                                    &projection,
                                    &[(vocabulary::VALUE, value.clone())],
                                    scope,
                                )
                            },
                        );
                        match layout {
                            Some(layout) => Some(layout),
                            None => {
                                return Some(evaluation.result)
                                    .filter(|result| result.absent_reason().is_none())
                                    .map(crate::libraries::presentation::result_layout)
                                    .map(|layout| nested(&occurrence, value, layout));
                            }
                        }
                    }
                };
                // The view chose what to show; only a field being added
                // follows it.
                drawn
                    .map(|layout| {
                        crate::display::structure::Fields::default().after(input, |_| true, layout)
                    })
                    .map(|layout| nested(&occurrence, value, layout))
            })
    })
}

/// A library's own record, as [`crate::workspace::libraries`] reads one: its
/// view, where that's a template, drawn as a template's author edits it.
fn declaration<W: Host>(
    input: &crate::display::host::ProjectionInput<'_, W, ::grap::RuntimeValue>,
) -> Option<Layout<W>> {
    use crate::workspace::vocabulary::{KEYS, PROJECTION};
    let fields = input.value?;
    fields.field(PROJECTION)?;
    let keys: std::rc::Rc<[CellId]> = fields
        .field(KEYS)?
        .list_values()?
        .filter_map(|key| key.as_cell())
        .collect();
    crate::display::structure::Fields::default()
        .child(move |key| (key == PROJECTION).then(|| layout::template::editor(keys.clone())))
        .record(input)
}

/// `layout`, drawn as a document library's drawing of `value` at `occurrence`.
fn nested<W: Host>(
    occurrence: &Option<crate::occurrence::Trail>,
    value: &::grap::RuntimeValue,
    layout: Layout<W>,
) -> Layout<W> {
    let (occurrence, value) = (occurrence.clone(), value.clone());
    Layout::new(move |builder| {
        let _drawing = Drawing::at(occurrence.clone(), value.clone());
        layout.run(builder)
    })
}
