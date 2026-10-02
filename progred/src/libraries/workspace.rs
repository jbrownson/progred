//! The document's workspace vocabulary: its panes and its own libraries. The
//! editor interprets both at the root; libraries can use the same ordinary
//! data when constructing templates.

use crate::libraries::{Library, name};
use gid::{CellId, Cells};

pub const ID: CellId = CellId::from_u128(0x7c295d8a64d3e257dc2c3932e43def74);

pub mod vocabulary {
    use gid::CellId;

    pub const PANES: CellId = CellId::from_u128(0xf30d400a4321a4d44d1628a8adc5a84d);
    pub const LEFT: CellId = CellId::from_u128(0xdc3a1b9a7fb4bc348760160e3b365bca);
    pub const RIGHT: CellId = CellId::from_u128(0xf13a5c1c4471c00178575a0e876768f8);
    pub const LIBRARIES: CellId = CellId::from_u128(0xbc197ad66fea59a9bd97c4191c7845b2);
    /// The keys a document library owns: it draws records carrying them.
    pub const KEYS: CellId = CellId::from_u128(0xbb30d215605f5599e31f6274641ca89f);
}

pub fn library() -> Library<crate::Editor, crate::frame::Hovered> {
    let mut cells = Cells::new();
    for (cell, spelling) in [
        (vocabulary::PANES, "panes"),
        (vocabulary::LEFT, "left"),
        (vocabulary::RIGHT, "right"),
        (vocabulary::LIBRARIES, "libraries"),
        (vocabulary::KEYS, "keys"),
    ] {
        cells.set_value(cell, name::record(spelling, []));
    }
    Library::named(
        ID,
        "workspace",
        crate::libraries::Definitions::from_parts(cells, Default::default()),
        crate::display::runtime_partial(|_| None),
    )
    .with_completions(|request| {
        use crate::display::{CompletionKind, CompletionScope};
        match (request.scope, request.kind, request.path) {
            (CompletionScope::Suggested, CompletionKind::Field, []) => Some(
                [vocabulary::PANES, vocabulary::LIBRARIES]
                    .map(|field| crate::libraries::completion::label(field).with_detail(ID))
                    .to_vec(),
            ),
            _ => None,
        }
    })
}
