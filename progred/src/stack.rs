//! The editor's ordered composition of Progred libraries.

use crate::frame::Hovered;
use crate::libraries::{
    absent, blob, color, control, controls, conversion, f32, f64, fidget, geometry,
    grap as grap_library, layout, line_edit, list, logic, name, number, png, presentation, random,
    selection, sequence, site, text, toolpath, tree, u64, workspace,
};
use crate::library::{Libraries, Library};
use crate::projection::Projection;
use std::rc::Rc;

pub struct Stack<World> {
    pub libraries: Libraries,
    pub projection: Projection<World>,
    pub pane_projection: Projection<World>,
    pub completions: crate::display::CompletionProvider,
    /// The projections libraries bring, in load order, so an area can leave
    /// some of them off.
    partials: Rc<[(gid::CellId, crate::display::Partial<World, Hovered>)]>,
}

impl<World> Clone for Stack<World> {
    fn clone(&self) -> Self {
        Self {
            libraries: self.libraries.clone(),
            projection: self.projection.clone(),
            pane_projection: self.pane_projection.clone(),
            completions: self.completions.clone(),
            partials: self.partials.clone(),
        }
    }
}

impl<World> Stack<World> {
    /// What an area can leave off, and its name, in the order projections
    /// are tried: each library that brings a projection, then the libraries
    /// the document declares, which stand together under their root key.
    pub fn projections(&self) -> impl Iterator<Item = (gid::CellId, &str)> + '_ {
        self.libraries
            .named()
            .filter(|(library, _)| self.partials.iter().any(|(id, _)| id == library))
            .chain([(workspace::vocabulary::LIBRARIES, "document libraries")])
    }
}

/// The libraries Plain reads through, for the editor and the tutorial page
/// alike: names, text, bytes and absent values read as themselves, and
/// numbers as numbers, but nothing else a library draws, so calls,
/// arithmetic included, stay records.
pub const PLAIN: [gid::CellId; 7] = [
    name::ID,
    text::ID,
    blob::ID,
    absent::ID,
    f32::ID,
    f64::ID,
    u64::ID,
];

impl Stack<crate::Editor> {
    /// Plain's projection, from those of its libraries this stack loads.
    pub fn plain(&self) -> Projection<crate::Editor> {
        Projection::new(
            PLAIN
                .into_iter()
                .filter_map(|library| self.plain_reading(library)),
        )
    }

    /// A number library draws its numbers but not its arithmetic; Plain's
    /// other libraries draw as they always do.
    fn plain_reading(
        &self,
        library: gid::CellId,
    ) -> Option<crate::display::Partial<crate::Editor, Hovered>> {
        let (_, partial) = self.partials.iter().find(|(id, _)| *id == library)?;
        Some(match library {
            f32::ID => crate::display::runtime_partial(|input| f32::convention().display(input)),
            f64::ID => crate::display::runtime_partial(|input| f64::convention().display(input)),
            u64::ID => crate::display::runtime_partial(|input| u64::convention().display(input)),
            _ => partial.clone(),
        })
    }

    /// The document and pane projections with `hidden` libraries' own
    /// projections left off.
    pub fn without(
        &self,
        hidden: &[gid::CellId],
    ) -> (Projection<crate::Editor>, Projection<crate::Editor>) {
        projections(
            self.partials
                .iter()
                .filter(|(id, _)| !hidden.contains(id))
                .map(|(_, partial)| partial.clone()),
            !hidden.contains(&workspace::vocabulary::LIBRARIES),
        )
    }
}

pub fn load() -> Stack<crate::Editor> {
    compose(BUILT_INS.iter().map(|(id, build)| (*id, build())))
}

#[cfg(any(test, target_arch = "wasm32"))]
pub fn load_selected(ids: &[gid::CellId]) -> Result<Stack<crate::Editor>, String> {
    ids.iter()
        .map(|id| {
            BUILT_INS
                .iter()
                .find(|(candidate, _)| candidate == id)
                .map(|(_, build)| (*id, build()))
                .ok_or_else(|| format!("Unknown built-in library: {id}"))
        })
        .collect::<Result<Vec<_>, _>>()
        .map(compose)
}

fn compose(
    contributions: impl IntoIterator<Item = (gid::CellId, Library<crate::Editor, Hovered>)>,
) -> Stack<crate::Editor> {
    let (libraries, partials, providers) = Libraries::from_contributions(contributions);
    let completions = crate::libraries::completion::combine(providers);
    let partials: Rc<[_]> = partials.into();
    let (projection, pane_projection) =
        projections(partials.iter().map(|(_, partial)| partial.clone()), true);
    Stack {
        libraries,
        pane_projection,
        projection,
        completions,
        partials,
    }
}

/// Libraries' projections composed for the document, and for a pane, whose
/// entry may present its value instead.
fn projections(
    partials: impl IntoIterator<Item = crate::display::Partial<crate::Editor, Hovered>>,
    document_libraries: bool,
) -> (Projection<crate::Editor>, Projection<crate::Editor>) {
    let partials: Vec<_> = partials.into_iter().collect();
    let loaded = crate::display::compose_partials(partials.clone());
    let projection = Projection::new(
        partials
            .into_iter()
            .chain(document_libraries.then(|| presentation::document_libraries(loaded))),
    );
    let pane = projection
        .clone()
        .with_entry(crate::display::runtime_partial(
            presentation::projected_display,
        ));
    (projection, pane)
}

type BuildLibrary = fn() -> Library<crate::Editor, Hovered>;

const BUILT_INS: &[(gid::CellId, BuildLibrary)] = &[
    (name::ID, name::library),
    (text::ID, text::library),
    (blob::ID, blob::library),
    (png::ID, png::library),
    (absent::ID, absent::library),
    (color::ID, color::library),
    (control::ID, control::library),
    (controls::ID, controls::library),
    (tree::ID, tree::library),
    (number::ID, number::library),
    (f32::ID, f32::library),
    (f64::ID, f64::library),
    (u64::ID, u64::library),
    (conversion::ID, conversion::library),
    (fidget::ID, fidget::library),
    (toolpath::ID, toolpath::library),
    (grap_library::ID, grap_library::library),
    (line_edit::ID, line_edit::library),
    (logic::ID, logic::library),
    (list::ID, list::library),
    (sequence::ID, sequence::library),
    (random::ID, random::library),
    (presentation::ID, presentation::library),
    (layout::ID, layout::library),
    (selection::ID, selection::library),
    (crate::libraries::path::ID, crate::libraries::path::library),
    (site::ID, site::library),
    (geometry::ID, geometry::library),
    (workspace::ID, workspace::library),
];

#[cfg(test)]
mod tests;
