use super::*;
use crate::libraries::control::vocabulary as control;

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
        assert!(write_through(&mut doc, &libraries, &mut selected));
        assert_eq!(
            src(&doc, &libraries).resolve_path(&name_path),
            Some(&text::value("width"))
        );
    }
}

#[test]
fn declaration_names_do_not_hide_metadata_or_new_fields() {
    let binder = new_cell_id();
    let extra = new_cell_id();
    let nested = new_cell_id();
    let (root, binder_path) = declarations(binder).remove(0);
    let mut cells = Cells::new();
    cells.set_value(binder, name::record("size", [(extra, nested.into())]));
    cells.set_value(nested, name::record("metadata", []));
    let mut doc = Document {
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
    let (bench, _) = place(&doc, None, 1200.0);
    assert!(
        bench
            .descends
            .iter()
            .any(|d| d.path.as_ref() == &extra_path)
    );
    let libraries = core_libraries();
    doc.cells.set_value(binder, name::record("size", []));
    let pending = pending_edge(
        &crate::test_root(),
        &src(&doc, &libraries),
        definition.clone(),
    )
    .unwrap();
    let mut world = editing_world(&doc, &libraries);
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
fn compact_grap_forms_expose_extra_fields_and_active_insertions() {
    let extra = new_cell_id();
    let binder = new_cell_id();
    let mut forms = declarations(binder)
        .into_iter()
        .map(|(root, _)| root)
        .collect::<Vec<_>>();
    forms.extend([
        grap::call(
            control::QUOTE.into(),
            [(grap::vocabulary::EXPRESSION, text::value("template"))],
        ),
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
        let (bench, _) = place(&doc, None, 1200.0);
        assert!(
            bench
                .descends
                .iter()
                .any(|d| d.path.as_ref() == [Step::Key(extra)])
        );
    }
}
