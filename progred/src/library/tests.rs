use super::*;
use crate::display::recording::{Recordable, Recorded};
use crate::libraries::name;
use crate::projection::{Layout, ProjectionInput};

use crate::display::test_support::NoEval;
use crate::display::text as text_layout;
use crate::libraries::test_widgets::projection_input;
use ::grap::{Environment, Expression, ForeignFunction, Halt};
use gid::{CellId, Value};
use puri::draw::Leaf;
use std::rc::Rc;

const SHARED_CELL: CellId = CellId::from_u128(1);
const SHARED_FUNCTION: CellId = CellId::from_u128(2);
const LEFT_LIBRARY: CellId = CellId::from_u128(3);
const RIGHT_LIBRARY: CellId = CellId::from_u128(4);

fn left_function(_: &mut ::grap::Context, _: &Expression, _: &Environment) -> Result<Value, Halt> {
    Ok(Value::from(b"left".to_vec()))
}

fn right_function(_: &mut ::grap::Context, _: &Expression, _: &Environment) -> Result<Value, Halt> {
    Ok(Value::from(b"right".to_vec()))
}

fn left_projection(_: &ProjectionInput<'_>) -> Option<Layout> {
    Some(text_layout("left"))
}

fn right_projection(_: &ProjectionInput<'_>) -> Option<Layout> {
    Some(text_layout("right"))
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
            crate::libraries::name::named_library::<crate::Editor>(
                LEFT_LIBRARY,
                "left",
                Definitions::from_parts(
                    left_cells,
                    ForeignFunctions::default()
                        .register(SHARED_FUNCTION, ForeignFunction::from_value(left_function)),
                ),
            )
            .with_projection(crate::display::partial(left_projection)),
        ),
        (
            RIGHT_LIBRARY,
            crate::libraries::name::named_library::<crate::Editor>(
                RIGHT_LIBRARY,
                "right",
                Definitions::from_parts(
                    right_cells,
                    ForeignFunctions::default()
                        .register(SHARED_FUNCTION, ForeignFunction::from_value(right_function)),
                ),
            )
            .with_projection(crate::display::partial(right_projection)),
        ),
    ]);

    assert_eq!(
        libraries
            .definitions(SHARED_CELL)
            .map(|(library, definition)| (library, definition.value().clone()))
            .collect::<Vec<_>>(),
        [
            (LEFT_LIBRARY, Value::from(b"left".to_vec())),
            (RIGHT_LIBRARY, Value::from(b"right".to_vec()))
        ]
    );
    assert_eq!(
        ::grap::evaluate_value(
            &::grap::call(Value::from(SHARED_FUNCTION), []),
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
            .map(|(_, projection)| {
                projection(&projection_input(&NoEval, &(&value).into()))
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
    let name = crate::libraries::name::record("shared", []);
    let mut cells = Cells::new();
    cells.set_value(SHARED_FUNCTION, name.clone());
    let definitions = Definitions::from_parts(
        cells,
        ForeignFunctions::default()
            .register(
                SHARED_FUNCTION,
                ForeignFunction::from_value(|_, _, _| Ok(crate::libraries::absent::decline())),
            )
            .register(SHARED_FUNCTION, ForeignFunction::from_value(left_function)),
    );
    let (libraries, _, _) = Libraries::from_contributions([(
        LEFT_LIBRARY,
        crate::libraries::name::named_library::<crate::Editor>(LEFT_LIBRARY, "test", definitions),
    )]);
    assert_eq!(
        libraries
            .definitions(SHARED_FUNCTION)
            .map(|(_, definition)| definition.value())
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
        libraries
            .definitions(SHARED_FUNCTION)
            .map(|(library, _)| library)
            .collect::<Vec<_>>(),
        [LEFT_LIBRARY]
    );
    assert_eq!(
        ::grap::evaluate_value(&SHARED_FUNCTION.into(), &libraries, 20).result,
        name
    );
    assert_eq!(
        ::grap::evaluate_value(&::grap::call(SHARED_FUNCTION.into(), []), &libraries, 20).result,
        Value::from(b"left".to_vec())
    );
    assert_eq!(
        ::grap::apply_value(&SHARED_FUNCTION.into(), [], &libraries, 20).result,
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
            ForeignFunction::from_value(left_function),
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
    let old: Library<crate::Editor, crate::frame::Hovered> =
        crate::libraries::name::named_library::<crate::Editor>(
            LEFT_LIBRARY,
            "old",
            Definitions::default(),
        )
        .with_projection(crate::display::partial(|_| panic!("replaced projection")))
        .with_completions(|_| panic!("replaced completion provider"));
    let replacement_projection = crate::display::partial(left_projection);
    let right_projection = crate::display::partial(right_projection);
    let replacement = crate::libraries::name::named_library::<crate::Editor>(
        LEFT_LIBRARY,
        "replacement",
        Definitions::default(),
    )
    .with_projection(replacement_projection.clone())
    .with_completions(|request| {
        Some(vec![crate::libraries::completion::select(
            request.query,
            SHARED_FUNCTION.into(),
        )])
    });
    let right = crate::libraries::name::named_library::<crate::Editor>(
        RIGHT_LIBRARY,
        "right",
        Definitions::default(),
    )
    .with_projection(right_projection.clone());
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
    assert!(Rc::ptr_eq(&projections[0].1, &replacement_projection));
    assert!(Rc::ptr_eq(&projections[1].1, &right_projection));
    assert_eq!(providers.len(), 1);
    for kind in [
        crate::display::CompletionKind::Value,
        crate::display::CompletionKind::Field,
    ] {
        assert_eq!(
            providers[0](&CompletionRequest {
                raw: false,
                query: "query",
                kind,
                scope: crate::display::CompletionScope::Everything,
                path: &[],
                value_at: &|_| None,
                resolve: &|_| None,
                cells: &Vec::new,
                name: &|_| None,
                libraries: None,
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
        crate::libraries::name::named_library::<crate::Editor>(
            LEFT_LIBRARY,
            "library",
            Definitions::default(),
        ),
    )]);
    let description = name::record("library", []);
    assert_eq!(libraries.first_value(LEFT_LIBRARY), Some(&description));
    assert_eq!(
        ::grap::evaluate_value(&LEFT_LIBRARY.into(), &libraries, 20).result,
        description
    );
    assert_eq!(libraries.cell_ids().collect::<Vec<_>>(), [LEFT_LIBRARY]);
}

#[test]
fn unnamed_native_definitions_have_an_empty_description() {
    let (libraries, _, _) = Libraries::from_contributions([(
        LEFT_LIBRARY,
        Library::<crate::Editor, crate::frame::Hovered>::new(Definitions::from_parts(
            Cells::new(),
            ForeignFunctions::default()
                .register(SHARED_FUNCTION, ForeignFunction::from_value(left_function)),
        )),
    )]);
    assert_eq!(
        ::grap::evaluate_value(&SHARED_FUNCTION.into(), &libraries, 20).result,
        Value::record([])
    );
    assert_eq!(
        ::grap::apply_value(&SHARED_FUNCTION.into(), [], &libraries, 20).result,
        Value::from(b"left".to_vec())
    );
}

#[test]
fn libraries_are_an_ordered_unique_map_without_hashing() {
    let mut left_cells = Cells::new();
    left_cells.set_value(SHARED_CELL, Value::from(b"left".to_vec()));
    let left = crate::libraries::name::named_library::<crate::Editor>(
        LEFT_LIBRARY,
        "left",
        Definitions::from_parts(
            left_cells,
            ForeignFunctions::default()
                .register(SHARED_CELL, ForeignFunction::from_value(left_function)),
        ),
    );
    let mut right_cells = Cells::new();
    right_cells.set_value(SHARED_CELL, Value::from(b"right".to_vec()));
    let right = crate::libraries::name::named_library::<crate::Editor>(
        RIGHT_LIBRARY,
        "right",
        Definitions::from_parts(right_cells, ForeignFunctions::default()),
    );
    let (mut libraries, _, _) =
        Libraries::from_contributions([(LEFT_LIBRARY, left), (RIGHT_LIBRARY, right)]);
    let replacement = Definitions::default();

    assert!(libraries.get(LEFT_LIBRARY).is_some());
    assert_eq!(
        libraries
            .first_value(LEFT_LIBRARY)
            .and_then(crate::libraries::name::read),
        Some("left")
    );
    assert_eq!(
        libraries.foreign_sources(SHARED_CELL).collect::<Vec<_>>(),
        [LEFT_LIBRARY]
    );
    assert_eq!(
        libraries
            .definitions(SHARED_CELL)
            .map(|(library, _)| library)
            .collect::<Vec<_>>(),
        [LEFT_LIBRARY, RIGHT_LIBRARY]
    );
    assert!(libraries.insert(LEFT_LIBRARY, replacement).is_some());
    assert_eq!(
        libraries.iter().map(|(id, _)| id).collect::<Vec<_>>(),
        [LEFT_LIBRARY, RIGHT_LIBRARY]
    );
}
