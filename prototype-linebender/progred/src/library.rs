//! A loaded library: cells, foreign functions, and projections.

use grap::ForeignFunctions;
use progred_display::Partial;
use progred_graph::Cells;

pub struct Library {
    pub cells: Cells,
    pub functions: ForeignFunctions,
    pub projection: Option<Partial>,
}

impl Default for Library {
    fn default() -> Self {
        Self {
            cells: Cells::new(),
            functions: ForeignFunctions::default(),
            projection: None,
        }
    }
}

/// Name, isa, and text: the editor's conventions library.
pub fn conventions() -> Library {
    merge([
        Library {
            cells: progred_name::library(),
            ..Library::default()
        },
        Library {
            cells: progred_isa::library(),
            ..Library::default()
        },
        Library {
            projection: Some(progred_text::display),
            ..Library::default()
        },
    ])
}

pub fn grap() -> Library {
    Library {
        cells: grap::library(),
        functions: grap::functions(),
        projection: Some(grap::display),
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
        projection: Some(grap_f64::display),
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
            projection: all.projection.or(next.projection),
        }
    })
}
