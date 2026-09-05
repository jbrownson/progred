//! The editor's ordered composition of Progred libraries.

use crate::hover::Hover;
use crate::projection::Projection;
use crate::workspace;
use progred_libraries::{
    Libraries, Library, absent, color, control, f32, f64, fidget, geometry, grap as grap_library,
    layout, line_edit, list, logic, name, number, presentation, random, selection, site, text, u64,
};

pub struct Stack<World> {
    pub libraries: Libraries,
    pub projection: Projection<World>,
    pub pane_projection: Projection<World>,
    pub root_completions: progred_display::CompletionProvider,
    pub root_field_completions: progred_display::CompletionProvider,
}

impl<World> Clone for Stack<World> {
    fn clone(&self) -> Self {
        Self {
            libraries: self.libraries.clone(),
            projection: self.projection.clone(),
            pane_projection: self.pane_projection.clone(),
            root_completions: self.root_completions.clone(),
            root_field_completions: self.root_field_completions.clone(),
        }
    }
}

pub fn load<World: 'static>() -> Stack<World> {
    let (libraries, projections, root_completions, root_field_completions) =
        Libraries::from_contributions(contributions());
    let root_completions = std::rc::Rc::new(move |_: &str| root_completions.clone());
    let root_field_completions = std::rc::Rc::new(move |_: &str| root_field_completions.clone());
    Stack {
        libraries,
        pane_projection: Projection::new(
            std::iter::once(progred_display::partial(presentation::projected_display))
                .chain(projections.iter().cloned()),
        ),
        projection: Projection::new(projections),
        root_completions,
        root_field_completions,
    }
}

fn contributions<World: 'static>() -> impl Iterator<Item = (gid::CellId, Library<World, Hover>)> {
    [
        (name::ID, name::library()),
        (text::ID, text::library()),
        (absent::ID, absent::library()),
        (color::ID, color::library()),
        (control::ID, control::library()),
        (number::ID, number::library()),
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
        (progred_libraries::path::ID, progred_libraries::path::library()),
        (site::ID, site::library()),
        (geometry::ID, geometry::library()),
        (workspace::ID, workspace::library()),
    ]
    .into_iter()
}
