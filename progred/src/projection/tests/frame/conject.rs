use super::*;
use crate::display as d;

fn world(root: Value) -> crate::Editor {
    crate::test_editor(Document {
        root: Some(root),
        cells: Cells::new(),
    })
}

fn projection(
    world: &crate::Editor,
    rule: d::Partial<crate::Editor, Hovered>,
) -> Projection<crate::Editor> {
    Projection::new([rule, world.stack.projection.partial.clone()])
}

fn stop<'a>(
    frame: &'a placed::HoverOutput<crate::Editor>,
    path: &[Step],
) -> &'a Descend<crate::Editor> {
    frame
        .descends
        .iter()
        .find(|d| d.path.as_ref() == path)
        .expect("projected occurrence")
}

fn command(key: Key) -> KeyboardEvent {
    KeyboardEvent {
        key,
        state: KeyState::Down,
        modifiers: if cfg!(target_os = "macos") {
            ui_events::keyboard::Modifiers::META
        } else {
            ui_events::keyboard::Modifiers::CONTROL
        },
        ..Default::default()
    }
}

fn grounds(commands: &[DrawCmd], color: Color) -> Vec<Rect> {
    commands
        .iter()
        .flat_map(|command| match command {
            DrawCmd::Clip { children, .. } => grounds(children, color),
            DrawCmd::Fill {
                shape: Shape::RoundedRect(rect),
                brush: Brush::Solid(brush),
                ..
            } if *brush == color => vec![rect.rect()],
            _ => vec![],
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
    let tints = grounds(
        &drawing.0,
        crate::styles::Theme::Light.palette().readonly_ground,
    );
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
    let tints = grounds(
        &drawing.0,
        crate::styles::Theme::Light.palette().readonly_ground,
    );
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
    let frame = editing_frame(&mut world, false);
    assert!((stop(&frame, &child_path).select)(&mut world, None));
    assert_eq!(world.model.selection.as_ref().unwrap().path(), child_path);
    assert!(
        world
            .model
            .selection
            .as_ref()
            .unwrap()
            .scope()
            .source(&child_path)
            .is_none()
    );
    let frame = editing_frame(&mut world, false);
    assert!(
        frame
            .resolve_for_dispatch()
            .dispatch_key(&mut world, &command(Key::Character("c".into())))
    );
    assert_eq!(
        world.clipboard_structure(),
        value.as_record().unwrap().get(&child).cloned()
    );

    let frame = editing_frame(&mut world, false);
    assert!((stop(&frame, &result).select)(&mut world, None));
    let frame = editing_frame(&mut world, false);
    assert!(
        frame
            .resolve_for_dispatch()
            .dispatch_key(&mut world, &command(Key::Named(NamedKey::ArrowUp)))
    );
    let annotations = world.model.workspace.document.annotations.clone();
    let frame = editing_frame_with_annotations(&mut world, false, None, None, &annotations);
    assert!(!frame.descends.iter().any(|d| d.path.as_ref() == child_path));
    // The stored expression remains projected, not folded with its result.
    stop(&frame, &[Step::Key(grap::vocabulary::EVALUATE)]);
    assert!(Rc::ptr_eq(&before, &world.model.doc));
    assert!(!world.model.dirty());
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
        let mut world = world(Value::record([(
            RENDER,
            Value::record([
                (grap::vocabulary::EXPRESSION, nested),
                (layout::vocabulary::FUEL, f64::value(3.0)),
            ]),
        )]));
        let frame = editing_frame(&mut world, false);
        let path = [Step::Key(RESULT), Step::Key(RESULT)];
        assert!((stop(&frame, &path).select)(&mut world, None));
        let frame = editing_frame(&mut world, false);
        assert!(
            frame
                .resolve_for_dispatch()
                .dispatch_key(&mut world, &command(Key::Character("c".into())))
        );
        assert_eq!(world.clipboard_structure(), Some(value.clone()));
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
    let projection = projection(
        &world,
        d::partial({
            let displayed = displayed.clone();
            move |input| {
                input.value?.as_record()?.get(&marker)?;
                Some(d::at([Step::Key(occurrence)], &displayed))
            }
        }),
    );
    let path = [Step::Key(occurrence)];
    let before = world.model.doc.clone();
    let frame = editing_frame_with_projection(&mut world, false, Some(&projection));
    assert!((stop(&frame, &path).select)(&mut world, None));
    let frame = editing_frame_with_projection(&mut world, false, Some(&projection));
    let handler = frame.resolve_for_dispatch();
    assert!(handler.dispatch_key(&mut world, &command(Key::Character("c".into()))));
    assert_eq!(world.clipboard_structure(), Some(displayed));
    // A cut can copy read-only data but must never delete a coincident source.
    assert!(handler.dispatch_key(&mut world, &command(Key::Character("x".into()))));
    assert!(Rc::ptr_eq(&before, &world.model.doc));
    assert!(!world.model.history.can_undo());

    let child_path = [Step::Key(occurrence), Step::Key(child)];
    let frame = editing_frame_with_projection(&mut world, false, Some(&projection));
    assert!((stop(&frame, &child_path).select)(&mut world, None));
    let frame = editing_frame_with_projection(&mut world, false, Some(&projection));
    assert!(
        frame
            .resolve_for_dispatch()
            .dispatch_key(&mut world, &command(Key::Character("c".into())),)
    );
    assert_eq!(
        world.text_clipboard.get_text().as_deref(),
        Some("\"computed\"")
    );
    assert_eq!(world.clipboard_structure(), None);
}

#[test]
fn copying_a_stored_or_jumped_cell_keeps_the_reference_shallow() {
    let (source, occurrence, cell) = (new_cell_id(), new_cell_id(), new_cell_id());
    for jump in [false, true] {
        let mut world = world(Value::record([(source, cell.into())]));
        Rc::make_mut(&mut world.model.doc)
            .cells
            .set_value(cell, text::value("definition"));
        let projection = projection(
            &world,
            d::partial(move |input| {
                input.value?.as_record()?.get(&source)?;
                Some(if jump {
                    d::jump([Step::Key(occurrence)], [Step::Key(source)])
                } else {
                    d::descend(Step::Key(source), None, None)
                })
            }),
        );
        let path = [Step::Key(if jump { occurrence } else { source })];
        let frame = editing_frame_with_projection(&mut world, false, Some(&projection));
        assert!((stop(&frame, &path).select)(&mut world, None));
        let frame = editing_frame_with_projection(&mut world, false, Some(&projection));
        let handler = frame.resolve_for_dispatch();
        assert!(handler.dispatch_key(&mut world, &command(Key::Character("c".into())),));
        assert_eq!(world.clipboard_structure(), Some(Value::Cell(cell)));
        assert!(handler.dispatch_key(&mut world, &command(Key::Character("x".into())),));
        assert!(world.sources().resolve_path(&[Step::Key(source)]).is_none());
        assert_eq!(
            world.model.doc.cells.value(cell),
            Some(&text::value("definition"))
        );
        assert!(world.model.step_history(true));
        assert_eq!(
            world.sources().resolve_path(&[Step::Key(source)]),
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
    let projection = projection(
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
    let frame = editing_frame_with_projection(&mut world, false, Some(&projection));
    assert!((stop(&frame, &path).select)(&mut world, None));
    let frame = editing_frame_with_projection(&mut world, false, Some(&projection));
    assert!(
        frame
            .resolve_for_dispatch()
            .dispatch_key(&mut world, &command(Key::Named(NamedKey::ArrowUp)),)
    );
    let annotations = world
        .model
        .workspace
        .view(&root)
        .unwrap()
        .annotations
        .clone();
    assert!(crate::annotations::collapsed(&annotations, &path, false));
    assert!(!crate::annotations::collapsed(
        &annotations,
        &[Step::Key(second)],
        false
    ));
    assert!(Rc::ptr_eq(&before, &world.model.doc));
    assert!(!world.model.dirty());
    let folded =
        editing_frame_with_annotations(&mut world, false, Some(&projection), None, &annotations);
    assert!(
        !folded
            .descends
            .iter()
            .any(|d| d.path.as_ref() == [Step::Key(first), Step::Key(child)])
    );
    assert!(
        folded
            .descends
            .iter()
            .any(|d| d.path.as_ref() == [Step::Key(second), Step::Key(child)])
    );
    // The folded ellipsis uses the same occurrence-local operation as the key.
    let target = Hovered::Tree(Hover::Toggle(Rc::from(path.clone())));
    assert!(folded.resolve_for_dispatch().dispatch_pointer_down_with(
        &mut world,
        &PointerButtonEvent {
            button: Some(PointerButton::Primary),
            pointer: PointerInfo {
                pointer_id: Some(PointerId::PRIMARY),
                persistent_device_id: None,
                pointer_type: PointerType::Mouse,
            },
            state: PointerState {
                position: (5.0, 5.0).into(),
                ..Default::default()
            },
        },
        &mut placed::DispatchContext::new(Some(root.clone()), Some(target)),
    ));
    assert!(!crate::annotations::collapsed(
        &world.model.workspace.view(&root).unwrap().annotations,
        &path,
        false
    ));
    assert!(world.model.step_history(true));
    assert!(crate::annotations::collapsed(
        &world.model.workspace.view(&root).unwrap().annotations,
        &path,
        false
    ));
    assert!(world.model.step_history(true));
    assert!(!crate::annotations::collapsed(
        &world.model.workspace.view(&root).unwrap().annotations,
        &path,
        false
    ));
    assert!(!world.model.dirty());
}

#[test]
fn line_editor_keeps_first_refusal_for_copy_and_space() {
    use puri::edit::TextClipboard;
    let mut world = world(text::value("hello"));
    let frame = editing_frame(&mut world, false);
    assert!((stop(&frame, &[]).select)(&mut world, None));
    for _ in 0..2 {
        let frame = editing_frame(&mut world, false);
        assert!(frame.resolve_for_dispatch().dispatch_key(
            &mut world,
            &KeyboardEvent {
                key: Key::Named(NamedKey::ArrowLeft),
                state: KeyState::Down,
                modifiers: ui_events::keyboard::Modifiers::SHIFT,
                ..Default::default()
            }
        ));
    }
    let frame = editing_frame(&mut world, false);
    let handler = frame.resolve_for_dispatch();
    assert!(handler.dispatch_key(&mut world, &command(Key::Character("c".into()))));
    assert_eq!(world.text_clipboard.get_text().as_deref(), Some("lo"));
    assert!(handler.dispatch_key(
        &mut world,
        &KeyboardEvent {
            key: Key::Character(" ".into()),
            state: KeyState::Down,
            ..Default::default()
        }
    ));
    assert_eq!(world.model.doc.root, Some(text::value("hel ")));
}

#[test]
fn jump_displays_and_edits_one_source_at_two_independent_occurrences() {
    let (source, first, second) = (new_cell_id(), new_cell_id(), new_cell_id());
    let mut world = world(Value::record([(source, text::value("before"))]));
    let projection = projection(
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
    let frame = editing_frame_with_projection(&mut world, false, Some(&projection));
    assert!(stop(&frame, &paths[0]).rect.y1 < stop(&frame, &paths[1]).rect.y0);
    assert!((stop(&frame, &paths[1]).select)(&mut world, None));
    let selected = world.model.selection.as_ref().unwrap();
    assert_eq!(selected.path(), paths[1]);
    assert_eq!(
        selected.source_path().unwrap().as_ref(),
        [Step::Key(source)]
    );
    assert_eq!(
        selected.value(&world.sources()),
        Some(&text::value("before"))
    );
    // Rebuilding remints the conject closure; this must not discard text state.
    let frame = editing_frame_with_projection(&mut world, false, Some(&projection));
    assert!(frame.resolve_for_dispatch().dispatch_key(
        &mut world,
        &KeyboardEvent {
            key: Key::Character("!".into()),
            state: KeyState::Down,
            ..Default::default()
        }
    ));
    assert_eq!(
        world.sources().resolve_path(&[Step::Key(source)]),
        Some(&text::value("before!"))
    );
    let frame = editing_frame_with_projection(&mut world, false, Some(&projection));
    for path in &paths {
        let target = stop(&frame, path);
        assert_eq!(
            target.scope.read(&world.sources(), path),
            Some(&text::value("before!"))
        );
        assert_eq!(
            target.scope.source(path).unwrap().as_ref(),
            [Step::Key(source)]
        );
    }
    assert!(world.model.step_history(true));
    assert_eq!(
        world.sources().resolve_path(&[Step::Key(source)]),
        Some(&text::value("before"))
    );
    assert_eq!(world.model.selection.as_ref().unwrap().path(), paths[1]);
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
    assert!(world.commit_completion(text::value("created"), None, None));
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

#[test]
fn at_and_its_descendants_have_no_source_even_when_their_paths_exist() {
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
    let before = world.model.doc.clone();
    let frame = editing_frame_with_projection(&mut world, false, Some(&projection));
    let path = [
        Step::Key(source),
        Step::Key(field),
        Step::Follow(gid::Resolution::Document),
    ];
    assert!((stop(&frame, &path).select)(&mut world, None));
    let selected = world.model.selection.as_ref().unwrap();
    assert_eq!(selected.path(), path);
    assert!(selected.source_path().is_none());
    assert!(!selected.writable(&world.sources()));
    assert!(!world.paste_value(text::value("must not write")));
    assert!(!world.delete_selected_edge(Default::default()));
    let frame = editing_frame_with_projection(&mut world, false, Some(&projection));
    assert!(frame.completion.is_none());
    assert!(Rc::ptr_eq(&before, &world.model.doc));
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
                    &Value::record([(computed, Value::record([]))]),
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
    let palette = crate::styles::Theme::Light.palette();
    assert_eq!(
        grounds(&drawing.0, palette.readonly_ground),
        [computed_rect.inset(3.0)]
    );
    assert_eq!(grounds(&drawing.0, palette.paper), [jumped.inset(3.0)]);
}

#[test]
fn custom_conject_is_used_for_both_descendant_reads_and_writes() {
    let (marker, source, occurrence, shown, real) = (
        new_cell_id(),
        new_cell_id(),
        new_cell_id(),
        new_cell_id(),
        new_cell_id(),
    );
    let mut world = world(Value::record([
        (marker, Value::record([])),
        (source, Value::record([(real, text::value("actual"))])),
    ]));
    let projection = projection(
        &world,
        d::partial(move |input| {
            input.value?.as_record()?.get(&marker)?;
            Some(d::jump_with_conject(
                [Step::Key(occurrence)],
                [Step::Key(source)],
                d::Conject::new(move |path, document| {
                    Some(
                        document
                            .iter()
                            .cloned()
                            .chain(path.iter().map(|step| {
                                if *step == Step::Key(shown) {
                                    Step::Key(real)
                                } else {
                                    step.clone()
                                }
                            }))
                            .collect(),
                    )
                }),
                Some(d::partial(move |_| {
                    Some(d::descend(Step::Key(shown), None, None))
                })),
                None,
            ))
        }),
    );
    let frame = editing_frame_with_projection(&mut world, false, Some(&projection));
    let path = [Step::Key(occurrence), Step::Key(shown)];
    assert!((stop(&frame, &path).select)(&mut world, None));
    assert_eq!(
        world
            .model
            .selection
            .as_ref()
            .unwrap()
            .value(&world.sources()),
        Some(&text::value("actual"))
    );
    assert!(world.paste_value(text::value("new")));
    assert_eq!(
        world
            .sources()
            .resolve_path(&[Step::Key(source), Step::Key(real)]),
        Some(&text::value("new"))
    );
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
            Some(d::descend_path([Step::Key(source), Step::Key(child)]))
        }),
    );
    let frame = editing_frame_with_projection(&mut world, false, Some(&projection));
    assert!(
        !frame
            .descends
            .iter()
            .any(|d| d.path.as_ref() == [Step::Key(source)])
    );
    let path = [Step::Key(source), Step::Key(child)];
    assert!((stop(&frame, &path).select)(&mut world, None));
    assert_eq!(
        world
            .model
            .selection
            .as_ref()
            .unwrap()
            .source_path()
            .unwrap()
            .as_ref(),
        path
    );
    assert!(world.paste_value(text::value("changed")));
    assert_eq!(
        world.sources().resolve_path(&path),
        Some(&text::value("changed"))
    );
}
