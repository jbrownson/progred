//! The document's pane vocabulary. The editor interprets it at the root;
//! libraries can use the same ordinary data when constructing templates.

use crate::{Library, name};
use gid::{CellId, Cells, Value};

pub const ID: CellId = CellId::from_u128(0x7c295d8a64d3e257dc2c3932e43def74);

pub mod vocabulary {
    use gid::CellId;

    pub const PANES: CellId = CellId::from_u128(0xf30d400a4321a4d44d1628a8adc5a84d);
    pub const LEFT: CellId = CellId::from_u128(0xdc3a1b9a7fb4bc348760160e3b365bca);
    pub const RIGHT: CellId = CellId::from_u128(0xf13a5c1c4471c00178575a0e876768f8);
}

pub fn library<World, Hover>() -> Library<World, Hover> {
    let mut cells = Cells::new();
    for (cell, spelling) in [
        (vocabulary::PANES, "panes"),
        (vocabulary::LEFT, "left"),
        (vocabulary::RIGHT, "right"),
    ] {
        cells.set_value(cell, name::record(spelling, []));
    }
    Library::named(
        "workspace",
        crate::Definitions::from_parts(cells, Default::default()),
        vec![],
    )
    .with_root_field_completions([progred_display::Completion::new(
        "panes",
        Value::from(vocabulary::PANES),
    )
    .with_detail("workspace library")])
}
