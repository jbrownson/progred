//! The editor's ordered composition of Progred libraries.

use crate::frame::Hovered;
use crate::libraries::{
    Libraries, Library, absent, blob, color, control, controls, f32, f64, fidget, geometry,
    grap as grap_library, layout, line_edit, list, logic, name, number, presentation, random,
    selection, site, text, toolpath, tree, u64, workspace,
};
use crate::projection::Projection;

pub struct Stack<World> {
    pub libraries: Libraries,
    pub projection: Projection<World>,
    pub pane_projection: Projection<World>,
    pub completions: crate::display::CompletionProvider,
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
    let (libraries, projections, providers) = Libraries::from_contributions(contributions);
    let completions = crate::libraries::completion::combine(providers);
    let projection = Projection::new(projections);
    Stack {
        libraries,
        pane_projection: projection
            .clone()
            .with_entry(crate::display::partial(presentation::projected_display)),
        projection,
        completions,
    }
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
    (fidget::ID, fidget::library),
    (toolpath::ID, toolpath::library),
    (grap_library::ID, grap_library::library),
    (line_edit::ID, line_edit::library),
    (logic::ID, logic::library),
    (list::ID, list::library),
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
}
