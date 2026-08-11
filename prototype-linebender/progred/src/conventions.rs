//! Projection policy over ordinary graph conventions. The data model
//! knows no names or classifications; libraries contribute values and
//! projections decide how to interpret them.

use crate::sources::Sources;
use progred_graph::{CellId, Cells};
use std::rc::Rc;

pub mod vocabulary {
    use progred_graph::CellId;

    /// Field projection: its value is evaluated by Grap and the result
    /// is projected in normal-form mode. This is not a Grap evaluator
    /// form.
    pub const GRAP: CellId = CellId::from_u128(0xac807d20d964e141d44c1b2eb98e5ca9);
}

pub fn library() -> Cells {
    let mut cells = progred_name::library();
    cells.merge(progred_isa::library());
    cells.merge(grap::library());
    cells.merge(grap_absent::library());
    cells.merge(grap_control::library());
    cells.merge(grap_f64::library());
    cells.merge(grap_geometry::library());
    cells.set_value(vocabulary::GRAP, progred_name::record("grap", []));
    cells
}

pub fn foreign_functions() -> grap::ForeignFunctions {
    let mut foreign = grap::ForeignFunctions::new();
    grap_control::install(&mut foreign).expect("control foreign functions are distinct");
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
