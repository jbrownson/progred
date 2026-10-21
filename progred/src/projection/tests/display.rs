use super::*;
use crate::projection::Layout;
use crate::projection::test_support::ProjectionInput;
use gid::Step;

#[test]
fn contextual_projection_is_local_whether_it_accepts_or_declines() {
    use crate::display::{at, descend, partial, row};
    for use_at in [false, true] {
        for accepts in [false, true] {
            let field = new_cell_id();
            let child = new_cell_id();
            let sibling = new_cell_id();
            let nested = Value::record([(child, Value::from(vec![1]))]);
            let root = Value::record([(field, nested.clone()), (sibling, Value::from(vec![2]))]);
            let seen = Rc::new(std::cell::RefCell::new(Vec::new()));
            let calls = seen.clone();
            let local = partial(
                move |input: &crate::display::ProjectionInput<'_, crate::Editor, Hovered>| {
                    calls.borrow_mut().push(input.value?.clone());
                    accepts
                        .then(|| descend([Step::Key(child)], crate::display::Projected::ordinary()))
                },
            );
            let expected_root = root.clone();
            let ambient_seen = Rc::new(std::cell::RefCell::new(Vec::new()));
            let ambient_calls = ambient_seen.clone();
            let projection = Projection::new([partial(move |input| {
                ambient_calls.borrow_mut().push(input.value?.clone());
                (input.value == Some(&expected_root)).then(|| {
                    row(
                        0.0,
                        [
                            if use_at {
                                at(
                                    [Step::Key(field)],
                                    &nested,
                                    crate::display::Projected::here(
                                        local.clone(),
                                        &input.default_projection,
                                    ),
                                )
                            } else {
                                descend(
                                    [Step::Key(field)],
                                    crate::display::Projected::here(
                                        local.clone(),
                                        &input.default_projection,
                                    ),
                                )
                            },
                            descend([Step::Key(sibling)], crate::display::Projected::ordinary()),
                        ],
                    )
                })
            })]);
            let mut world = crate::editor_of(root);
            world.stack.projection = projection;
            editing_frame(&mut world);
            assert_eq!(
                *seen.borrow(),
                [Value::record([(child, Value::from(vec![1]))])]
            );
            assert!(ambient_seen.borrow().contains(&Value::from(vec![1])));
            assert!(ambient_seen.borrow().contains(&Value::from(vec![2])));
            assert_eq!(ambient_seen.borrow().contains(&seen.borrow()[0]), !accepts);
        }
    }
}

#[test]
fn local_projection_receives_missing_values_without_leaking_through_follow_or_at() {
    use crate::display::{at, descend, partial};
    for mode in 0..3 {
        let cell = new_cell_id();
        let field = new_cell_id();
        let leaf = Value::from(vec![7]);
        let root = Value::record([(field, Value::Cell(cell))]);
        let seen = Rc::new(std::cell::RefCell::new(Vec::new()));
        let calls = seen.clone();
        let local = partial(
            move |input: &crate::display::ProjectionInput<'_, crate::Editor, Hovered>| {
                calls.borrow_mut().push(input.value.cloned());
                match mode {
                    0 => Some(descend(
                        [Step::Follow(gid::Resolution::Document)],
                        crate::display::Projected::ordinary(),
                    )),
                    1 => Some(at(
                        [Step::Key(field)],
                        Value::from(vec![7]),
                        crate::display::Projected::ordinary(),
                    )),
                    _ => None,
                }
            },
        );
        let expected_root = root.clone();
        let projection = Projection::new([partial(move |input| {
            (input.value == Some(&expected_root)).then(|| {
                descend(
                    [Step::Key(if mode == 2 { cell } else { field })],
                    crate::display::Projected::here(local.clone(), &input.default_projection),
                )
            })
        })]);
        let mut cells = Cells::new();
        cells.set_value(cell, leaf.clone());
        let mut world = crate::test_editor(Document {
            root: Some(root),
            cells,
        });
        world.stack.projection = projection;
        editing_frame(&mut world);
        assert_eq!(
            *seen.borrow(),
            [if mode == 2 { None } else { Some(cell.into()) }]
        );
        assert!(!seen.borrow().contains(&Some(leaf)));
    }
}

#[test]
fn pane_entry_still_follows_cells_but_stops_at_the_first_non_cell() {
    let cell = new_cell_id();
    let field = new_cell_id();
    let definition = Value::record([(field, Value::from(vec![3]))]);
    let mut cells = Cells::new();
    cells.set_value(cell, definition.clone());
    let mut world = crate::test_editor(Document {
        root: Some(cell.into()),
        cells,
    });
    let seen = Rc::new(std::cell::RefCell::new(Vec::new()));
    let calls = seen.clone();
    let projection = Projection::default().with_entry(crate::display::partial(move |input| {
        calls.borrow_mut().push(input.value?.clone());
        None
    }));
    world.stack.projection = projection;
    editing_frame(&mut world);
    assert_eq!(*seen.borrow(), [Value::Cell(cell), definition]);
}

#[test]
fn descents_replace_current_and_default_projections_independently() {
    use crate::display::{at, descend, dim, partial, row};
    for use_at in [false, true] {
        let field = new_cell_id();
        let child = new_cell_id();
        let sibling = new_cell_id();
        let nested = Value::record([(child, Value::from(vec![1]))]);
        let root = Value::record([(field, nested.clone()), (sibling, Value::from(vec![2]))]);
        let seen = Rc::new(std::cell::RefCell::new(Vec::new()));
        let current_seen = seen.clone();
        let current = partial(
            move |input: &crate::display::ProjectionInput<'_, crate::Editor, Hovered>| {
                current_seen
                    .borrow_mut()
                    .push(("current", input.value?.clone()));
                None
            },
        );
        let child_seen = seen.clone();
        let children = partial(
            move |input: &crate::display::ProjectionInput<'_, crate::Editor, Hovered>| {
                child_seen
                    .borrow_mut()
                    .push(("children", input.value?.clone()));
                Some(dim("child"))
            },
        );
        let default_seen = seen.clone();
        let expected_root = root.clone();
        let projection = Projection::new([partial(move |input| {
            default_seen
                .borrow_mut()
                .push(("original", input.value?.clone()));
            (input.value == Some(&expected_root)).then(|| {
                row(
                    0.0,
                    [
                        if use_at {
                            at(
                                [Step::Key(field)],
                                &nested,
                                crate::display::Projected::by(
                                    Some(current.clone()),
                                    Some(children.clone()),
                                ),
                            )
                        } else {
                            descend(
                                [Step::Key(field)],
                                crate::display::Projected::by(
                                    Some(current.clone()),
                                    Some(children.clone()),
                                ),
                            )
                        },
                        descend([Step::Key(sibling)], crate::display::Projected::ordinary()),
                    ],
                )
            })
        })]);
        let mut world = crate::editor_of(root.clone());
        world.stack.projection = projection;
        editing_frame(&mut world);
        assert_eq!(
            *seen.borrow(),
            [
                ("original", root),
                ("current", Value::record([(child, Value::from(vec![1]))])),
                ("children", Value::from(vec![1])),
                ("original", Value::from(vec![2])),
            ]
        );
    }
}

#[test]
fn explicit_scope_reaches_nested_containers_and_cells_but_not_siblings() {
    use crate::display::{at, compose_partials, descend, dim, partial, row};
    let scoped = new_cell_id();
    let ordinary = new_cell_id();
    let items = new_cell_id();
    let cell = new_cell_id();
    let nested = Value::record([(items, Value::list([Value::from(vec![1]), cell.into()]))]);
    let root = Value::record([(scoped, nested.clone()), (ordinary, nested.clone())]);
    let seen = Rc::new(std::cell::RefCell::new(Vec::new()));
    let calls = seen.clone();
    let special = partial(
        move |input: &crate::display::ProjectionInput<'_, crate::Editor, Hovered>| {
            input.value?.as_blob()?;
            calls.borrow_mut().push(input.value?.clone());
            Some(dim("scoped"))
        },
    );
    let expected_root = root.clone();
    let projection = Projection::new([partial(move |input| {
        (input.value == Some(&expected_root)).then(|| {
            let scoped_projection =
                compose_partials([special.clone(), input.default_projection.clone()]);
            row(
                0.0,
                [
                    at(
                        [Step::Key(scoped)],
                        &nested,
                        crate::display::Projected::by(
                            Some(scoped_projection.clone()),
                            Some(scoped_projection),
                        ),
                    ),
                    descend([Step::Key(ordinary)], crate::display::Projected::ordinary()),
                ],
            )
        })
    })]);
    let mut cells = Cells::new();
    cells.set_value(cell, Value::from(vec![2]));
    let mut world = crate::test_editor(Document {
        root: Some(root),
        cells,
    });
    world.stack.projection = projection;
    editing_frame(&mut world);
    assert_eq!(*seen.borrow(), [Value::from(vec![1]), Value::from(vec![2])]);
}

#[test]
fn record_combinator_chooses_a_projection_for_each_field() {
    use crate::display::{dim, partial};
    let first = new_cell_id();
    let second = new_cell_id();
    let ordinary = new_cell_id();
    let value = Value::from(vec![1]);
    let root = Value::record([
        (first, value.clone()),
        (second, value.clone()),
        (ordinary, value),
    ]);
    let seen = Rc::new(std::cell::RefCell::new(Vec::new()));
    let observe = |key| {
        let seen = seen.clone();
        partial(
            move |_: &crate::display::ProjectionInput<'_, crate::Editor, Hovered>| {
                seen.borrow_mut().push(key);
                Some(dim("field"))
            },
        )
    };
    let first_projection = observe(first);
    let second_projection = observe(second);
    let default_projection = observe(ordinary);
    let projection = Projection::new([
        crate::display::test_support::record_partial(move |key| match key {
            key if key == first => Some(first_projection.clone()),
            key if key == second => Some(second_projection.clone()),
            _ => None,
        }),
        default_projection,
    ]);
    let mut world = crate::editor_of(root);
    world.stack.projection = projection;
    editing_frame(&mut world);
    let mut actual = seen.borrow().clone();
    actual.sort();
    let mut expected = [first, second, ordinary];
    expected.sort();
    assert_eq!(actual, expected);
}

#[test]
fn partials_receive_selection_and_annotations_positionally() {
    // The first library code ever to SEE editor state — as data,
    // positionally: the payload only at the selected path, the
    // annotation record only at its own.
    fn probe(input: &ProjectionInput<'_>) -> Option<Layout> {
        input.value?.as_blob()?;
        Some(crate::display::dim(
            match (input.selection.is_some(), input.state.is_some()) {
                (true, _) => "selected here",
                (false, true) => "annotated here",
                (false, false) => "cold",
            },
        ))
    }
    let projection: Projection = Projection::new([crate::display::partial(probe)]);
    // How many glyphs the frame draws, which tells the three apart.
    let glyphs = |selection: Option<Selection>, annotations: Annotations| {
        let mut world = crate::editor_of(Value::from(vec![7u8]));
        world.model.selection = selection;
        world.model.workspace.document.annotations = annotations;
        world.stack.projection = projection.clone();
        let frame = editing_frame(&mut world);
        crate::display::test_support::painted(&settle(frame, &world.sources()).list.0)
            .map(|command| match command {
                puri::draw::DrawCmd::GlyphRun(run) => run.glyphs.len(),
                _ => 0,
            })
            .sum::<usize>()
    };
    let cold = glyphs(None, Annotations::default());
    let selected = glyphs(
        Some(crate::selection::Selection::edge(Vec::new())),
        Annotations::default(),
    );
    let mut marked = Annotations::default();
    marked.set_field(&[], crate::annotations::FOLD, Some(Value::from(vec![1u8])));
    let annotated = glyphs(None, marked);
    assert_ne!(cold, selected);
    assert_ne!(cold, annotated);
    assert_ne!(selected, annotated);
}

#[test]
fn a_projection_defined_as_data_realizes() {
    // The display language's data form, decoded with the PROVIDED
    // intents and realized through the ordinary pipeline — the same
    // boundary a Grap-backed library projection can use.
    fn probe(input: &ProjectionInput<'_>) -> Option<Layout> {
        use crate::libraries::layout::{test_support as data, vocabulary};
        input.value?.as_blob()?;
        data::decode(
            &data::selectable(data::row(
                4.0,
                [
                    data::text_leaf("from", vocabulary::NAME_FACE),
                    data::text_leaf("data", vocabulary::DIM_FACE),
                ],
            )),
            &input.site(),
        )
    }
    let mut editor = crate::editor_of(Value::from(vec![7u8]));
    editor.stack.projection = Projection::new([crate::display::partial(probe)]);
    let mut driver = crate::driver::Driver::new(editor, crate::driver::Driver::VIEWPORT);
    assert!(driver.rect::<Step>(&[]).width() > 0.0);
    driver.click::<Step>(&[]);
    assert!(driver.selection().is_empty());
}
