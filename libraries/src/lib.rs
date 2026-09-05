//! Progred's built-in libraries. Each module owns one conceptual
//! library and exports its complete contribution to the editor.

use gid::Cells;
use grap_runtime::ForeignFunctions;
use progred_display::{Completion, Partial};
use std::rc::Rc;

pub mod absent;
pub mod color;
#[cfg(test)]
mod conformance;
pub mod control;
pub mod f32;
pub mod f64;
pub mod fidget;
pub mod geometry;
pub mod grap;
pub mod layout;
pub mod line_edit;
pub mod list;
pub mod logic;
pub mod name;
pub mod number;
pub mod presentation;
pub mod random;
pub mod selection;
pub mod site;
pub mod text;
pub mod u64;

#[cfg(test)]
pub(crate) fn test_evaluate(
    expression: &gid::Value,
    resolve: impl Fn(gid::CellId) -> Option<gid::Value>,
    foreign: &ForeignFunctions,
    fuel: usize,
) -> grap_runtime::Evaluation {
    grap_runtime::evaluate(
        expression,
        |cell| {
            foreign
                .get(cell)
                .cloned()
                .map(grap_runtime::Definition::ForeignFunction)
                .into_iter()
                .chain(resolve(cell).map(grap_runtime::Definition::Value))
                .map(|definition| (gid::Resolution::Document, definition))
                .collect()
        },
        fuel,
    )
}

#[cfg(test)]
pub(crate) fn test_apply(
    function: &gid::Value,
    arguments: impl IntoIterator<Item = (gid::CellId, gid::Value)>,
    resolve: impl Fn(gid::CellId) -> Option<gid::Value>,
    foreign: &ForeignFunctions,
    fuel: usize,
) -> grap_runtime::Evaluation {
    grap_runtime::apply(
        function,
        arguments,
        |cell| {
            foreign
                .get(cell)
                .cloned()
                .map(grap_runtime::Definition::ForeignFunction)
                .into_iter()
                .chain(resolve(cell).map(grap_runtime::Definition::Value))
                .map(|definition| (gid::Resolution::Document, definition))
                .collect()
        },
        fuel,
    )
}

#[derive(Clone, Copy)]
pub enum DefinitionRef<'a> {
    ForeignFunction(&'a grap_runtime::ForeignFunction),
    Value(&'a gid::Value),
}

impl DefinitionRef<'_> {
    pub fn cloned(self) -> grap_runtime::Definition {
        match self {
            Self::ForeignFunction(function) => {
                grap_runtime::Definition::ForeignFunction(function.clone())
            }
            Self::Value(value) => grap_runtime::Definition::Value(value.clone()),
        }
    }
}

#[derive(Clone, Copy)]
pub struct LocatedDefinition<'a> {
    pub library: gid::CellId,
    pub definition: DefinitionRef<'a>,
}

#[derive(Clone, Copy)]
pub struct LocatedValue<'a> {
    pub library: gid::CellId,
    pub value: &'a gid::Value,
}

#[derive(Clone, Default)]
pub struct Definitions {
    entries: Vec<(gid::CellId, Vec<grap_runtime::Definition>)>,
}

impl Definitions {
    pub fn from_parts(cells: Cells, functions: ForeignFunctions) -> Self {
        functions
            .iter()
            .map(|(cell, function)| {
                (
                    cell,
                    grap_runtime::Definition::ForeignFunction(function.clone()),
                )
            })
            .chain(
                cells
                    .iter()
                    .map(|(cell, value)| (*cell, grap_runtime::Definition::Value(value.clone()))),
            )
            .fold(Self::default(), |mut definitions, (cell, definition)| {
                definitions.insert(cell, definition);
                definitions
            })
    }

    pub fn insert(&mut self, cell: gid::CellId, definition: grap_runtime::Definition) {
        match self.entries.binary_search_by_key(&cell, |(cell, _)| *cell) {
            Ok(index) => match definition {
                grap_runtime::Definition::Value(value) => {
                    match self.entries[index]
                        .1
                        .iter_mut()
                        .find(|definition| matches!(definition, grap_runtime::Definition::Value(_)))
                    {
                        Some(definition) => *definition = grap_runtime::Definition::Value(value),
                        None => self.entries[index]
                            .1
                            .push(grap_runtime::Definition::Value(value)),
                    }
                }
                grap_runtime::Definition::ForeignFunction(function) => self.entries[index]
                    .1
                    .push(grap_runtime::Definition::ForeignFunction(function)),
            },
            Err(index) => self.entries.insert(index, (cell, vec![definition])),
        }
    }

    pub fn get(&self, cell: gid::CellId) -> &[grap_runtime::Definition] {
        self.entries
            .binary_search_by_key(&cell, |(cell, _)| *cell)
            .ok()
            .map(|index| self.entries[index].1.as_slice())
            .unwrap_or_default()
    }

    #[cfg(test)]
    pub fn value(&self, cell: gid::CellId) -> Option<&gid::Value> {
        self.get(cell)
            .iter()
            .find_map(|definition| match definition {
                grap_runtime::Definition::Value(value) => Some(value),
                grap_runtime::Definition::ForeignFunction(_) => None,
            })
    }

    #[cfg(test)]
    pub fn values(&self) -> impl Iterator<Item = (gid::CellId, &gid::Value)> {
        self.entries.iter().flat_map(|(cell, definitions)| {
            definitions
                .iter()
                .filter_map(|definition| match definition {
                    grap_runtime::Definition::Value(value) => Some((*cell, value)),
                    grap_runtime::Definition::ForeignFunction(_) => None,
                })
        })
    }

    #[cfg(test)]
    pub fn functions(&self) -> ForeignFunctions {
        self.entries
            .iter()
            .flat_map(|(cell, definitions)| {
                definitions
                    .iter()
                    .filter_map(|definition| match definition {
                        grap_runtime::Definition::ForeignFunction(function) => {
                            Some((*cell, function))
                        }
                        grap_runtime::Definition::Value(_) => None,
                    })
            })
            .fold(
                ForeignFunctions::default(),
                |functions, (cell, function)| {
                    if functions.get(cell).is_some() {
                        functions
                    } else {
                        functions.register(cell, function.clone())
                    }
                },
            )
    }
}

pub struct Library<World, Hover> {
    pub metadata: gid::Value,
    pub definitions: Definitions,
    pub projections: Vec<Partial<World, Hover>>,
    pub root_completions: Vec<Completion>,
    pub root_field_completions: Vec<Completion>,
}

impl<World, Hover> Clone for Library<World, Hover> {
    fn clone(&self) -> Self {
        Self {
            metadata: self.metadata.clone(),
            definitions: self.definitions.clone(),
            projections: self.projections.clone(),
            root_completions: self.root_completions.clone(),
            root_field_completions: self.root_field_completions.clone(),
        }
    }
}

impl<World, Hover> Default for Library<World, Hover> {
    fn default() -> Self {
        Self {
            metadata: gid::Value::record([]),
            definitions: Definitions::default(),
            projections: Vec::new(),
            root_completions: Vec::new(),
            root_field_completions: Vec::new(),
        }
    }
}

impl<World, Hover> Library<World, Hover> {
    pub fn new(
        metadata: gid::Value,
        definitions: Definitions,
        projections: Vec<Partial<World, Hover>>,
    ) -> Self {
        Self {
            metadata,
            definitions,
            projections,
            root_completions: Vec::new(),
            root_field_completions: Vec::new(),
        }
    }

    pub fn named(
        name: impl Into<String>,
        definitions: Definitions,
        projections: Vec<Partial<World, Hover>>,
    ) -> Self {
        Self::new(crate::name::record(name, []), definitions, projections)
    }

    pub fn with_root_completions(
        mut self,
        completions: impl IntoIterator<Item = Completion>,
    ) -> Self {
        self.root_completions = completions.into_iter().collect();
        self
    }

    pub fn with_root_field_completions(
        mut self,
        completions: impl IntoIterator<Item = Completion>,
    ) -> Self {
        self.root_field_completions = completions.into_iter().collect();
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
/// now; insertion replaces that identity in place. Each library is an
/// ordered multimap from cell identity to contributed definitions.
#[derive(Clone, Default)]
pub struct Libraries {
    entries: Rc<Vec<(gid::CellId, gid::Value, Definitions)>>,
}

impl Libraries {
    pub fn from_contributions<World, Hover>(
        entries: impl IntoIterator<Item = (gid::CellId, Library<World, Hover>)>,
    ) -> (
        Self,
        Vec<Partial<World, Hover>>,
        Vec<Completion>,
        Vec<Completion>,
    ) {
        entries.into_iter().fold(
            (Self::default(), Vec::new(), Vec::new(), Vec::new()),
            |(mut libraries, mut projections, mut root_completions, mut root_field_completions),
             (id, library)| {
                libraries.insert(id, library.metadata, library.definitions);
                projections.extend(library.projections);
                root_completions.extend(library.root_completions);
                root_field_completions.extend(library.root_field_completions);
                (
                    libraries,
                    projections,
                    root_completions,
                    root_field_completions,
                )
            },
        )
    }

    pub fn insert(
        &mut self,
        id: gid::CellId,
        metadata: gid::Value,
        definitions: Definitions,
    ) -> Option<(gid::Value, Definitions)> {
        let entries = Rc::make_mut(&mut self.entries);
        match entries
            .iter()
            .position(|(candidate, _, _)| *candidate == id)
        {
            Some(index) => {
                let entry = &mut entries[index];
                Some((
                    std::mem::replace(&mut entry.1, metadata),
                    std::mem::replace(&mut entry.2, definitions),
                ))
            }
            None => {
                entries.push((id, metadata, definitions));
                None
            }
        }
    }

    pub fn get(&self, id: gid::CellId) -> Option<&Definitions> {
        self.entries
            .iter()
            .find(|(candidate, _, _)| *candidate == id)
            .map(|(_, _, definitions)| definitions)
    }

    pub fn metadata(&self, id: gid::CellId) -> Option<&gid::Value> {
        self.entries
            .iter()
            .find(|(candidate, _, _)| *candidate == id)
            .map(|(_, metadata, _)| metadata)
    }

    pub fn iter(&self) -> impl Iterator<Item = (gid::CellId, &Definitions)> {
        self.entries
            .iter()
            .map(|(id, _, definitions)| (*id, definitions))
    }

    pub fn definitions(&self, cell: gid::CellId) -> impl Iterator<Item = LocatedDefinition<'_>> {
        self.entries
            .iter()
            .flat_map(move |(library_id, _, definitions)| {
                definitions
                    .get(cell)
                    .iter()
                    .map(|definition| LocatedDefinition {
                        library: *library_id,
                        definition: match definition {
                            grap_runtime::Definition::ForeignFunction(function) => {
                                DefinitionRef::ForeignFunction(function)
                            }
                            grap_runtime::Definition::Value(value) => DefinitionRef::Value(value),
                        },
                    })
            })
    }

    pub fn values(&self, cell: gid::CellId) -> impl Iterator<Item = LocatedValue<'_>> {
        self.entries
            .iter()
            .flat_map(move |(library, _, definitions)| {
                definitions
                    .get(cell)
                    .iter()
                    .filter_map(|definition| match definition {
                        grap_runtime::Definition::Value(value) => Some(LocatedValue {
                            library: *library,
                            value,
                        }),
                        grap_runtime::Definition::ForeignFunction(_) => None,
                    })
            })
    }

    pub fn first_value(&self, cell: gid::CellId) -> Option<&gid::Value> {
        self.values(cell).next().map(|definition| definition.value)
    }

    pub fn cell_ids(&self) -> impl Iterator<Item = gid::CellId> + '_ {
        self.entries
            .iter()
            .flat_map(|(_, _, definitions)| definitions.entries.iter().map(|(cell, _)| *cell))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::{CellId, Value};
    use grap_runtime::{Environment, Expression, ForeignFunction, Halt};
    use progred_display::{Env, Layout, ProjectionInput, text as text_layout};
    use puri::Leaf;
    use std::rc::Rc;

    const SHARED_CELL: CellId = CellId::from_u128(1);
    const SHARED_FUNCTION: CellId = CellId::from_u128(2);
    const LEFT_LIBRARY: CellId = CellId::from_u128(3);
    const RIGHT_LIBRARY: CellId = CellId::from_u128(4);

    fn left_function(
        _: &mut grap_runtime::Context,
        _: Expression,
        _: &Environment,
    ) -> Result<Value, Halt> {
        Ok(Value::from(b"left".to_vec()))
    }

    fn right_function(
        _: &mut grap_runtime::Context,
        _: Expression,
        _: &Environment,
    ) -> Result<Value, Halt> {
        Ok(Value::from(b"right".to_vec()))
    }

    fn left_projection(_: &ProjectionInput<'_, (), ()>) -> Option<Layout<(), ()>> {
        Some(text_layout("left"))
    }

    fn right_projection(_: &ProjectionInput<'_, (), ()>) -> Option<Layout<(), ()>> {
        Some(text_layout("right"))
    }

    struct NoEval;

    impl Env for NoEval {
        fn apply(&self, _: &gid::Value, _: &[(gid::CellId, gid::Value)]) -> (gid::Value, usize) {
            panic!("unexpected projection application")
        }

        fn evaluate(&self, _: &Value) -> (Value, usize) {
            (Value::record([]), 0)
        }
    }

    #[test]
    fn library_definitions_and_projections_compose_in_order() {
        let mut left_cells = Cells::new();
        left_cells.set_value(SHARED_CELL, Value::from(b"left".to_vec()));
        let mut right_cells = Cells::new();
        right_cells.set_value(SHARED_CELL, Value::from(b"right".to_vec()));
        let (libraries, projections, _, _) = Libraries::from_contributions([
            (
                LEFT_LIBRARY,
                Library::named(
                    "left",
                    Definitions::from_parts(
                        left_cells,
                        ForeignFunctions::default()
                            .register(SHARED_FUNCTION, ForeignFunction::new(left_function)),
                    ),
                    vec![progred_display::partial(left_projection)],
                ),
            ),
            (
                RIGHT_LIBRARY,
                Library::named(
                    "right",
                    Definitions::from_parts(
                        right_cells,
                        ForeignFunctions::default()
                            .register(SHARED_FUNCTION, ForeignFunction::new(right_function)),
                    ),
                    vec![progred_display::partial(right_projection)],
                ),
            ),
        ]);

        assert_eq!(
            libraries
                .values(SHARED_CELL)
                .map(|definition| (definition.library, definition.value.clone()))
                .collect::<Vec<_>>(),
            [
                (LEFT_LIBRARY, Value::from(b"left".to_vec())),
                (RIGHT_LIBRARY, Value::from(b"right".to_vec()))
            ]
        );
        assert_eq!(
            grap_runtime::evaluate(
                &grap_runtime::call(Value::from(SHARED_FUNCTION), []),
                |cell| libraries
                    .definitions(cell)
                    .map(|definition| (
                        gid::Resolution::Library(definition.library),
                        definition.definition.cloned()
                    ))
                    .collect(),
                10,
            )
            .result,
            Value::from(b"left".to_vec())
        );
        let value = Value::record([]);
        assert_eq!(
            projections
                .iter()
                .map(|projection| {
                    let target = |_| progred_display::ProjectionTarget {
                        select: Rc::new(|_: &mut ()| false),
                        select_with: Rc::new(|_: &mut (), _| false),
                        hover: (),
                    };
                    projection(&ProjectionInput {
                        env: &NoEval,
                        value: &value,
                        scale_factor: 1.0,
                        writable: true,
                        selection: None,
                        pending: None,
                        state: None,
                        targets: progred_display::ProjectionTargets::new(&target),
                    })
                    .and_then(|layout| match layout {
                        Layout::Leaf(Leaf::Text { text, .. }) => Some(text),
                        _ => None,
                    })
                    .unwrap()
                })
                .collect::<Vec<_>>(),
            ["left", "right"]
        );
    }

    #[test]
    fn libraries_are_an_ordered_unique_map_without_hashing() {
        let mut left_cells = Cells::new();
        left_cells.set_value(SHARED_CELL, Value::from(b"left".to_vec()));
        let left = Library::<(), ()>::named(
            "left",
            Definitions::from_parts(
                left_cells,
                ForeignFunctions::default()
                    .register(SHARED_CELL, ForeignFunction::new(left_function)),
            ),
            vec![],
        );
        let mut right_cells = Cells::new();
        right_cells.set_value(SHARED_CELL, Value::from(b"right".to_vec()));
        let right = Library::<(), ()>::named(
            "right",
            Definitions::from_parts(right_cells, ForeignFunctions::default()),
            vec![],
        );
        let (mut libraries, _, _, _) =
            Libraries::from_contributions([(LEFT_LIBRARY, left), (RIGHT_LIBRARY, right)]);
        let replacement = Definitions::default();

        assert!(libraries.get(LEFT_LIBRARY).is_some());
        assert_eq!(
            libraries.metadata(LEFT_LIBRARY).and_then(crate::name::read),
            Some("left")
        );
        assert_eq!(
            libraries
                .definitions(SHARED_CELL)
                .map(|definition| definition.library)
                .collect::<Vec<_>>(),
            [LEFT_LIBRARY, LEFT_LIBRARY, RIGHT_LIBRARY]
        );
        assert_eq!(
            libraries
                .values(SHARED_CELL)
                .map(|definition| definition.library)
                .collect::<Vec<_>>(),
            [LEFT_LIBRARY, RIGHT_LIBRARY]
        );
        assert!(
            libraries
                .insert(
                    LEFT_LIBRARY,
                    crate::name::record("replacement", []),
                    replacement
                )
                .is_some()
        );
        assert_eq!(
            libraries.iter().map(|(id, _)| id).collect::<Vec<_>>(),
            [LEFT_LIBRARY, RIGHT_LIBRARY]
        );
    }
}
