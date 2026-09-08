//! Progred's built-in libraries. Each module owns one conceptual
//! library and exports its complete contribution to the editor.

use gid::Cells;
use grap_runtime::{Definition, ForeignFunctions};
use progred_display::{Completion, CompletionProvider, CompletionRequest, Partial};
use std::rc::Rc;

pub mod absent;
pub mod blob;
pub mod color;
pub mod completion;
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
pub mod path;
pub mod presentation;
pub mod random;
pub mod selection;
pub mod site;
pub mod text;
pub mod u64;
pub mod workspace;

#[cfg(test)]
pub(crate) struct TestHost<F>(pub F);

#[cfg(test)]
impl<F: Fn(gid::CellId) -> Vec<(gid::Resolution, grap_runtime::Definition)>> grap_runtime::Host
    for TestHost<F>
{
    fn resolve(&self, cell: gid::CellId) -> Option<(gid::Resolution, grap_runtime::Definition)> {
        (self.0)(cell).into_iter().next()
    }
}

#[cfg(test)]
fn test_host<'a>(
    resolve: impl Fn(gid::CellId) -> Option<gid::Value> + 'a,
    foreign: &'a ForeignFunctions,
) -> impl grap_runtime::Host + 'a {
    TestHost(move |cell| {
        match (resolve(cell), foreign.get(cell)) {
            (value, Some(function)) => Some(Definition::foreign(
                value.unwrap_or_else(|| gid::Value::record([])),
                function.clone(),
            )),
            (Some(value), None) => Some(Definition::Value(value)),
            (None, None) => None,
        }
        .map(|definition| (gid::Resolution::Document, definition))
        .into_iter()
        .collect()
    })
}

#[cfg(test)]
pub(crate) fn test_evaluate(
    expression: &gid::Value,
    resolve: impl Fn(gid::CellId) -> Option<gid::Value>,
    foreign: &ForeignFunctions,
    fuel: usize,
) -> grap_runtime::Evaluation {
    grap_runtime::evaluate(expression, &test_host(resolve, foreign), fuel)
}

#[cfg(test)]
pub(crate) fn test_apply(
    function: &gid::Value,
    arguments: impl IntoIterator<Item = (gid::CellId, gid::Value)>,
    resolve: impl Fn(gid::CellId) -> Option<gid::Value>,
    foreign: &ForeignFunctions,
    fuel: usize,
) -> grap_runtime::Evaluation {
    grap_runtime::apply(function, arguments, &test_host(resolve, foreign), fuel)
}

#[derive(Clone, Copy)]
pub struct LocatedValue<'a> {
    pub library: gid::CellId,
    pub value: &'a gid::Value,
}

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
    pub fn values(&self) -> impl Iterator<Item = (gid::CellId, &gid::Value)> {
        self.iter()
            .map(|(cell, definition)| (cell, definition.value()))
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
    pub projection: Partial<World, Hover>,
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
        Self {
            definitions: Definitions::default(),
            projection: progred_display::partial(|_| None),
            completions: None,
        }
    }
}

impl<World, Hover> Library<World, Hover> {
    pub fn new(definitions: Definitions, projection: Partial<World, Hover>) -> Self {
        Self {
            definitions,
            projection,
            completions: None,
        }
    }

    pub fn named(
        id: gid::CellId,
        name: impl Into<String>,
        mut definitions: Definitions,
        projection: Partial<World, Hover>,
    ) -> Self {
        definitions.insert(id, Definition::Value(crate::name::record(name, [])));
        Self::new(definitions, projection)
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
    pub fn from_contributions<World, Hover>(
        entries: impl IntoIterator<Item = (gid::CellId, Library<World, Hover>)>,
    ) -> (Self, Vec<Partial<World, Hover>>, Vec<CompletionProvider>) {
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
                projections.push(library.projection);
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

    pub fn get(&self, id: gid::CellId) -> Option<&Definitions> {
        self.entries
            .iter()
            .find(|(candidate, _)| *candidate == id)
            .map(|(_, definitions)| definitions)
    }

    pub fn iter(&self) -> impl Iterator<Item = (gid::CellId, &Definitions)> {
        self.entries
            .iter()
            .map(|(id, definitions)| (*id, definitions))
    }

    pub fn values(&self, cell: gid::CellId) -> impl Iterator<Item = LocatedValue<'_>> {
        self.entries
            .iter()
            .filter_map(move |(library, definitions)| {
                definitions.value(cell).map(|value| LocatedValue {
                    library: *library,
                    value,
                })
            })
    }

    pub fn foreign_sources(&self, cell: gid::CellId) -> impl Iterator<Item = gid::CellId> + '_ {
        self.entries
            .iter()
            .filter_map(move |(library, definitions)| {
                matches!(definitions.get(cell), Some(Definition::Foreign(_))).then_some(*library)
            })
    }

    pub fn contributors(&self, cell: gid::CellId) -> impl Iterator<Item = gid::CellId> + '_ {
        self.entries
            .iter()
            .filter_map(move |(library, definitions)| definitions.get(cell).map(|_| *library))
    }

    pub fn first_value(&self, cell: gid::CellId) -> Option<&gid::Value> {
        self.values(cell).next().map(|definition| definition.value)
    }

    pub fn resolve(
        &self,
        cell: gid::CellId,
    ) -> Option<(gid::Resolution, grap_runtime::Definition)> {
        self.entries.iter().find_map(move |(source, definitions)| {
            definitions
                .get(cell)
                .cloned()
                .map(|target| (gid::Resolution::Library(*source), target))
        })
    }

    pub fn cell_ids(&self) -> impl Iterator<Item = gid::CellId> + '_ {
        self.entries
            .iter()
            .flat_map(|(_, definitions)| definitions.iter().map(|(cell, _)| cell))
    }
}

impl grap_runtime::Host for Libraries {
    fn resolve(&self, cell: gid::CellId) -> Option<(gid::Resolution, grap_runtime::Definition)> {
        self.resolve(cell)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use progred_display::recording::{Recordable, Recorded};

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
fn apply_scoped(&self, _: &gid::Value, _: &[(gid::CellId, gid::Value)], _scope: Option<&grap_runtime::ForeignOverlay<'_>>) -> grap_runtime::Evaluation {
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
        let (libraries, projections, _) = Libraries::from_contributions([
            (
                LEFT_LIBRARY,
                Library::named(
                    LEFT_LIBRARY,
                    "left",
                    Definitions::from_parts(
                        left_cells,
                        ForeignFunctions::default()
                            .register(SHARED_FUNCTION, ForeignFunction::new(left_function)),
                    ),
                    progred_display::partial(left_projection),
                ),
            ),
            (
                RIGHT_LIBRARY,
                Library::named(
                    RIGHT_LIBRARY,
                    "right",
                    Definitions::from_parts(
                        right_cells,
                        ForeignFunctions::default()
                            .register(SHARED_FUNCTION, ForeignFunction::new(right_function)),
                    ),
                    progred_display::partial(right_projection),
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
                &libraries,
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
                        default_projection: progred_display::partial(|_| None),
                        env: &NoEval,
                        value: Some(&value),
                        scale_factor: 1.0,
                        writable: true,
                        selection: None,
                        pending: None,
                        state: None,
                        targets: progred_display::ProjectionTargets::new(&target),
                    })
                    .and_then(|layout| match layout.record() {
                        Recorded::Leaf(Leaf::Text { text, .. }) => Some(text),
                        _ => None,
                    })
                    .unwrap()
                })
                .collect::<Vec<_>>(),
            ["left", "right"]
        );
    }

    #[test]
    fn replacing_a_foreign_registration_does_not_add_cell_values() {
        let name = crate::name::record("shared", []);
        let mut cells = Cells::new();
        cells.set_value(SHARED_FUNCTION, name.clone());
        let definitions = Definitions::from_parts(
            cells,
            ForeignFunctions::default()
                .register(
                    SHARED_FUNCTION,
                    ForeignFunction::new(|_, _, _| Ok(crate::absent::decline())),
                )
                .register(SHARED_FUNCTION, ForeignFunction::new(left_function)),
        );
        let (libraries, _, _) = Libraries::from_contributions([(
            LEFT_LIBRARY,
            Library::<(), ()>::named(
                LEFT_LIBRARY,
                "test",
                definitions,
                progred_display::partial(|_| None),
            ),
        )]);
        assert_eq!(
            libraries
                .values(SHARED_FUNCTION)
                .map(|entry| entry.value)
                .collect::<Vec<_>>(),
            [&name]
        );
        assert_eq!(
            libraries
                .foreign_sources(SHARED_FUNCTION)
                .collect::<Vec<_>>(),
            [LEFT_LIBRARY]
        );
        assert_eq!(
            libraries.contributors(SHARED_FUNCTION).collect::<Vec<_>>(),
            [LEFT_LIBRARY]
        );
        assert_eq!(
            grap_runtime::evaluate(&SHARED_FUNCTION.into(), &libraries, 20).result,
            name
        );
        assert_eq!(
            grap_runtime::evaluate(
                &grap_runtime::call(SHARED_FUNCTION.into(), []),
                &libraries,
                20
            )
            .result,
            Value::from(b"left".to_vec())
        );
        assert_eq!(
            grap_runtime::apply(&SHARED_FUNCTION.into(), [], &libraries, 20).result,
            Value::from(b"left".to_vec())
        );
    }

    #[test]
    fn definitions_are_sorted_unique_and_copy_on_write() {
        let mut definitions = Definitions::default();
        definitions.insert(
            SHARED_FUNCTION,
            Definition::foreign(
                name::record("native", []),
                ForeignFunction::new(left_function),
            ),
        );
        definitions.insert(
            SHARED_CELL,
            Definition::Value(Value::from(b"original".to_vec())),
        );
        let original = definitions.clone();
        definitions.insert(
            SHARED_CELL,
            Definition::Value(Value::from(b"replacement".to_vec())),
        );

        assert_eq!(
            definitions.iter().map(|(id, _)| id).collect::<Vec<_>>(),
            [SHARED_CELL, SHARED_FUNCTION]
        );
        assert_eq!(
            original.value(SHARED_CELL),
            Some(&Value::from(b"original".to_vec()))
        );
        assert_eq!(
            definitions.value(SHARED_CELL),
            Some(&Value::from(b"replacement".to_vec()))
        );
        assert_eq!(
            definitions.value(SHARED_FUNCTION).and_then(name::read),
            Some("native")
        );
        assert!(matches!(
            definitions.get(SHARED_FUNCTION),
            Some(Definition::Foreign(_))
        ));
    }

    #[test]
    fn replacing_a_library_replaces_every_contribution_in_place() {
        let old: Library<(), ()> = Library::named(
            LEFT_LIBRARY,
            "old",
            Definitions::default(),
            progred_display::partial(|_| panic!("replaced projection")),
        )
        .with_completions(|_| panic!("replaced completion provider"));
        let replacement_projection = progred_display::partial(left_projection);
        let right_projection = progred_display::partial(right_projection);
        let replacement = Library::named(
            LEFT_LIBRARY,
            "replacement",
            Definitions::default(),
            replacement_projection.clone(),
        )
        .with_completions(|request| {
            Some(vec![Completion::new(request.query, SHARED_FUNCTION.into())])
        });
        let right = Library::named(
            RIGHT_LIBRARY,
            "right",
            Definitions::default(),
            right_projection.clone(),
        );
        let (libraries, projections, providers) = Libraries::from_contributions([
            (LEFT_LIBRARY, old),
            (RIGHT_LIBRARY, right),
            (LEFT_LIBRARY, replacement),
        ]);

        assert_eq!(
            libraries.iter().map(|(id, _)| id).collect::<Vec<_>>(),
            [LEFT_LIBRARY, RIGHT_LIBRARY]
        );
        assert_eq!(
            libraries.first_value(LEFT_LIBRARY).and_then(name::read),
            Some("replacement")
        );
        assert_eq!(projections.len(), 2);
        assert!(Rc::ptr_eq(&projections[0], &replacement_projection));
        assert!(Rc::ptr_eq(&projections[1], &right_projection));
        assert_eq!(providers.len(), 1);
        for kind in [
            progred_display::CompletionKind::Value,
            progred_display::CompletionKind::Field,
        ] {
            assert_eq!(
                providers[0](&CompletionRequest {
                    query: "query",
                    kind,
                    scope: progred_display::CompletionScope::Suggested,
                    path: &[],
                    value_at: &|_| None,
                    resolve: &|_| None,
                })
                .unwrap()[0]
                    .display,
                "query".into()
            );
        }
    }

    #[test]
    fn library_descriptions_are_ordinary_cells() {
        let (libraries, _, _) = Libraries::from_contributions([(
            LEFT_LIBRARY,
            Library::<(), ()>::named(
                LEFT_LIBRARY,
                "library",
                Definitions::default(),
                progred_display::partial(|_| None),
            ),
        )]);
        let description = name::record("library", []);
        assert_eq!(libraries.first_value(LEFT_LIBRARY), Some(&description));
        assert_eq!(
            grap_runtime::evaluate(&LEFT_LIBRARY.into(), &libraries, 20).result,
            description
        );
        assert_eq!(libraries.cell_ids().collect::<Vec<_>>(), [LEFT_LIBRARY]);
    }

    #[test]
    fn unnamed_native_definitions_have_an_empty_description() {
        let (libraries, _, _) = Libraries::from_contributions([(
            LEFT_LIBRARY,
            Library::<(), ()>::new(
                Definitions::from_parts(
                    Cells::new(),
                    ForeignFunctions::default()
                        .register(SHARED_FUNCTION, ForeignFunction::new(left_function)),
                ),
                progred_display::partial(|_| None),
            ),
        )]);
        assert_eq!(
            grap_runtime::evaluate(&SHARED_FUNCTION.into(), &libraries, 20).result,
            Value::record([])
        );
        assert_eq!(
            grap_runtime::apply(&SHARED_FUNCTION.into(), [], &libraries, 20).result,
            Value::from(b"left".to_vec())
        );
    }

    #[test]
    fn libraries_are_an_ordered_unique_map_without_hashing() {
        let mut left_cells = Cells::new();
        left_cells.set_value(SHARED_CELL, Value::from(b"left".to_vec()));
        let left = Library::<(), ()>::named(
            LEFT_LIBRARY,
            "left",
            Definitions::from_parts(
                left_cells,
                ForeignFunctions::default()
                    .register(SHARED_CELL, ForeignFunction::new(left_function)),
            ),
            progred_display::partial(|_| None),
        );
        let mut right_cells = Cells::new();
        right_cells.set_value(SHARED_CELL, Value::from(b"right".to_vec()));
        let right = Library::<(), ()>::named(
            RIGHT_LIBRARY,
            "right",
            Definitions::from_parts(right_cells, ForeignFunctions::default()),
            progred_display::partial(|_| None),
        );
        let (mut libraries, _, _) =
            Libraries::from_contributions([(LEFT_LIBRARY, left), (RIGHT_LIBRARY, right)]);
        let replacement = Definitions::default();

        assert!(libraries.get(LEFT_LIBRARY).is_some());
        assert_eq!(
            libraries
                .first_value(LEFT_LIBRARY)
                .and_then(crate::name::read),
            Some("left")
        );
        assert_eq!(
            libraries.foreign_sources(SHARED_CELL).collect::<Vec<_>>(),
            [LEFT_LIBRARY]
        );
        assert_eq!(
            libraries
                .values(SHARED_CELL)
                .map(|definition| definition.library)
                .collect::<Vec<_>>(),
            [LEFT_LIBRARY, RIGHT_LIBRARY]
        );
        assert!(libraries.insert(LEFT_LIBRARY, replacement).is_some());
        assert_eq!(
            libraries.iter().map(|(id, _)| id).collect::<Vec<_>>(),
            [LEFT_LIBRARY, RIGHT_LIBRARY]
        );
    }
}
#[cfg(test)]
mod test_widgets;
