use super::*;
use crate::display as d;
use crate::display::widget::style;
use crate::driver::Driver;
use crate::projection::Partial;
use ui_events::keyboard::Modifiers;

fn world(root: Value) -> crate::Editor {
    crate::editor_of(root)
}

fn projection(world: &crate::Editor, rule: Partial) -> Projection {
    Projection::new([rule, world.stack.projection.partial.clone()])
}

fn stop<'a>(
    frame: &'a crate::projection::test_support::HoverOutput<crate::Editor>,
    path: &[Step],
) -> &'a Landmark<crate::Editor> {
    frame
        .landmarks
        .landmark(path)
        .expect("projected occurrence")
}

/// Press `key` with the command key held; whether the editor took it.
fn command(driver: &mut Driver, key: Key) -> bool {
    driver.keystroke(key, crate::modifiers::command())
}

fn copy(driver: &mut Driver) -> bool {
    command(driver, Key::Character("c".into()))
}

fn grounds(commands: &[DrawCmd], color: Color) -> Vec<Rect> {
    crate::display::test_support::painted(commands)
        .filter_map(|command| match command {
            DrawCmd::Fill {
                shape: Shape::RoundedRect(rect),
                brush: Brush::Solid(brush),
                ..
            } if *brush == color => Some(rect.rect()),
            _ => None,
        })
        .collect()
}

#[test]
fn evaluated_output_is_tinted_without_tinting_its_expression() {
    use crate::libraries::presentation::vocabulary::RESULT;
    let child = new_cell_id();
    let value = Value::record([(child, Value::list([text::value("result")]))]);
    let mut world = world(Value::record([(grap::vocabulary::EVALUATE, value)]));
    let frame = editing_frame(&mut world, false);
    let result = stop(&frame, &[Step::Key(RESULT)]).rect;
    let expression = stop(&frame, &[Step::Key(grap::vocabulary::EVALUATE)]).rect;
    let drawing = settle(frame).list;
    let tints = grounds(&drawing.0, style::Theme::Light.palette().readonly_ground);
    assert_eq!(tints, [result.inset(3.0)]);
    assert!(!tints[0].contains(expression.center()));
}

#[test]
fn nested_computed_results_do_not_stack_readonly_tints() {
    use crate::libraries::presentation::vocabulary::RESULT;
    let mut world = world(Value::record([(
        grap::vocabulary::EVALUATE,
        Value::record([(grap::vocabulary::EVALUATE, text::value("nested"))]),
    )]));
    let frame = editing_frame(&mut world, false);
    let outer = stop(&frame, &[Step::Key(RESULT)]).rect;
    let source_result = stop(
        &frame,
        &[Step::Key(grap::vocabulary::EVALUATE), Step::Key(RESULT)],
    )
    .rect;
    stop(&frame, &[Step::Key(RESULT), Step::Key(RESULT)]);
    let drawing = settle(frame).list;
    let tints = grounds(&drawing.0, style::Theme::Light.palette().readonly_ground);
    assert_eq!(tints, [source_result.inset(3.0), outer.inset(3.0)]);
}

#[test]
fn evaluated_results_have_independent_read_only_selection_copy_and_folds() {
    use crate::libraries::presentation::vocabulary::RESULT;
    let child = new_cell_id();
    let value = Value::record([(child, Value::list([f64::value(1.0)]))]);
    let mut world = world(Value::record([(grap::vocabulary::EVALUATE, value.clone())]));
    let result = [Step::Key(RESULT)];
    let child_path = [Step::Key(RESULT), Step::Key(child)];
    let before = world.model.doc.clone();
    world.model.mark_saved();
    let mut driver = Driver::new(world, Driver::VIEWPORT);
    driver.select(&child_path);
    assert!(
        driver
            .editor()
            .model
            .selection
            .as_ref()
            .unwrap()
            .scope()
            .location(&child_path)
            .is_none()
    );
    assert!(copy(&mut driver));
    assert_eq!(
        driver.copied(),
        value.as_record().unwrap().get(&child).cloned()
    );

    driver.select(&result);
    assert!(command(&mut driver, Key::Named(NamedKey::ArrowUp)));
    assert!(!driver.drawn(&child_path));
    // The stored expression remains projected, not folded with its result.
    assert!(driver.drawn(&[Step::Key(grap::vocabulary::EVALUATE)]));
    assert!(Rc::ptr_eq(&before, &driver.editor().model.doc));
    assert!(!driver.editor().model.dirty());
}

#[test]
fn nested_result_evaluations_use_their_own_allowance() {
    use crate::libraries::{control, layout, presentation};
    use presentation::vocabulary::{RENDER, RESULT};
    let value = Value::list([f64::value(42.0)]);
    let expression = grap::call(
        control::vocabulary::DO.into(),
        [(
            control::vocabulary::EXPRESSIONS,
            Value::list(std::iter::repeat_n(Value::record([]), 10).chain([value.clone()])),
        )],
    );
    for explicit in [false, true] {
        let nested = if explicit {
            Value::record([(
                RENDER,
                Value::record([
                    (grap::vocabulary::EXPRESSION, expression.clone()),
                    (layout::vocabulary::FUEL, f64::value(100.0)),
                ]),
            )])
        } else {
            Value::record([(grap::vocabulary::EVALUATE, expression.clone())])
        };
        // Returning this inert record is cheap; evaluating its expression is not.
        let mut driver = Driver::of(Value::record([(
            RENDER,
            Value::record([
                (grap::vocabulary::EXPRESSION, nested),
                (layout::vocabulary::FUEL, f64::value(3.0)),
            ]),
        )]));
        driver.select(&[Step::Key(RESULT), Step::Key(RESULT)]);
        assert!(copy(&mut driver));
        assert_eq!(driver.copied(), Some(value.clone()));
    }
}

#[test]
fn at_copies_the_displayed_value_not_a_coincident_document_path() {
    use puri::edit::TextClipboard;
    let (marker, occurrence, child) = (new_cell_id(), new_cell_id(), new_cell_id());
    let displayed = Value::record([(child, text::value("computed"))]);
    let mut world = world(Value::record([
        (marker, Value::record([])),
        (occurrence, text::value("unrelated stored value")),
    ]));
    world.stack.projection = projection(
        &world,
        d::partial({
            let displayed = displayed.clone();
            move |input| {
                input.value?.as_record()?.get(&marker)?;
                Some(d::at([Step::Key(occurrence)], &displayed))
            }
        }),
    );
    let before = world.model.doc.clone();
    let mut driver = Driver::new(world, Driver::VIEWPORT);
    driver.select(&[Step::Key(occurrence)]);
    assert!(copy(&mut driver));
    assert_eq!(driver.copied(), Some(displayed));
    // A cut can copy read-only data but must never delete a coincident source.
    assert!(command(&mut driver, Key::Character("x".into())));
    assert!(Rc::ptr_eq(&before, &driver.editor().model.doc));
    assert!(!driver.editor().model.history.can_undo());

    driver.select(&[Step::Key(occurrence), Step::Key(child)]);
    assert!(copy(&mut driver));
    assert_eq!(
        driver.runner.editor.text_clipboard.get_text().as_deref(),
        Some("\"computed\"")
    );
    assert_eq!(driver.copied(), None);
}

#[test]
fn copying_a_stored_or_jumped_cell_keeps_the_reference_shallow() {
    let (source, occurrence, cell) = (new_cell_id(), new_cell_id(), new_cell_id());
    for jump in [false, true] {
        let mut world = world(Value::record([(source, cell.into())]));
        Rc::make_mut(&mut world.model.doc)
            .cells
            .set_value(cell, text::value("definition"));
        world.stack.projection = projection(
            &world,
            d::partial(move |input| {
                input.value?.as_record()?.get(&source)?;
                Some(if jump {
                    d::jump([Step::Key(occurrence)], [Step::Key(source)])
                } else {
                    d::descend([Step::Key(source)], None, None)
                })
            }),
        );
        let mut driver = Driver::new(world, Driver::VIEWPORT);
        driver.select(&[Step::Key(if jump { occurrence } else { source })]);
        assert!(copy(&mut driver));
        assert_eq!(driver.copied(), Some(Value::Cell(cell)));
        assert!(command(&mut driver, Key::Character("x".into())));
        let editor = driver.editor();
        assert!(
            editor
                .sources()
                .resolve_path(&[Step::Key(source)])
                .is_none()
        );
        assert_eq!(
            editor.model.doc.cells.value(cell),
            Some(&text::value("definition"))
        );
        assert!(driver.undo());
        assert_eq!(
            driver.editor().sources().resolve_path(&[Step::Key(source)]),
            Some(&Value::Cell(cell))
        );
    }
}

#[test]
fn at_folds_are_occurrence_local_undoable_and_do_not_dirty_the_document() {
    let (marker, first, second, child) =
        (new_cell_id(), new_cell_id(), new_cell_id(), new_cell_id());
    let displayed = Value::record([(child, text::value("computed"))]);
    let mut world = world(Value::record([
        (marker, Value::record([])),
        // This location exists but is not foldable; the other does not exist.
        (first, text::value("unrelated")),
    ]));
    world.stack.projection = projection(
        &world,
        d::partial(move |input| {
            input.value?.as_record()?.get(&marker)?;
            Some(d::col(
                0,
                8.0,
                [
                    d::at([Step::Key(first)], &displayed),
                    d::at([Step::Key(second)], &displayed),
                ],
            ))
        }),
    );
    let path = [Step::Key(first)];
    let root = world.model.workspace.document_root().clone();
    let before = world.model.doc.clone();
    world.model.mark_saved();
    let mut driver = Driver::new(world, Driver::VIEWPORT);
    let folded = |driver: &Driver, path: &[Step]| {
        crate::annotations::folded(
            driver
                .editor()
                .model
                .workspace
                .view(&root)
                .unwrap()
                .annotations
                .at(path),
            false,
        )
    };
    driver.select(&path);
    assert!(command(&mut driver, Key::Named(NamedKey::ArrowUp)));
    assert!(folded(&driver, &path));
    assert!(!folded(&driver, &[Step::Key(second)]));
    assert!(Rc::ptr_eq(&before, &driver.editor().model.doc));
    assert!(!driver.editor().model.dirty());
    assert!(!driver.drawn(&[Step::Key(first), Step::Key(child)]));
    assert!(driver.drawn(&[Step::Key(second), Step::Key(child)]));
    // The folded ellipsis uses the same occurrence-local operation as the key.
    driver.click_at(driver.locate(Hovered::Tree(Hover::Toggle(Rc::from(path.clone())))));
    assert!(!folded(&driver, &path));
    assert!(driver.undo());
    assert!(folded(&driver, &path));
    assert!(driver.undo());
    assert!(!folded(&driver, &path));
    assert!(!driver.editor().model.dirty());
}

#[test]
fn line_editor_keeps_first_refusal_for_copy_and_space() {
    use puri::edit::TextClipboard;
    let mut driver = Driver::of(text::value("hello"));
    driver.select(&[]);
    for _ in 0..2 {
        assert!(driver.keystroke(Key::Named(NamedKey::ArrowLeft), Modifiers::SHIFT));
    }
    assert!(copy(&mut driver));
    assert_eq!(
        driver.runner.editor.text_clipboard.get_text().as_deref(),
        Some("lo")
    );
    assert!(driver.keystroke(Key::Character(" ".into()), Modifiers::empty()));
    assert_eq!(driver.value(&[]), text::value("hel "));
}

#[test]
fn jump_displays_and_edits_one_source_at_two_independent_occurrences() {
    let (source, first, second) = (new_cell_id(), new_cell_id(), new_cell_id());
    let mut world = world(Value::record([(source, text::value("before"))]));
    world.stack.projection = projection(
        &world,
        d::partial(move |input| {
            input.value?.as_record()?.get(&source)?;
            Some(d::col(
                0,
                8.0,
                [
                    d::jump([Step::Key(first)], [Step::Key(source)]),
                    d::jump([Step::Key(second)], [Step::Key(source)]),
                ],
            ))
        }),
    );
    let paths = [[Step::Key(first)], [Step::Key(second)]];
    let mut driver = Driver::new(world, Driver::VIEWPORT);
    assert!(driver.rect(&paths[0]).y1 < driver.rect(&paths[1]).y0);
    driver.select(&paths[1]);
    let editor = driver.editor();
    let selected = editor.model.selection.as_ref().unwrap();
    assert_eq!(
        selected.location(),
        Some(Location::root(vec![Step::Key(source)]))
    );
    assert_eq!(
        selected.value(&editor.sources()),
        Some(&text::value("before"))
    );
    // Rebuilding remints the jump's scope; this must not discard text state.
    assert!(driver.keystroke(Key::Character("!".into()), Modifiers::empty()));
    let editor = driver.editor();
    assert_eq!(
        editor.sources().resolve_path(&[Step::Key(source)]),
        Some(&text::value("before!"))
    );
    for path in &paths {
        let target = driver
            .runner
            .frame
            .dispatch
            .landmarks
            .landmark(path)
            .unwrap();
        assert_eq!(
            target.scope.view(editor.sources()).resolve_path(path),
            Some(&text::value("before!"))
        );
        assert_eq!(
            target.scope.location(path),
            Some(Location::root(vec![Step::Key(source)]))
        );
    }
    assert!(driver.undo());
    assert_eq!(
        driver.editor().sources().resolve_path(&[Step::Key(source)]),
        Some(&text::value("before"))
    );
    assert_eq!(driver.selection(), paths[1]);
}

#[test]
fn missing_jump_target_is_a_real_editable_location() {
    let (marker, missing, occurrence) = (new_cell_id(), new_cell_id(), new_cell_id());
    let mut world = world(Value::record([(marker, Value::record([]))]));
    let projection = projection(
        &world,
        d::partial(move |input| {
            input.value?.as_record()?.get(&marker)?;
            Some(d::jump([Step::Key(occurrence)], [Step::Key(missing)]))
        }),
    );
    let frame = editing_frame_with_projection(&mut world, false, Some(&projection));
    assert!((stop(&frame, &[Step::Key(occurrence)]).select)(
        &mut world, None
    ));
    let frame = editing_frame_with_projection(&mut world, false, Some(&projection));
    assert!(frame.completion.is_some());
    assert!(crate::completion::commit(
        &mut world.session(),
        text::value("created"),
        None,
        None
    ));
    assert_eq!(
        world.sources().resolve_path(&[Step::Key(missing)]),
        Some(&text::value("created"))
    );
    assert!(
        world
            .sources()
            .resolve_path(&[Step::Key(occurrence)])
            .is_none()
    );
}

/// A computed record holding a reference, drawn by `at` where the same path
/// also names stored data.
fn computed_reference() -> (crate::Editor, Projection, [CellId; 3]) {
    let (marker, source, field, cell) =
        (new_cell_id(), new_cell_id(), new_cell_id(), new_cell_id());
    let mut world = world(Value::record([
        (marker, Value::record([])),
        (source, Value::record([(field, cell.into())])),
    ]));
    Rc::make_mut(&mut world.model.doc)
        .cells
        .set_value(cell, text::value("real"));
    let value = Value::record([(field, cell.into())]);
    let projection = projection(
        &world,
        d::partial(move |input| {
            input.value?.as_record()?.get(&marker)?;
            Some(d::at([Step::Key(source)], &value))
        }),
    );
    (world, projection, [source, field, cell])
}

#[test]
fn at_and_its_descendants_have_no_source_even_when_their_paths_exist() {
    let (mut world, projection, [source, field, _]) = computed_reference();
    let before = world.model.doc.clone();
    for path in [
        vec![Step::Key(source)],
        vec![Step::Key(source), Step::Key(field)],
    ] {
        let frame = editing_frame_with_projection(&mut world, false, Some(&projection));
        assert!((stop(&frame, &path).select)(&mut world, None));
        let selected = world.model.selection.as_ref().unwrap();
        assert_eq!(selected.path(), path);
        assert!(selected.location().is_none());
        assert!(!selected.writable(&world.sources()));
        assert!(!world.paste_value(text::value("must not write")));
        assert!(!world.delete_selected_edge(Default::default()));
    }
    assert!(Rc::ptr_eq(&before, &world.model.doc));
}

#[test]
fn a_library_cell_followed_from_a_computed_value_stays_read_only() {
    let (marker, source, field) = (new_cell_id(), new_cell_id(), new_cell_id());
    let mut world = world(Value::record([(marker, Value::record([]))]));
    let value = Value::record([(field, text::ID.into())]);
    let projection = projection(
        &world,
        d::partial(move |input| {
            input.value?.as_record()?.get(&marker)?;
            Some(d::at([Step::Key(source)], &value))
        }),
    );
    let before = world.model.doc.clone();
    let frame = editing_frame_with_projection(&mut world, false, Some(&projection));
    let definition = gid::Resolution::Library(text::ID);
    let path = [
        Step::Key(source),
        Step::Key(field),
        Step::Follow(definition),
    ];
    assert!((stop(&frame, &path).select)(&mut world, None));
    let selected = world.model.selection.as_ref().unwrap();
    assert_eq!(
        selected.location(),
        Some(Location::in_cell(text::ID, definition, Vec::new()))
    );
    assert!(selected.value(&world.sources()).is_some());
    assert!(!selected.writable(&world.sources()));
    assert!(!world.paste_value(text::value("must not write")));
    assert!(!world.delete_selected_edge(Default::default()));
    assert!(Rc::ptr_eq(&before, &world.model.doc));
}

#[test]
fn a_cell_followed_from_a_computed_value_is_its_stored_definition() {
    let (mut world, projection, [source, field, cell]) = computed_reference();
    let frame = editing_frame_with_projection(&mut world, false, Some(&projection));
    let path = [
        Step::Key(source),
        Step::Key(field),
        Step::Follow(gid::Resolution::Document),
    ];
    assert!((stop(&frame, &path).select)(&mut world, None));
    let selected = world.model.selection.as_ref().unwrap();
    assert_eq!(selected.path(), path);
    assert_eq!(
        selected.location(),
        Some(Location::in_cell(
            cell,
            gid::Resolution::Document,
            Vec::new()
        ))
    );
    assert!(world.paste_value(text::value("pasted")));
    assert_eq!(
        world.model.doc.cells.value(cell),
        Some(&text::value("pasted"))
    );
    assert_eq!(
        world
            .sources()
            .resolve_path(&[Step::Key(source), Step::Key(field)]),
        Some(&cell.into()),
        "the stored path the occurrence spells is untouched"
    );
    assert!(world.delete_selected_edge(Default::default()));
    assert_eq!(world.model.doc.cells.value(cell), None);
}

#[test]
fn explicit_jump_can_establish_a_source_below_at() {
    let (marker, computed, source, occurrence) =
        (new_cell_id(), new_cell_id(), new_cell_id(), new_cell_id());
    let mut world = world(Value::record([
        (marker, Value::record([])),
        (source, text::value("source")),
    ]));
    let projection = projection(
        &world,
        d::partial(move |input| {
            let fields = input.value?.as_record()?;
            if fields.contains_key(&marker) {
                Some(d::at(
                    [Step::Key(computed)],
                    Value::record([(computed, Value::record([]))]),
                ))
            } else if fields.contains_key(&computed) {
                Some(d::jump([Step::Key(occurrence)], [Step::Key(source)]))
            } else {
                None
            }
        }),
    );
    let frame = editing_frame_with_projection(&mut world, false, Some(&projection));
    let path = [Step::Key(computed), Step::Key(occurrence)];
    let jumped = stop(&frame, &path).rect;
    let computed_rect = stop(&frame, &[Step::Key(computed)]).rect;
    assert!((stop(&frame, &path).select)(&mut world, None));
    assert!(
        world
            .model
            .selection
            .as_ref()
            .unwrap()
            .writable(&world.sources())
    );
    assert!(world.paste_value(text::value("changed")));
    assert_eq!(
        world.sources().resolve_path(&[Step::Key(source)]),
        Some(&text::value("changed"))
    );
    let drawing = settle(frame).list;
    let palette = style::Theme::Light.palette();
    assert_eq!(
        grounds(&drawing.0, palette.readonly_ground),
        [computed_rect.inset(3.0)]
    );
    assert_eq!(grounds(&drawing.0, palette.paper), [jumped.inset(3.0)]);
}

#[test]
fn multi_step_descent_keeps_stored_children_editable_without_projecting_containers() {
    let (source, child) = (new_cell_id(), new_cell_id());
    let mut world = world(Value::record([(
        source,
        Value::record([(child, text::value("deep"))]),
    )]));
    let projection = projection(
        &world,
        d::partial(move |input| {
            input.value?.as_record()?.get(&source)?;
            Some(d::descend(
                [Step::Key(source), Step::Key(child)],
                None,
                None,
            ))
        }),
    );
    let frame = editing_frame_with_projection(&mut world, false, Some(&projection));
    assert!(
        !frame
            .landmarks
            .iter()
            .any(|d| d.path.as_ref() == [Step::Key(source)])
    );
    let path = [Step::Key(source), Step::Key(child)];
    assert!((stop(&frame, &path).select)(&mut world, None));
    assert_eq!(
        world.model.selection.as_ref().unwrap().location(),
        Some(Location::root(&path[..]))
    );
    assert!(world.paste_value(text::value("changed")));
    assert_eq!(
        world.sources().resolve_path(&path),
        Some(&text::value("changed"))
    );
}
