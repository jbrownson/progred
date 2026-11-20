//! What a library contributes to the editor, and the ordered set of loaded
//! libraries a document reads through. The built-in libraries in
//! [`crate::libraries`] are contributions like any other.

use crate::display::{Completion, CompletionProvider, CompletionRequest, Partial};
use ::grap::{ForeignFunction, ForeignFunctions};
use gid::Cells;
use std::rc::Rc;

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

/// Writes into the record of the library `id` the cells it says are its
/// `data` and its `absents`, which its description lists as it declares
/// them.
fn declare(
    definitions: &mut Definitions,
    id: gid::CellId,
    data: &[gid::CellId],
    absents: &[gid::CellId],
) {
    use crate::libraries::def::vocabulary::{ABSENTS, DATA};
    if data.is_empty() && absents.is_empty() {
        return;
    }
    let mut record = definitions
        .value(id)
        .and_then(gid::Value::as_record)
        .cloned()
        .unwrap_or_default();
    for (list, cells) in [(DATA, data), (ABSENTS, absents)] {
        record.set(
            list,
            gid::Value::list(cells.iter().copied().map(gid::Value::from)),
        );
    }
    definitions.insert(id, gid::Value::Record(record));
}

pub struct Library<World, Hover> {
    pub definitions: Definitions,
    pub projection: Option<Partial<World, Hover>>,
    /// How this library draws what no library's projection draws, tried
    /// after all of them and before Raw.
    pub fallback: Option<Partial<World, Hover>>,
    /// The projections this library's cells provide for the records they
    /// identify: a kind's for records of it, a function's for calls of it.
    pub provided: Vec<(gid::CellId, Partial<World, Hover>)>,
    pub completions: Option<CompletionProvider>,
    /// The cells it defines to be used as values, which its description
    /// lists as its data.
    pub data: Vec<gid::CellId>,
    /// The reasons the absents it gives say why, which its description
    /// lists as its absents.
    pub absents: Vec<gid::CellId>,
}

impl<World, Hover> Clone for Library<World, Hover> {
    fn clone(&self) -> Self {
        Self {
            definitions: self.definitions.clone(),
            projection: self.projection.clone(),
            fallback: self.fallback.clone(),
            provided: self.provided.clone(),
            completions: self.completions.clone(),
            data: self.data.clone(),
            absents: self.absents.clone(),
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
            fallback: None,
            provided: Vec::new(),
            completions: None,
            data: Vec::new(),
            absents: Vec::new(),
        }
    }

    /// Also defines each `(cell, value)` in `values`, data to be used as
    /// it's defined.
    pub fn with_values(
        mut self,
        values: impl IntoIterator<Item = (gid::CellId, gid::Value)>,
    ) -> Self {
        for (cell, value) in values {
            self.definitions.insert(cell, value);
            self.data.push(cell);
        }
        self
    }

    /// Also defines each `(cell, name, value)` in `constants`: `value`, named
    /// `name`, as Grap's value wrapper holds it, `{isa: value wrapper, name,
    /// value}`. The name is the cell's, and evaluating the cell gives the
    /// value alone, as a literal would be, not one that says it's the
    /// constant.
    pub fn with_constants<'a>(
        self,
        constants: impl IntoIterator<Item = (gid::CellId, &'a str, gid::Value)>,
    ) -> Self {
        self.with_values(constants.into_iter().map(|(cell, name, value)| {
            (
                cell,
                ::grap::isa::record(
                    ::grap::vocabulary::kind::VALUE_WRAPPER,
                    [
                        (
                            crate::libraries::name::vocabulary::NAME,
                            ::grap::text::value(name),
                        ),
                        (::grap::vocabulary::VALUE, value),
                    ],
                ),
            )
        }))
    }

    /// Says that `cells`, which it names, are data: used as values, as the
    /// options a field picks from are.
    pub fn with_data(mut self, cells: impl IntoIterator<Item = gid::CellId>) -> Self {
        self.data.extend(cells);
        self
    }

    /// Says that `cells`, which it names, are the reasons its absents give.
    pub fn with_absents(mut self, cells: impl IntoIterator<Item = gid::CellId>) -> Self {
        self.absents.extend(cells);
        self
    }

    pub fn with_projection(mut self, projection: Partial<World, Hover>) -> Self {
        self.projection = Some(projection);
        self
    }

    /// Also defines each of `kinds` as a kind, `{isa: def, name, fields}`:
    /// its name as this library names it, and the fields every record of it
    /// has, in order. How it's drawn stays this library's.
    pub fn with_kinds(
        mut self,
        kinds: impl IntoIterator<Item = (gid::CellId, Vec<gid::CellId>)>,
    ) -> Self {
        use crate::libraries::def::vocabulary::{DEF, FIELDS};
        for (kind, fields) in kinds {
            let mut definition = self
                .definitions
                .value(kind)
                .and_then(gid::Value::as_record)
                .cloned()
                .unwrap_or_default();
            definition.set(::grap::isa::ISA, gid::Value::from(DEF));
            definition.set(
                FIELDS,
                gid::Value::list(fields.into_iter().map(gid::Value::from)),
            );
            self.definitions
                .insert(kind, gid::Value::Record(definition));
        }
        self
    }

    /// Also draws what no library's projection draws with `fallback`.
    pub fn with_fallback(mut self, fallback: Partial<World, Hover>) -> Self {
        self.fallback = Some(fallback);
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
    /// The libraries `entries` contribute, a later one of the same identity
    /// replacing an earlier, with their projections in load order and then
    /// their fallbacks, the projections their cells provide, and their
    /// completion providers.
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
        let mut fallbacks = Vec::new();
        let (libraries, mut projections, provided, completions) = unique.into_iter().fold(
            (Self::default(), Vec::new(), Vec::new(), Vec::new()),
            |(mut libraries, mut projections, mut provided, mut completions), (id, library)| {
                let mut definitions = library.definitions;
                declare(&mut definitions, id, &library.data, &library.absents);
                libraries.insert(id, definitions);
                projections.extend(library.projection.map(|projection| (id, projection)));
                fallbacks.extend(library.fallback.map(|fallback| (id, fallback)));
                provided.extend(
                    library
                        .provided
                        .into_iter()
                        .map(|(cell, projection)| (id, cell, projection)),
                );
                completions.extend(library.completions.map(|provider| (id, provider)));
                (libraries, projections, provided, completions)
            },
        );
        // Fallbacks come after every library's own projection.
        projections.extend(fallbacks);
        (libraries, projections, provided, completions)
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
