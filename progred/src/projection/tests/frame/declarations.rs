use super::*;
use crate::libraries::control::vocabulary as control;

#[test]
fn named_values_keep_the_name_and_expression_editable_at_their_stored_paths() {
    let cell = new_cell_id();
    let follow = Step::Follow(gid::Resolution::Document);
    let name_path = [follow.clone(), Step::Key(name::vocabulary::NAME)];
    let value_path = [follow, Step::Key(grap::vocabulary::VALUE)];
    let metadata = new_cell_id();
    let mut cells = Cells::new();
    cells.set_value(
        cell,
        name::record(
            "size",
            [
                (grap::vocabulary::VALUE, f64::value(1.0)),
                (metadata, Value::record([])),
            ],
        ),
    );
    let mut world = crate::test_editor(Document {
        root: Some(cell.into()),
        cells,
    });
    for (path, typed) in [(&name_path[..], " label"), (&value_path[..], "2")] {
        let frame = editing_frame(&mut world, false);
        // Unrelated metadata does not disqualify the wrapper projection.
        assert!(
            !frame
                .descends
                .iter()
                .any(|d| d.path.last() == Some(&Step::Key(metadata)))
        );
        let target = frame
            .descends
            .iter()
            .find(|d| d.path.as_ref() == path)
            .unwrap();
        assert!((target.select)(&mut world, None));
        let frame = editing_frame(&mut world, false);
        assert!(frame.resolve_for_dispatch().dispatch_key(
            &mut world,
            &KeyboardEvent {
                key: Key::Character(typed.into()),
                state: KeyState::Down,
                ..Default::default()
            },
        ));
    }
    assert_eq!(
        world.model.doc.cells.value(cell),
        Some(&name::record(
            "size label",
            [
                (grap::vocabulary::VALUE, f64::value(12.0)),
                (metadata, Value::record([]))
            ]
        )),
    );
}

#[test]
fn value_wrappers_always_offer_the_name_slot_and_keep_direct_references_shallow() {
    let cell = new_cell_id();
    let mut cells = Cells::new();
    cells.set_value(
        cell,
        name::record("expression", [(grap::vocabulary::VALUE, f64::value(1.0))]),
    );
    let mut world = crate::test_editor(Document {
        root: Some(Value::record([(grap::vocabulary::VALUE, cell.into())])),
        cells,
    });
    let frame = editing_frame(&mut world, false);
    let name_path = [Step::Key(name::vocabulary::NAME)];
    let name_slot = frame
        .descends
        .iter()
        .find(|d| d.path.as_ref() == name_path)
        .unwrap();
    assert!(!frame.descends.iter().any(|d| d.path.as_ref()
        == [
            Step::Key(grap::vocabulary::VALUE),
            Step::Follow(gid::Resolution::Document)
        ]));
    let before = world.model.doc.clone();
    assert!((name_slot.select)(&mut world, None));
    assert!(editing_frame(&mut world, false).completion.is_some());
    assert!(
        Rc::ptr_eq(&world.model.doc, &before),
        "projecting and selecting a missing name must not write one"
    );
}

fn declarations(binder: CellId) -> Vec<(Value, Path)> {
    let parameters = Value::list([binder.into()]);
    let position = parameters.as_list().unwrap().keys().next().unwrap().clone();
    let lambda = Value::record([
        (grap::vocabulary::PARAMS, parameters),
        (grap::vocabulary::BODY, binder.into()),
    ]);
    let mut forms = vec![(
        lambda,
        vec![Step::Key(grap::vocabulary::PARAMS), Step::Element(position)],
    )];
    for function in [control::LET, control::WHERE] {
        let bindings = Value::list([Value::record([
            (control::BIND, binder.into()),
            (control::VALUE, f64::value(1.0)),
        ])]);
        let position = bindings.as_list().unwrap().keys().next().unwrap().clone();
        forms.push((
            grap::call(
                function.into(),
                [
                    (control::BINDINGS, bindings),
                    (grap::vocabulary::EXPRESSION, binder.into()),
                ],
            ),
            vec![
                Step::Key(control::BINDINGS),
                Step::Element(position),
                Step::Key(control::BIND),
            ],
        ));
    }
    let cases = Value::list([Value::record([
        (
            control::PATTERN,
            Value::list([Value::record([(control::BIND, binder.into())])]),
        ),
        (grap::vocabulary::EXPRESSION, binder.into()),
    ])]);
    let (position, case) = cases.as_list().unwrap().iter().next().unwrap();
    let pattern = case.as_record().unwrap().get(&control::PATTERN).unwrap();
    let element = pattern.as_list().unwrap().keys().next().unwrap().clone();
    let path = vec![
        Step::Key(control::CASES),
        Step::Element(position.clone()),
        Step::Key(control::PATTERN),
        Step::Element(element),
        Step::Key(control::BIND),
    ];
    forms.push((
        grap::call(
            control::MATCH.into(),
            [
                (control::VALUE, Value::list([f64::value(1.0)])),
                (control::CASES, cases),
            ],
        ),
        path,
    ));
    forms
}

#[test]
fn declarations_keep_cell_handles_real_name_paths_and_editing() {
    let binder = new_cell_id();
    let libraries = core_libraries();
    for (root, binder_path) in declarations(binder) {
        let mut cells = Cells::new();
        cells.set_value(binder, name::record("size", []));
        let mut doc = Rc::new(Document {
            root: Some(root),
            cells,
        });
        let name_path = binder_path
            .iter()
            .cloned()
            .chain([
                Step::Follow(gid::Resolution::Document),
                Step::Key(name::vocabulary::NAME),
            ])
            .collect::<Path>();
        let (bench, _) = place(&doc, None, 1200.0);
        let rect = |path: &[Step]| {
            bench
                .descends
                .iter()
                .find(|d| d.path.as_ref() == path)
                .unwrap()
                .rect
        };
        let cell_rect = rect(&binder_path);
        let name_rect = rect(&name_path);
        let (hovered, _) = place_with_pointer(&doc, None, 1200.0, Some(name_rect.center()));
        assert!(matches!(hovered.hit,
            Some(Claim::Direct(Hovered::Tree(Hover::Value(found)))) if *found == name_path));
        let point = Point::new((cell_rect.x0 + name_rect.x0) / 2.0, cell_rect.center().y);
        let (hovered, _) = place_with_pointer(&doc, None, 1200.0, Some(point));
        assert!(matches!(hovered.hit,
            Some(Claim::Direct(Hovered::Tree(Hover::Value(found)))) if *found == binder_path));
        let mut selected = make_projected_editing_selection(&doc, &libraries, name_path.clone());
        assert_eq!(selected.edit().unwrap().text(), "size");
        selected.edit_mut().unwrap().set_text("width");
        assert!(write_text(&mut doc, &libraries, &mut selected));
        assert_eq!(
            src(&doc, &libraries).resolve_path(&name_path),
            Some(&text::value("width"))
        );
    }
}

#[test]
fn declaration_names_ignore_metadata_but_raw_and_field_insertion_keep_it_accessible() {
    let binder = new_cell_id();
    let extra = new_cell_id();
    let nested = new_cell_id();
    let (root, binder_path) = declarations(binder).remove(0);
    let mut cells = Cells::new();
    cells.set_value(binder, name::record("size", [(extra, nested.into())]));
    cells.set_value(nested, name::record("metadata", []));
    let doc = Document {
        root: Some(root),
        cells,
    };
    let definition = binder_path
        .iter()
        .cloned()
        .chain([Step::Follow(gid::Resolution::Document)])
        .collect::<Path>();
    let extra_path = definition
        .iter()
        .cloned()
        .chain([Step::Key(extra)])
        .collect::<Path>();
    let libraries = core_libraries();
    let mut world = editing_world(&doc, &libraries);
    assert!(
        !editing_frame(&mut world, false)
            .descends
            .iter()
            .any(|d| d.path.as_ref() == &extra_path)
    );
    assert!(
        editing_frame(&mut world, true)
            .descends
            .iter()
            .any(|d| d.path.as_ref() == &extra_path)
    );
    let pending = pending_edge(
        &crate::test_root(),
        &src(&doc, &libraries),
        definition.clone(),
    )
    .unwrap();
    world.model.selection = Some(pending);
    assert!(editing_frame(&mut world, false).completion.is_some());
}

#[test]
fn specialized_lists_keep_missing_entries_and_record_insertions_visible() {
    let binder = new_cell_id();
    let libraries = core_libraries();
    for (root, binder_path) in declarations(binder) {
        let mut cells = Cells::new();
        cells.set_value(binder, name::record("size", []));
        let doc = Document {
            root: Some(root),
            cells,
        };
        let sources = src(&doc, &libraries);
        let list_path = &binder_path[..1];
        let pending = pending_into(&crate::test_root(), &sources, list_path).unwrap();
        let missing_path = pending.path().to_vec();
        assert!(sources.resolve_path(&missing_path).is_none());
        let mut world = editing_world(&doc, &libraries);
        world.model.selection = Some(pending);
        let frame = editing_frame(&mut world, false);
        assert!(frame.completion.is_some());
        assert!(
            frame
                .descends
                .iter()
                .any(|d| d.path.as_ref() == missing_path)
        );
        assert!(
            src(&world.model.doc, &libraries)
                .resolve_path(&missing_path)
                .is_none()
        );

        // A new field in a compact lambda, match, or let must not disappear.
        world.model.selection = pending_edge(&crate::test_root(), &sources, Vec::new());
        assert!(editing_frame(&mut world, false).completion.is_some());
        if binder_path.len() > 2 {
            world.model.selection =
                pending_edge(&crate::test_root(), &sources, binder_path[..2].to_vec());
            assert!(editing_frame(&mut world, false).completion.is_some());
        }
    }
}

#[test]
fn use_site_overrides_do_not_hide_definitions_inside_inert_arguments() {
    let binder = new_cell_id();
    let function = new_cell_id();
    let direct = new_cell_id();
    let data = new_cell_id();
    let field = new_cell_id();
    let quoted = new_cell_id();
    let nested = new_cell_id();
    let root = grap::lambda(
        [binder],
        grap::call(
            function.into(),
            [
                (direct, binder.into()),
                (data, Value::record([(field, binder.into())])),
                (
                    quoted,
                    grap::call(
                        control::QUOTE.into(),
                        [(
                            grap::vocabulary::EXPRESSION,
                            Value::record([(field, binder.into())]),
                        )],
                    ),
                ),
                (nested, grap::lambda([binder], binder.into())),
            ],
        ),
    );
    let mut cells = Cells::new();
    cells.set_value(binder, name::record("size", []));
    cells.set_value(function, name::record("function", []));
    let doc = Document {
        root: Some(root),
        cells,
    };
    let (bench, _) = place(&doc, None, 1200.0);
    let has = |path: &[Step]| bench.descends.iter().any(|d| d.path.as_ref() == path);
    let body = Step::Key(grap::vocabulary::BODY);
    let follow = Step::Follow(gid::Resolution::Document);
    assert!(!has(&[body.clone(), Step::Key(direct), follow.clone()]));
    assert!(!has(&[
        body.clone(),
        Step::Key(grap::vocabulary::FUNCTION),
        follow.clone()
    ]));
    assert!(has(&[
        body.clone(),
        Step::Key(data),
        Step::Key(field),
        follow.clone()
    ]));
    assert!(has(&[
        body.clone(),
        Step::Key(quoted),
        Step::Key(grap::vocabulary::EXPRESSION),
        Step::Key(field),
        follow.clone()
    ]));
    assert!(!has(&[
        body.clone(),
        Step::Key(nested),
        body.clone(),
        follow
    ]));
    let param_list = src(&doc, &core_libraries())
        .resolve_path(&[
            body.clone(),
            Step::Key(nested),
            Step::Key(grap::vocabulary::PARAMS),
        ])
        .unwrap()
        .as_list()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    assert!(has(&[
        body,
        Step::Key(nested),
        Step::Key(grap::vocabulary::PARAMS),
        Step::Element(param_list),
        Step::Follow(gid::Resolution::Document),
        Step::Key(name::vocabulary::NAME)
    ]));
}

#[test]
fn recursive_patterns_keep_text_and_number_facets() {
    let binder = new_cell_id();
    let field = new_cell_id();
    let pattern = Value::record([(
        field,
        Value::list([
            text::value("literal"),
            f64::value(3.0),
            Value::record([(control::BIND, binder.into())]),
        ]),
    )]);
    let elements = pattern
        .as_record()
        .unwrap()
        .get(&field)
        .unwrap()
        .as_list()
        .unwrap()
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    let cases = Value::list([Value::record([
        (control::PATTERN, pattern),
        (grap::vocabulary::EXPRESSION, binder.into()),
    ])]);
    let case = cases.as_list().unwrap().keys().next().unwrap().clone();
    let mut cells = Cells::new();
    cells.set_value(binder, name::record("captured", []));
    let doc = Document {
        cells,
        root: Some(grap::call(
            control::MATCH.into(),
            [(control::VALUE, Value::record([])), (control::CASES, cases)],
        )),
    };
    let prefix = vec![
        Step::Key(control::CASES),
        Step::Element(case),
        Step::Key(control::PATTERN),
        Step::Key(field),
    ];
    let (bench, _) = place(&doc, None, 1200.0);
    for position in &elements[..2] {
        let path = prefix
            .iter()
            .cloned()
            .chain([Step::Element(position.clone())])
            .collect::<Path>();
        assert!(bench.descends.iter().any(|d| d.path.as_ref() == path));
        assert!(
            !bench
                .descends
                .iter()
                .any(|d| d.path.starts_with(&path) && d.path.len() > path.len())
        );
    }
    let name_path = prefix
        .into_iter()
        .chain([
            Step::Element(elements[2].clone()),
            Step::Key(control::BIND),
            Step::Follow(gid::Resolution::Document),
            Step::Key(name::vocabulary::NAME),
        ])
        .collect::<Path>();
    assert!(bench.descends.iter().any(|d| d.path.as_ref() == name_path));
}

#[test]
fn unquote_preserves_expression_paths_and_local_shallow_references() {
    let binding = new_cell_id();
    let field = new_cell_id();
    let mut cells = Cells::new();
    cells.set_value(binding, name::record("size", []));
    let expression_path = vec![
        Step::Key(grap::vocabulary::EXPRESSION),
        Step::Key(control::UNQUOTE),
    ];
    for expression in [binding.into(), Value::record([(field, binding.into())])] {
        let direct_reference = expression.as_cell().is_some();
        let doc = Document {
            root: Some(grap::call(
                control::QUOTE.into(),
                [(
                    grap::vocabulary::EXPRESSION,
                    Value::record([(control::UNQUOTE, expression)]),
                )],
            )),
            cells: cells.clone(),
        };
        let (bench, _) = place(&doc, None, 1200.0);
        let has = |path: &[Step]| bench.descends.iter().any(|d| d.path.as_ref() == path);
        assert!(has(&expression_path));
        let mut followed = expression_path.clone();
        if !direct_reference {
            followed.push(Step::Key(field));
        }
        followed.push(Step::Follow(gid::Resolution::Document));
        assert_eq!(has(&followed), !direct_reference);

        let marker_position = |descends: &[Descend<World>]| {
            let wrapper = descends
                .iter()
                .find(|d| d.path.as_ref() == &expression_path[..1])
                .unwrap()
                .rect;
            let body = descends
                .iter()
                .find(|d| d.path.as_ref() == expression_path)
                .unwrap()
                .rect;
            Point::new((wrapper.x0 + body.x0) / 2.0, wrapper.center().y)
        };
        let marker = marker_position(&bench.descends);
        let (hovered, _) = place_with_pointer(&doc, None, 1200.0, Some(marker));
        assert!(matches!(hovered.hit,
            Some(Claim::Direct(Hovered::Tree(Hover::Value(found)))) if found.as_ref() == &expression_path[..1]));

        let mut world = crate::test_editor(doc);
        let marker = marker_position(&editing_frame(&mut world, false).descends);
        let frame = editing_frame_at(&mut world, false, None, Some(marker));
        let (_, Claim::Direct(hovered)) = frame.claim.as_ref().unwrap() else {
            panic!("backtick hover");
        };
        let mut dispatch =
            placed::DispatchContext::new(Some(crate::test_root()), Some(hovered.clone()));
        let event = PointerButtonEvent {
            button: Some(PointerButton::Primary),
            pointer: PointerInfo {
                pointer_id: Some(PointerId::PRIMARY),
                persistent_device_id: None,
                pointer_type: PointerType::Mouse,
            },
            state: PointerState {
                position: (marker.x, marker.y).into(),
                ..Default::default()
            },
        };
        assert!(frame.resolve_for_dispatch().dispatch_pointer_down_with(
            &mut world,
            &event,
            &mut dispatch,
        ));
        assert_eq!(
            world.model.selection.as_ref().unwrap().path(),
            &expression_path[..1]
        );
    }
}

#[test]
fn quote_prefixes_tolerate_extra_fields_and_keep_field_insertion_available() {
    let extra = new_cell_id();
    for root in [
        Value::record([(control::UNQUOTE, text::value("expression"))]),
        grap::call(
            control::QUOTE.into(),
            [(grap::vocabulary::EXPRESSION, text::value("template"))],
        ),
    ] {
        let mut world = crate::test_editor(Document {
            cells: Cells::new(),
            root: Some(Value::record(
                root.as_record()
                    .unwrap()
                    .update(extra, text::value("metadata")),
            )),
        });
        let has_metadata = |frame: &crate::placed::HoverOutput<World>| {
            frame
                .descends
                .iter()
                .any(|d| d.path.as_ref() == [Step::Key(extra)])
        };
        assert!(!has_metadata(&editing_frame(&mut world, false)));
        assert!(has_metadata(&editing_frame(&mut world, true)));
        world.model.selection = pending_edge(&crate::test_root(), &world.sources(), Vec::new());
        assert!(editing_frame(&mut world, false).completion.is_some());
    }
}

#[test]
fn compact_grap_forms_ignore_metadata_but_keep_raw_and_field_insertion_available() {
    let extra = new_cell_id();
    let binder = new_cell_id();
    let mut forms = declarations(binder)
        .into_iter()
        .map(|(root, _)| root)
        .collect::<Vec<_>>();
    forms.extend([
        grap::call(
            control::DO.into(),
            [(control::EXPRESSIONS, Value::list([]))],
        ),
        grap::call(
            f64::vocabulary::SUM.into(),
            [
                (f64::vocabulary::LEFT, f64::value(1.0)),
                (f64::vocabulary::RIGHT, f64::value(2.0)),
            ],
        ),
    ]);
    let libraries = core_libraries();
    for root in forms {
        let doc = Document {
            cells: Cells::new(),
            root: Some(root.clone()),
        };
        let mut world = editing_world(&doc, &libraries);
        world.model.selection =
            pending_edge(&crate::test_root(), &src(&doc, &libraries), Vec::new());
        assert!(editing_frame(&mut world, false).completion.is_some());

        let doc = Document {
            cells: Cells::new(),
            root: Some(Value::record(
                root.as_record()
                    .unwrap()
                    .update(extra, text::value("metadata")),
            )),
        };
        let mut world = editing_world(&doc, &libraries);
        assert!(
            !editing_frame(&mut world, false)
                .descends
                .iter()
                .any(|d| d.path.as_ref() == [Step::Key(extra)])
        );
        assert!(
            editing_frame(&mut world, true)
                .descends
                .iter()
                .any(|d| d.path.as_ref() == [Step::Key(extra)])
        );
    }
}
