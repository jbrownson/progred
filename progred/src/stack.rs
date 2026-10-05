//! The editor's ordered composition of Progred libraries.

use crate::frame::Hovered;
use crate::libraries::{
    Libraries, Library, absent, blob, color, control, controls, conversion, f32, f64, fidget,
    geometry, grap as grap_library, layout, line_edit, list, logic, name, number, presentation,
    random, selection, sequence, site, text, toolpath, tree, u64, workspace,
};
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
    /// Each library that brings a projection, and its name, in load order.
    pub fn projections(&self) -> impl Iterator<Item = (gid::CellId, &str)> + '_ {
        self.libraries
            .named()
            .filter(|(library, _)| self.partials.iter().any(|(id, _)| id == library))
    }
}

impl Stack<crate::Editor> {
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
fn projections(
    partials: impl IntoIterator<Item = crate::display::Partial<crate::Editor, Hovered>>,
) -> (Projection<crate::Editor>, Projection<crate::Editor>) {
    let partials: Vec<_> = partials.into_iter().collect();
    let loaded = crate::display::compose_partials(partials.clone());
    let projection = Projection::new(
        partials
            .into_iter()
            .chain([presentation::document_libraries(loaded)]),
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
mod tests {
    use super::*;

    #[test]
    fn selected_libraries_supply_only_their_own_definitions() {
        let ids = [name::ID, text::ID, blob::ID, number::ID, f64::ID];
        let stack = load_selected(&ids).unwrap();
        assert_eq!(
            stack.libraries.iter().map(|(id, _)| id).collect::<Vec<_>>(),
            ids
        );
        assert!(stack.libraries.resolve(f64::vocabulary::F64).is_some());
        assert!(stack.libraries.resolve(f32::vocabulary::F32).is_none());
        for id in [fidget::ID, toolpath::ID, control::ID, grap_library::ID] {
            assert!(stack.libraries.resolve(id).is_none());
        }
    }

    #[test]
    fn explicit_order_and_empty_selection_are_preserved_and_unknown_ids_fail() {
        let stack = load_selected(&[text::ID, name::ID, text::ID]).unwrap();
        assert_eq!(
            stack.libraries.iter().map(|(id, _)| id).collect::<Vec<_>>(),
            [text::ID, name::ID]
        );
        assert_eq!(load_selected(&[]).unwrap().libraries.iter().count(), 0);
        assert!(load_selected(&[gid::new_cell_id()]).is_err());
        assert_eq!(load().libraries.iter().count(), BUILT_INS.len());
    }

    #[test]
    fn only_libraries_that_bring_a_projection_offer_one() {
        let stack = load_selected(&[name::ID, text::ID, random::ID, color::ID]).unwrap();
        assert_eq!(
            stack.projections().collect::<Vec<_>>(),
            [(text::ID, "text"), (color::ID, "color")]
        );
    }
}
