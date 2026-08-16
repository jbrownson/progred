//! A loaded library: cells, foreign functions, and projections.

use crate::conventions;
use grap::ForeignFunctions;
use progred_display::Partial;
use progred_graph::Cells;

pub struct Library {
    pub cells: Cells,
    pub functions: ForeignFunctions,
    pub projections: Vec<Partial>,
}

impl Default for Library {
    fn default() -> Self {
        Self {
            cells: Cells::new(),
            functions: ForeignFunctions::default(),
            projections: Vec::new(),
        }
    }
}

pub fn name() -> Library {
    Library {
        cells: progred_name::library(),
        ..Library::default()
    }
}

pub fn isa() -> Library {
    Library {
        cells: progred_isa::library(),
        ..Library::default()
    }
}

pub fn text() -> Library {
    Library {
        projections: vec![progred_text::display],
        ..Library::default()
    }
}

pub fn grap() -> Library {
    let mut cells = grap::library();
    cells.set_value(
        conventions::vocabulary::GRAP,
        progred_name::record("grap", []),
    );
    Library {
        cells,
        functions: grap::functions(),
        projections: vec![conventions::display],
        ..Library::default()
    }
}

pub fn absent() -> Library {
    Library {
        cells: grap_absent::library(),
        ..Library::default()
    }
}

pub fn control() -> Library {
    Library {
        cells: grap_control::library(),
        functions: grap_control::functions(),
        ..Library::default()
    }
}

pub fn f64() -> Library {
    Library {
        cells: grap_f64::library(),
        functions: grap_f64::functions(),
        projections: vec![grap_f64::display],
        ..Library::default()
    }
}

pub fn geometry() -> Library {
    Library {
        cells: grap_geometry::library(),
        functions: grap_geometry::functions(),
        ..Library::default()
    }
}

pub fn merge(libraries: impl IntoIterator<Item = Library>) -> Library {
    libraries.into_iter().fold(Library::default(), |all, next| {
        Library {
            cells: all.cells.merged(next.cells),
            functions: all.functions.merge(next.functions),
            projections: {
                let mut projections = all.projections;
                projections.extend(next.projections);
                projections
            },
        }
    })
}
