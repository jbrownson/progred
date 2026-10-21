//! The libraries a document declares: one library whose projection finds
//! them at the document's root and behaves as them, so switching it off
//! switches them all off, like any other library.

use crate::display::host::{Host, Layout, Partial};
use crate::libraries::presentation::vocabulary;
use crate::libraries::{Library, layout, name};
use gid::{CellId, Step};

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
        Vec<(crate::occurrence::Trail, ::grap::RuntimeValue)>,
    > =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// How deeply document libraries' drawings may nest before the loaded
/// libraries take over, so a view that keeps drawing itself still ends.
const NESTING: usize = 64;

struct Drawing;

impl Drawing {
    fn at(occurrence: crate::occurrence::Trail, value: ::grap::RuntimeValue) -> Self {
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
        let occurrence = input.env.occurrence().clone();
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
                let (evaluation, layout) = layout::scope::run(
                    || input.site(),
                    |scope| {
                        input.env.apply_runtime_scoped(
                            &projection,
                            &[(vocabulary::VALUE, value.clone())],
                            Some(scope),
                        )
                    },
                );
                match layout {
                    // The view chose what to show; only a field being added
                    // follows it.
                    Some(layout) => Some(crate::display::structure::with_extra_fields(
                        input,
                        |_| true,
                        layout,
                    )),
                    None => Some(evaluation.result)
                        .filter(|result| evaluation.completed && result.absent_reason().is_none())
                        .map(|result| {
                            crate::display::at_with_projection(
                                [Step::Key(vocabulary::RESULT)],
                                result,
                                Some(input.default_projection.clone()),
                                Some(input.default_projection.clone()),
                            )
                        }),
                }
                .map(|layout| {
                    let occurrence = occurrence.clone();
                    let value = value.clone();
                    Layout::new(move |builder| {
                        let _drawing = Drawing::at(occurrence.clone(), value.clone());
                        layout.run(builder)
                    })
                })
            })
    })
}
