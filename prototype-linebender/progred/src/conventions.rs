//! Projection policy over ordinary graph conventions. The data model
//! knows no names or classifications; libraries contribute values and
//! projections decide how to interpret them.

use crate::sources::Sources;
use progred_graph::{CellId, Cells};
use std::rc::Rc;

pub fn library() -> Cells {
    let mut cells = progred_name::library();
    cells.merge(progred_isa::library());
    cells.merge(grap::library());
    cells.merge(grap_error::library());
    cells.merge(grap_f64::library());
    cells.merge(grap_geometry::library());
    cells
}

pub fn foreign_functions() -> grap::ForeignFunctions {
    let mut foreign = grap::ForeignFunctions::new();
    grap_f64::install(&mut foreign).expect("f64 foreign functions are distinct");
    grap_geometry::install(&mut foreign).expect("geometry foreign functions are distinct");
    foreign
}

/// A swappable display policy. The bootstrap policy recognizes the
/// ordinary simple-name relation; languages and domains can layer
/// scope-sensitive, multilingual, or computed descriptions later.
#[derive(Clone)]
pub struct Names(Rc<dyn Fn(&Sources, CellId) -> Option<String>>);

impl Names {
    pub fn convention() -> Self {
        Self(Rc::new(|sources, cell| {
            sources
                .value(cell)
                .and_then(progred_name::read)
                .map(str::to_owned)
        }))
    }

    pub fn of(&self, sources: &Sources, cell: CellId) -> Option<String> {
        (self.0)(sources, cell)
    }
}

impl Default for Names {
    fn default() -> Self {
        Self::convention()
    }
}

/// Raw shows the uninterpreted value and therefore uses the short id.
/// Other views ask their configured display policy.
pub fn display_name(sources: &Sources, names: &Names, raw: bool, cell: CellId) -> Option<String> {
    (!raw).then(|| names.of(sources, cell)).flatten()
}
