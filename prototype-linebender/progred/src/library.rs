//! A loaded library: cells, foreign functions, and projections.

use crate::hover::Hover;
use gid::Cells;
use grap::ForeignFunctions;
use progred_display::Partial;

pub struct Library<World> {
    pub cells: Cells,
    pub functions: ForeignFunctions,
    pub projection: Option<Partial<World, Hover>>,
}

impl<World> Default for Library<World> {
    fn default() -> Self {
        Self {
            cells: Cells::new(),
            functions: ForeignFunctions::default(),
            projection: None,
        }
    }
}

/// Name, isa, and text: the editor's conventions library.
pub fn conventions<World>() -> Library<World> {
    Library {
        cells: progred_name::library().merged(progred_isa::library()),
        projection: Some(progred_text::display::<World, Hover>),
        ..Library::default()
    }
}

pub fn grap<World>() -> Library<World> {
    Library {
        cells: grap::library(),
        functions: grap::functions(),
        projection: Some(grap::display::<World, Hover>),
        ..Library::default()
    }
}

pub fn absent<World>() -> Library<World> {
    Library {
        cells: grap_absent::library(),
        ..Library::default()
    }
}

pub fn control<World>() -> Library<World> {
    Library {
        cells: grap_control::library(),
        functions: grap_control::functions(),
        ..Library::default()
    }
}

pub fn f64<World>() -> Library<World> {
    Library {
        cells: grap_f64::library(),
        functions: grap_f64::functions(),
        projection: Some(grap_f64::display::<World, Hover>),
        ..Library::default()
    }
}

pub fn geometry<World>() -> Library<World> {
    Library {
        cells: grap_geometry::library(),
        functions: grap_geometry::functions(),
        ..Library::default()
    }
}
