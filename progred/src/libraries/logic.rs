//! Boolean values used by Grap libraries. They are ordinary named
//! cells; control remains in `match`, rather than in the evaluator.

use crate::libraries::{Library, name};
use gid::{Cells, Value};

pub const ID: gid::CellId = gid::CellId::from_u128(0xf76a2ef341541a5c955fc23094e2df52);

pub mod vocabulary {
    use gid::CellId;

    pub const TRUE: CellId = CellId::from_u128(0x04831f9b704935059231eb111770e62e);
    pub const FALSE: CellId = CellId::from_u128(0x8fcc0a2e5f26c72efd9e917a28919801);
}

pub fn value(value: bool) -> Value {
    Value::from(if value {
        vocabulary::TRUE
    } else {
        vocabulary::FALSE
    })
}

pub fn library() -> Library<crate::Editor, crate::frame::Hovered> {
    let mut cells = Cells::new();
    for (cell, spelling) in [(vocabulary::TRUE, "true"), (vocabulary::FALSE, "false")] {
        cells.set_value(cell, name::record(spelling, []));
    }
    Library::named(
        ID,
        "logic",
        crate::libraries::Definitions::from_parts(cells, Default::default()),
        crate::display::partial(|_| None),
    )
}
