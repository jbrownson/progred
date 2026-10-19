//! The editor's ordered composition of Progred libraries.

use crate::frame::Hovered;
use crate::libraries::{
    absent, blob, color, control, controls, conversion, declared, f32, f64, fidget, geometry,
    grap as grap_library, layout, list, logic, name, number, png, presentation, random, selection,
    sequence, site, text, toolpath, tree, u64, workspace,
};
use crate::library::{Libraries, Library};
use crate::projection::Partial;
use crate::projection::Projection;
use std::rc::Rc;

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

#[derive(Clone)]
pub struct Stack {
    pub libraries: Libraries,
    pub projection: Projection,
    pub pane_projection: Projection,
    pub completions: crate::display::CompletionProvider,
    /// The projections libraries bring, in load order, so an area can leave
    /// some of them off.
    partials: Rc<[(gid::CellId, Partial)]>,
}

impl Stack {
    /// Each library that brings a projection, and its name, in the order
    /// projections are tried.
    pub fn projections(&self) -> impl Iterator<Item = (gid::CellId, &str)> + '_ {
        crate::libraries::name::library_names(&self.libraries)
            .filter(|(library, _)| self.partials.iter().any(|(id, _)| id == library))
    }

    /// Plain's projection, from those of its libraries this stack loads.
    pub fn plain(&self) -> Projection {
        Projection::new(
            PLAIN
                .into_iter()
                .filter_map(|library| self.plain_reading(library)),
        )
    }

    /// A number library draws its numbers but not its arithmetic; Plain's
    /// other libraries draw as they always do.
    fn plain_reading(&self, library: gid::CellId) -> Option<Partial> {
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
    pub fn without(&self, hidden: &[gid::CellId]) -> (Projection, Projection) {
        projections(
            self.partials
                .iter()
                .filter(|(id, _)| !hidden.contains(id))
                .map(|(_, partial)| partial.clone()),
        )
    }
}

pub fn load() -> Stack {
    compose(BUILT_INS.iter().map(|(id, build)| (*id, build())))
}

#[cfg(any(test, target_arch = "wasm32"))]
pub fn load_selected(ids: &[gid::CellId]) -> Result<Stack, String> {
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
) -> Stack {
    let (libraries, partials, providers) = Libraries::from_contributions(contributions);
    let completions = crate::libraries::completion::combine(providers);
    let partials: Rc<[_]> = partials.into();
    let (projection, pane_projection) =
        projections(partials.iter().map(|(_, partial)| partial.clone()));
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
fn projections(partials: impl IntoIterator<Item = Partial>) -> (Projection, Projection) {
    let projection = Projection::new(partials);
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
    // Tried after every built-in library.
    (declared::ID, declared::library),
];

#[cfg(test)]
mod tests;
