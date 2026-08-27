//! The editor's ordered composition of Progred libraries.

use crate::hover::Hover;
use crate::projection::Projection;
use crate::workspace;
use gid::Cells;
use progred_libraries::{
    Library, absent, color, control, f64, geometry, grap as grap_library, layout, line_edit, list,
    logic, name, presentation, random, selection, site, text, u64,
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
    let foreign = library.functions.clone();
    let projection = Projection::new(library.projections);
    Stack {
        library: library.cells.merged(workspace::cells()),
        foreign,
        projection,
    }
}

fn libraries<World>() -> impl Iterator<Item = Library<World, Hover>> {
    [
        name::library(),
        text::library(),
        absent::library(),
        color::library(),
        control::library(),
        grap_library::library(),
        line_edit::library(),
        f64::library(),
        u64::library(),
        logic::library(),
        list::library(),
        random::library(),
        presentation::library(),
        layout::library(),
        selection::library(),
        site::library(),
        geometry::library(),
    ]
    .into_iter()
}
