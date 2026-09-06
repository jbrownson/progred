//! The editor's ordered composition of Progred libraries.

use crate::hover::Hover;
use crate::projection::Projection;
use progred_libraries::{
    Libraries, Library, absent, blob, color, control, f32, f64, fidget, geometry,
    grap as grap_library, layout, line_edit, list, logic, name, number, presentation, random,
    selection, site, text, u64, workspace,
};

pub struct Stack<World> {
    pub libraries: Libraries,
    pub projection: Projection<World>,
    pub pane_projection: Projection<World>,
    pub completions: progred_display::CompletionProvider,
}

impl<World> Clone for Stack<World> {
    fn clone(&self) -> Self {
        Self {
            libraries: self.libraries.clone(),
            projection: self.projection.clone(),
            pane_projection: self.pane_projection.clone(),
            completions: self.completions.clone(),
        }
    }
}

pub fn load<World: 'static>() -> Stack<World> {
    let (libraries, projections, providers) = Libraries::from_contributions(contributions());
    let completions = std::rc::Rc::new(move |request: &progred_display::CompletionRequest<'_>| {
        providers
            .iter()
            .filter_map(|provider| provider(request))
            .fold(None, |offers, next| {
                let mut offers = offers.unwrap_or_else(Vec::new);
                offers.extend(next);
                Some(offers)
            })
    });
    let projection = Projection::new(projections);
    Stack {
        libraries,
        pane_projection: projection
            .clone()
            .with_entry(progred_display::partial(presentation::projected_display)),
        projection,
        completions,
    }
}

fn contributions<World: 'static>() -> impl Iterator<Item = (gid::CellId, Library<World, Hover>)> {
    [
        (name::ID, name::library()),
        (text::ID, text::library()),
        (blob::ID, blob::library()),
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
        (
            progred_libraries::path::ID,
            progred_libraries::path::library(),
        ),
        (site::ID, site::library()),
        (geometry::ID, geometry::library()),
        (workspace::ID, workspace::library()),
    ]
    .into_iter()
}
