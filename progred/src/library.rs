//! What a library contributes to the editor, and the ordered set of loaded
//! libraries a document reads through. The built-in libraries in
//! [`crate::libraries`] are contributions like any other.

use crate::display::{Completion, CompletionProvider, CompletionRequest, Partial};
use ::grap::{ForeignFunction, ForeignFunctions};
use gid::Cells;
use std::rc::Rc;

/// What a loaded library's own cell holds, so a pane can show it: the
/// editor library names these.
pub mod vocabulary {
    use gid::CellId;

    /// The cells a library defines.
    pub const CELLS: CellId = CellId::from_u128(0xfccbc79141f24111bd0ba86b1fc775e6);

    pub mod kind {
        use gid::CellId;

        /// A loaded library: `{isa: loaded library, name, cells}`.
        pub const LIBRARY: CellId = CellId::from_u128(0x6d1ad922d3c24267ab64c33627349a19);
    }
}

/// What a library defines: each cell's value, and the native code the
/// foreign functions implemented under its cells run.
#[derive(Clone, Default)]
pub struct Definitions {
    values: Rc<Vec<(gid::CellId, gid::Value)>>,
    implementations: Rc<ForeignFunctions<'static>>,
}

impl Definitions {
    /// `cells`, with each of `functions` a foreign function implemented
    /// under its cell.
    pub fn from_parts(cells: Cells, functions: ForeignFunctions<'static>) -> Self {
        let mut values: Vec<_> = cells
            .iter()
            .map(|(cell, value)| (*cell, value.clone()))
            .collect();
        values.sort_unstable_by_key(|(cell, _)| *cell);
        let mut definitions = Self {
            values: Rc::new(values),
            implementations: Rc::default(),
        };
        for (cell, function) in functions.iter() {
            definitions.insert_foreign(cell, function.clone());
        }
        definitions
    }

    pub fn value(&self, cell: gid::CellId) -> Option<&gid::Value> {
        self.values
            .binary_search_by_key(&cell, |(cell, _)| *cell)
            .ok()
            .map(|index| &self.values[index].1)
    }

    /// The code the foreign function implemented under `cell` runs.
    pub fn implementation(&self, cell: gid::CellId) -> Option<&ForeignFunction<'static>> {
        self.implementations.get(cell)
    }

    #[cfg(test)]
    pub fn implementations(&self) -> &ForeignFunctions<'static> {
        &self.implementations
    }

    pub fn insert(&mut self, cell: gid::CellId, value: gid::Value) {
        let values = Rc::make_mut(&mut self.values);
        match values.binary_search_by_key(&cell, |(cell, _)| *cell) {
            Ok(index) => values[index].1 = value,
            Err(index) => values.insert(index, (cell, value)),
        }
    }

    /// Defines `cell` as a foreign function implemented under it, running
    /// `function`, keeping what its definition already says of it, such as
    /// its name.
    pub fn insert_foreign(&mut self, cell: gid::CellId, function: ForeignFunction<'static>) {
        self.insert(cell, foreign_record(cell, self.value(cell), &function));
        let implementations = Rc::make_mut(&mut self.implementations);
        *implementations = std::mem::take(implementations).register(cell, function);
    }

    pub fn iter(&self) -> impl Iterator<Item = (gid::CellId, &gid::Value)> {
        self.values.iter().map(|(cell, value)| (*cell, value))
    }
}

/// `cell`'s definition as a foreign function implemented under it, running
/// `function`: `description`'s fields, saying so, with the parameters
/// `function` declares unless they list their own.
pub fn foreign_record(
    cell: gid::CellId,
    description: Option<&gid::Value>,
    function: &ForeignFunction,
) -> gid::Value {
    let mut fields = description
        .and_then(gid::Value::as_record)
        .cloned()
        .unwrap_or_default();
    fields.set(
        ::grap::isa::ISA,
        gid::Value::from(::grap::vocabulary::kind::FOREIGN_FUNCTION),
    );
    fields.set(::grap::vocabulary::IMPLEMENTATION, gid::Value::from(cell));
    if let Some(parameters) = function.declared_parameters()
        && fields.get(&::grap::vocabulary::PARAMS).is_none()
    {
        fields.set(
            ::grap::vocabulary::PARAMS,
            gid::Value::list(parameters.iter().map(|&parameter| parameter.into())),
        );
    }
    gid::Value::Record(fields)
}

pub struct Library<World, Hover> {
    pub definitions: Definitions,
    pub projection: Option<Partial<World, Hover>>,
    /// The projections this library's cells provide for the records they
    /// identify: a kind's for records of it, a function's for calls of it.
    pub provided: Vec<(gid::CellId, Partial<World, Hover>)>,
    pub completions: Option<CompletionProvider>,
}

impl<World, Hover> Clone for Library<World, Hover> {
    fn clone(&self) -> Self {
        Self {
            definitions: self.definitions.clone(),
            projection: self.projection.clone(),
            provided: self.provided.clone(),
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
            provided: Vec::new(),
            completions: None,
        }
    }

    /// Also defines each `(cell, value)` in `values`.
    pub fn with_values(
        mut self,
        values: impl IntoIterator<Item = (gid::CellId, gid::Value)>,
    ) -> Self {
        for (cell, value) in values {
            self.definitions.insert(cell, value);
        }
        self
    }

    pub fn with_projection(mut self, projection: Partial<World, Hover>) -> Self {
        self.projection = Some(projection);
        self
    }

    /// Also draws the records each cell identifies with its projection.
    pub fn with_projections_for(
        mut self,
        provided: impl IntoIterator<Item = (gid::CellId, Partial<World, Hover>)>,
    ) -> Self {
        self.provided.extend(provided);
        self
    }

    pub fn with_completions(
        mut self,
        completions: impl Fn(&CompletionRequest<'_>) -> Option<Vec<Completion>> + 'static,
    ) -> Self {
        self.completions = Some(Rc::new(completions));
        self
    }
}

/// The ordered loaded-library set. A library identity is unique:
/// insertion replaces that identity in place.
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
        Vec<(gid::CellId, gid::CellId, Partial<World, Hover>)>,
        Vec<(gid::CellId, CompletionProvider)>,
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
            (Self::default(), Vec::new(), Vec::new(), Vec::new()),
            |(mut libraries, mut projections, mut provided, mut completions), (id, library)| {
                libraries.insert(id, library.definitions);
                projections.extend(library.projection.map(|projection| (id, projection)));
                provided.extend(
                    library
                        .provided
                        .into_iter()
                        .map(|(cell, projection)| (id, cell, projection)),
                );
                completions.extend(library.completions.map(|provider| (id, provider)));
                (libraries, projections, provided, completions)
            },
        )
    }

    pub fn insert(&mut self, id: gid::CellId, definitions: Definitions) {
        let entries = Rc::make_mut(&mut self.entries);
        match entries.iter_mut().find(|(candidate, _)| *candidate == id) {
            Some((_, existing)) => *existing = definitions,
            None => entries.push((id, definitions)),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (gid::CellId, &Definitions)> {
        self.entries
            .iter()
            .map(|(id, definitions)| (*id, definitions))
    }

    /// The first loaded library to define `cell`, and its definition.
    pub fn first_definition(&self, cell: gid::CellId) -> Option<(gid::CellId, &gid::Value)> {
        self.entries.iter().find_map(|(library, definitions)| {
            definitions.value(cell).map(|value| (*library, value))
        })
    }

    /// The code the first loaded library with a foreign function implemented
    /// under `cell` runs for it.
    pub fn implementation(&self, cell: gid::CellId) -> Option<&ForeignFunction<'static>> {
        self.entries
            .iter()
            .find_map(|(_, definitions)| definitions.implementation(cell))
    }

    pub fn cell_ids(&self) -> impl Iterator<Item = gid::CellId> + '_ {
        self.entries
            .iter()
            .flat_map(|(_, definitions)| definitions.iter().map(|(cell, _)| cell))
    }
}

/// Tests evaluate against libraries alone; the editor always reads through
/// [`crate::sources::Sources`].
#[cfg(test)]
impl ::grap::Host for Libraries {
    fn definition(&self, cell: gid::CellId) -> Option<gid::Value> {
        self.first_definition(cell).map(|(_, value)| value.clone())
    }

    fn implementation(&self, cell: gid::CellId) -> Option<ForeignFunction<'static>> {
        self.implementation(cell).cloned()
    }
}

#[cfg(test)]
#[path = "tests/library.rs"]
mod tests;
