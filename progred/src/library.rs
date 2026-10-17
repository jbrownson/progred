//! What a library contributes to the editor, and the ordered set of loaded
//! libraries a document reads through. The built-in libraries in
//! [`crate::libraries`] are contributions like any other.

use crate::display::{Completion, CompletionProvider, CompletionRequest, Partial};
use crate::libraries::name;
use ::grap::{Definition, ForeignFunctions};
use gid::Cells;
use std::rc::Rc;

#[derive(Clone, Default)]
pub struct Definitions {
    entries: Rc<Vec<(gid::CellId, Definition)>>,
}

impl Definitions {
    pub fn from_parts(cells: Cells, foreign: ForeignFunctions) -> Self {
        let mut entries: Vec<_> = cells
            .iter()
            .map(|(cell, value)| (*cell, Definition::Value(value.clone())))
            .collect();
        entries.sort_unstable_by_key(|(cell, _)| *cell);
        let mut definitions = Self {
            entries: Rc::new(entries),
        };
        for (cell, function) in foreign.iter() {
            definitions.insert(
                cell,
                Definition::foreign(
                    definitions
                        .value(cell)
                        .cloned()
                        .unwrap_or_else(|| gid::Value::record([])),
                    function.clone(),
                ),
            );
        }
        definitions
    }

    pub fn get(&self, cell: gid::CellId) -> Option<&Definition> {
        self.entries
            .binary_search_by_key(&cell, |(cell, _)| *cell)
            .ok()
            .map(|index| &self.entries[index].1)
    }

    pub fn value(&self, cell: gid::CellId) -> Option<&gid::Value> {
        self.get(cell).map(Definition::value)
    }

    pub fn insert(&mut self, cell: gid::CellId, definition: Definition) -> Option<Definition> {
        let entries = Rc::make_mut(&mut self.entries);
        match entries.binary_search_by_key(&cell, |(cell, _)| *cell) {
            Ok(index) => Some(std::mem::replace(&mut entries[index].1, definition)),
            Err(index) => {
                entries.insert(index, (cell, definition));
                None
            }
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (gid::CellId, &Definition)> {
        self.entries
            .iter()
            .map(|(cell, definition)| (*cell, definition))
    }

    #[cfg(test)]
    pub fn functions(&self) -> ForeignFunctions {
        self.iter().fold(
            ForeignFunctions::default(),
            |functions, (cell, definition)| match definition {
                Definition::Value(_) => functions,
                Definition::Foreign(native) => {
                    functions.register(cell, native.implementation.clone())
                }
            },
        )
    }
}

pub struct Library<World, Hover> {
    pub definitions: Definitions,
    pub projection: Option<Partial<World, Hover>>,
    pub completions: Option<CompletionProvider>,
}

impl<World, Hover> Clone for Library<World, Hover> {
    fn clone(&self) -> Self {
        Self {
            definitions: self.definitions.clone(),
            projection: self.projection.clone(),
            completions: self.completions.clone(),
        }
    }
}

impl<World, Hover> Default for Library<World, Hover> {
    fn default() -> Self {
        Self::new(Definitions::default())
    }
}

impl<World, Hover> Library<World, Hover> {
    pub fn new(definitions: Definitions) -> Self {
        Self {
            definitions,
            projection: None,
            completions: None,
        }
    }

    pub fn named(id: gid::CellId, name: impl Into<String>, mut definitions: Definitions) -> Self {
        definitions.insert(id, Definition::Value(name::record(name, [])));
        Self::new(definitions)
    }

    pub fn with_projection(mut self, projection: Partial<World, Hover>) -> Self {
        self.projection = Some(projection);
        self
    }

    pub fn with_completions(
        mut self,
        completions: impl Fn(&CompletionRequest<'_>) -> Option<Vec<Completion>> + 'static,
    ) -> Self {
        self.completions = Some(Rc::new(completions));
        self
    }

    #[cfg(test)]
    pub fn value(&self, cell: gid::CellId) -> Option<&gid::Value> {
        self.definitions.value(cell)
    }

    #[cfg(test)]
    pub fn functions(&self) -> ForeignFunctions {
        self.definitions.functions()
    }
}

/// The ordered loaded-library set. A library identity is unique for
/// now; insertion replaces that identity in place.
#[derive(Clone, Default)]
pub struct Libraries {
    entries: Rc<Vec<(gid::CellId, Definitions)>>,
}

impl Libraries {
    pub(crate) fn same_storage(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.entries, &other.entries)
    }
    pub fn from_contributions<World, Hover>(
        entries: impl IntoIterator<Item = (gid::CellId, Library<World, Hover>)>,
    ) -> (
        Self,
        Vec<(gid::CellId, Partial<World, Hover>)>,
        Vec<CompletionProvider>,
    ) {
        let mut unique: Vec<(gid::CellId, Library<World, Hover>)> = Vec::new();
        for (id, library) in entries {
            if let Some((_, previous)) = unique.iter_mut().find(|(key, _)| *key == id) {
                *previous = library;
            } else {
                unique.push((id, library));
            }
        }
        unique.into_iter().fold(
            (Self::default(), Vec::new(), Vec::new()),
            |(mut libraries, mut projections, mut completions), (id, library)| {
                libraries.insert(id, library.definitions);
                projections.extend(library.projection.map(|projection| (id, projection)));
                completions.extend(library.completions);
                (libraries, projections, completions)
            },
        )
    }

    pub fn insert(&mut self, id: gid::CellId, definitions: Definitions) -> Option<Definitions> {
        let entries = Rc::make_mut(&mut self.entries);
        match entries.iter().position(|(candidate, _)| *candidate == id) {
            Some(index) => {
                let entry = &mut entries[index];
                Some(std::mem::replace(&mut entry.1, definitions))
            }
            None => {
                entries.push((id, definitions));
                None
            }
        }
    }

    #[cfg(test)]
    pub fn get(&self, id: gid::CellId) -> Option<&Definitions> {
        self.entries
            .iter()
            .find(|(candidate, _)| *candidate == id)
            .map(|(_, definitions)| definitions)
    }

    #[cfg(test)]
    pub fn iter(&self) -> impl Iterator<Item = (gid::CellId, &Definitions)> {
        self.entries
            .iter()
            .map(|(id, definitions)| (*id, definitions))
    }

    /// The names libraries give themselves, in load order.
    #[cfg(any(test, target_arch = "wasm32"))]
    pub fn names(&self) -> impl Iterator<Item = &str> + '_ {
        self.named().map(|(_, name)| name)
    }

    /// Each library and the name it gives itself, in load order.
    pub fn named(&self) -> impl Iterator<Item = (gid::CellId, &str)> + '_ {
        self.entries.iter().filter_map(|(id, definitions)| {
            definitions
                .value(*id)
                .and_then(name::read)
                .map(|name| (*id, name))
        })
    }

    /// Each loaded library's definition of `cell`, in load order.
    pub fn definitions(
        &self,
        cell: gid::CellId,
    ) -> impl Iterator<Item = (gid::CellId, &Definition)> + '_ {
        self.entries
            .iter()
            .filter_map(move |(library, definitions)| {
                definitions
                    .get(cell)
                    .map(|definition| (*library, definition))
            })
    }

    #[cfg(test)]
    pub fn foreign_sources(&self, cell: gid::CellId) -> impl Iterator<Item = gid::CellId> + '_ {
        self.entries
            .iter()
            .filter_map(move |(library, definitions)| {
                matches!(definitions.get(cell), Some(Definition::Foreign(_))).then_some(*library)
            })
    }

    #[cfg(test)]
    pub fn first_value(&self, cell: gid::CellId) -> Option<&gid::Value> {
        self.definitions(cell)
            .next()
            .map(|(_, definition)| definition.value())
    }

    pub fn resolve(&self, cell: gid::CellId) -> Option<(gid::Resolution, ::grap::Definition)> {
        self.definitions(cell)
            .next()
            .map(|(library, definition)| (gid::Resolution::Library(library), definition.clone()))
    }

    pub fn cell_ids(&self) -> impl Iterator<Item = gid::CellId> + '_ {
        self.entries
            .iter()
            .flat_map(|(_, definitions)| definitions.iter().map(|(cell, _)| cell))
    }
}

impl ::grap::Host for Libraries {
    fn resolve(&self, cell: gid::CellId) -> Option<(gid::Resolution, ::grap::Definition)> {
        self.resolve(cell)
    }
}

#[cfg(test)]
mod tests;
