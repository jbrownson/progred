//! The editor's ordered composition of Progred libraries.

use crate::hover::Hover;
use crate::projection::Projection;
use gid::Cells;
use progred_libraries::{
    layout, line_edit, selection,
    Library, absent, control, f64, geometry, grap as grap_library, isa, name, site, text,
};

pub struct Stack<World> {
    pub library: Cells,
    pub foreign: grap::ForeignFunctions,
    pub projection: Projection<World>,
}

impl<World> Clone for Stack<World> {
    fn clone(&self) -> Self {
        Self {
            library: self.library.clone(),
            foreign: self.foreign.clone(),
            projection: self.projection.clone(),
        }
    }
}

pub fn load<World>() -> Stack<World> {
    let library = Library::merge_all(libraries());
    let foreign = library
        .functions
        .clone()
        .merge(crate::line_edit::drawing_functions());
    let projection = Projection::new(library.projections);
    Stack {
        library: library.cells,
        foreign,
        projection,
    }
}

fn libraries<World>() -> impl Iterator<Item = Library<World, Hover>> {
    [
        name::library(),
        text::library(),
        isa::library(),
        absent::library(),
        control::library(),
        grap_library::library(),
        line_edit::library(),
        f64::library(),
        layout::library(),
        selection::library(),
        site::library(),
        geometry::library(),
    ]
    .into_iter()
}
