//! The editor's ordered composition of Progred libraries.

use crate::hover::Hover;
use crate::projection::Projection;
use crate::workspace;
use progred_libraries::{
    Libraries, Library, absent, color, control, f32, f64, fidget, geometry, grap as grap_library,
    layout, line_edit, list, logic, name, presentation, random, selection, site, text, u64,
};

pub struct Stack<World> {
    pub libraries: Libraries,
    pub projection: Projection<World>,
}

impl<World> Clone for Stack<World> {
    fn clone(&self) -> Self {
        Self {
            libraries: self.libraries.clone(),
            projection: self.projection.clone(),
        }
    }
}

pub fn load<World: 'static>() -> Stack<World> {
    let (libraries, projections) = Libraries::from_contributions(contributions());
    Stack {
        libraries,
        projection: Projection::new(projections),
    }
}

fn contributions<World: 'static>() -> impl Iterator<Item = (gid::CellId, Library<World, Hover>)> {
    [
        (name::ID, name::library()),
        (text::ID, text::library()),
        (absent::ID, absent::library()),
        (color::ID, color::library()),
        (control::ID, control::library()),
        (f32::ID, f32::library()),
        (f64::ID, f64::library()),
        (fidget::ID, fidget::library()),
        (grap_library::ID, grap_library::library()),
        (line_edit::ID, line_edit::library()),
        (u64::ID, u64::library()),
        (logic::ID, logic::library()),
        (list::ID, list::library()),
        (random::ID, random::library()),
        (presentation::ID, presentation::library()),
        (layout::ID, layout::library()),
        (selection::ID, selection::library()),
        (site::ID, site::library()),
        (geometry::ID, geometry::library()),
        (workspace::ID, workspace::library()),
    ]
    .into_iter()
}
