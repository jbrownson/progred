//! The libraries a document declares: one library whose projection finds
//! them at the document's root and behaves as them, so switching it off
//! switches them all off, like any other library.

use crate::display::host::{Host, Layout, Partial};
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
        input
            .env
            .libraries()
            .into_iter()
            .filter(|library| library.keys.iter().any(|key| value.field(*key).is_some()))
            .find_map(|library| crate::libraries::def::drawn(input, &library.projection))
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
        .child(move |key| {
            let keys = keys.clone();
            (key == PROJECTION).then(|| {
                layout::template::editor(std::rc::Rc::new(move |request| {
                    layout::template::scanned_parts(request, &keys)
                }))
            })
        })
        .record(input)
}
