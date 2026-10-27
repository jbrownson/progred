use super::*;
use crate::display::test_support::Landmarks;
use crate::display::widget::navigation::Landmark;
use crate::driver::Driver;
use crate::frame::Hovered;
use crate::frame::test_support::editing_frame;
use crate::hover::Hover;
use crate::libraries::grap::control::vocabulary as control;
use crate::libraries::{f64, text};
use crate::projection::test_support::*;
use crate::selection::{pending_edge, pending_into};
use gid::Path;
use kurbo::Point;
use puri::hover::Claim;
use std::rc::Rc;

#[test]
fn expression_cells_are_shallow_only_with_a_valid_name() {
    use grap::vocabulary::{EXPRESSION, FUNCTION};

    let cell = new_cell_id();
    for (definition, named) in [
        (Some(name::record("amount", [])), true),
        (Some(f64::value(3.0)), false),
        (
            Some(Value::record([(name::vocabulary::NAME, f64::value(3.0))])),
            false,
        ),
        (None, false),
    ] {
        let mut cells = Cells::new();
        if let Some(definition) = definition {
            cells.set_value(cell, definition);
        }
        for (root, path, expression) in [
            (cell.into(), vec![], false),
            (
                crate::libraries::grap::evaluation(cell.into()),
                vec![Step::Key(EXPRESSION)],
                true,
            ),
            (grap::call(cell.into(), []), vec![Step::Key(FUNCTION)], true),
            (
                grap::call(
                    crate::libraries::grap::numbers::f64::SUM.into(),
                    [
                        (
                            crate::libraries::grap::numbers::vocabulary::LEFT,
                            cell.into(),
                        ),
                        (
                            crate::libraries::grap::numbers::vocabulary::RIGHT,
                            f64::value(2.0),
                        ),
                    ],
                ),
                vec![Step::Key(crate::libraries::grap::numbers::vocabulary::LEFT)],
                true,
            ),
        ] {
            let doc = Document {
                root: Some(root),
                cells: cells.clone(),
            };
            let (frame, _) = place(&doc, None, 1200.0);
            let followed = path
                .iter()
                .cloned()
                .chain([Step::Follow(gid::Resolution::Document)])
                .collect::<Path>();
            assert!(frame.landmarks.landmark(&path).is_some());
            assert_eq!(
                frame.landmarks.landmark(&followed).is_some(),
                !(expression && named),
                "named={named}, expression={expression}, path={path:?}",
            );
        }
    }
}

#[test]
fn unnamed_expression_cells_edit_the_shared_definition() {
    let cell = new_cell_id();
    let expression = grap::call(
        crate::libraries::grap::numbers::f64::SUM.into(),
        [
            (
                crate::libraries::grap::numbers::vocabulary::LEFT,
                cell.into(),
            ),
            (
                crate::libraries::grap::numbers::vocabulary::RIGHT,
                cell.into(),
            ),
        ],
    );
    let mut cells = Cells::new();
    cells.set_value(cell, f64::value(3.0));
    let mut driver = Driver::new(
        crate::test_editor(Document {
            root: Some(expression.clone()),
            cells,
        }),
        Driver::VIEWPORT,
    );
    driver.select(&[
        Step::Key(crate::libraries::grap::numbers::vocabulary::LEFT),
        Step::Follow(gid::Resolution::Document),
    ]);
    driver.type_text("4");
    let editor = driver.editor();
    assert_eq!(editor.model.doc.root.as_ref(), Some(&expression));
    assert_eq!(editor.model.doc.cells.value(cell), Some(&f64::value(34.0)));
    let evaluation = grap::evaluate_value(&expression, &editor.sources(), grap::DEFAULT_FUEL);
    assert!(evaluation.completed);
    assert_eq!(f64::read(&evaluation.result), Some(68.0));
}

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
                (
                    ::grap::isa::ISA,
                    gid::Value::from(::grap::vocabulary::kind::VALUE_WRAPPER).into(),
                ),
                (grap::vocabulary::VALUE, f64::value(1.0)),
                (metadata, Value::record([])),
            ],
        ),
    );
    let mut driver = Driver::new(
        crate::test_editor(Document {
            root: Some(cell.into()),
            cells,
        }),
        Driver::VIEWPORT,
    );
    for (path, typed) in [(&name_path[..], " label"), (&value_path[..], "2")] {
        // Unrelated metadata doesn't disqualify the wrapper; it follows it.
        assert!(
            driver
                .drawn_under::<Step>(&[])
                .iter()
                .any(|(path, _)| path.last() == Some(&crate::occurrence::Step::Key(metadata)))
        );
        driver.select(path);
        driver.type_text(typed);
    }
    assert_eq!(
        driver.editor().model.doc.cells.value(cell),
        Some(&name::record(
            "size label",
            [
                (
                    ::grap::isa::ISA,
                    gid::Value::from(::grap::vocabulary::kind::VALUE_WRAPPER).into()
                ),
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
        name::record(
            "expression",
            [
                (
                    ::grap::isa::ISA,
                    gid::Value::from(::grap::vocabulary::kind::VALUE_WRAPPER).into(),
                ),
                (grap::vocabulary::VALUE, f64::value(1.0)),
            ],
        ),
    );
    let mut world = crate::test_editor(Document {
        root: Some(Value::record([
            (
                ::grap::isa::ISA,
                gid::Value::from(::grap::vocabulary::kind::VALUE_WRAPPER).into(),
            ),
            (grap::vocabulary::VALUE, cell.into()),
        ])),
        cells,
    });
    let frame = editing_frame(&mut world);
    let name_path = [Step::Key(name::vocabulary::NAME)];
    let name_slot = frame.landmarks.landmark(&name_path).unwrap();
    assert!(!frame.landmarks.iter().any(|d| d.path
        == [
            Step::Key(grap::vocabulary::VALUE),
            Step::Follow(gid::Resolution::Document)
        ][..]));
    let before = world.model.doc.clone();
    assert!((name_slot.select)(&mut world, None));
    assert!(editing_frame(&mut world).completion.is_some());
    assert!(
        Rc::ptr_eq(&world.model.doc, &before),
        "projecting and selecting a missing name must not write one"
    );
}

#[test]
fn anonymous_lambda_marker_opens_its_name_for_typing_when_clicked() {
    let mut driver = Driver::of(grap::lambda([], f64::value(3.0)));
    let before = driver.editor().model.doc.clone();
    let name_path = [Step::Key(name::vocabulary::NAME)];
    driver.click(&name_path);
    assert_eq!(driver.selection(), name_path);
    assert_eq!(driver.stage(), Some(crate::selection::Stage::Pending));
    assert!(Rc::ptr_eq(&driver.editor().model.doc, &before));
    driver.type_text("f");
    assert_eq!(driver.value(&name_path), text::value("f"));
}

fn declarations(binder: CellId) -> Vec<(Value, Path)> {
    let parameters = Value::list([binder.into()]);
    let position = parameters.as_list().unwrap().keys().next().unwrap().clone();
    let lambda = Value::record([
        (
            ::grap::isa::ISA,
            gid::Value::from(::grap::vocabulary::kind::LAMBDA).into(),
        ),
        (grap::vocabulary::PARAMS, parameters),
        (grap::vocabulary::BODY, binder.into()),
    ]);
    let mut forms = vec![(
        lambda,
        vec![Step::Key(grap::vocabulary::PARAMS), Step::Element(position)],
    )];
    for function in [control::LET, control::WHERE] {
        let bindings = Value::list([Value::record([
            (::grap::isa::ISA, control::kind::BINDING.into()),
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
        (::grap::isa::ISA, control::kind::CASE.into()),
        (
            control::PATTERN,
            Value::list([Value::record([
                (::grap::isa::ISA, control::kind::BINDER.into()),
                (control::BIND, binder.into()),
            ])]),
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
        let rect = |path: &[Step]| bench.landmarks.landmark(path).unwrap().rect;
        let cell_rect = rect(&binder_path);
        let name_rect = rect(&name_path);
        let (hovered, _) = place_with(BenchFrame {
            pointer: Some(name_rect.center()),
            ..BenchFrame::of(&doc, 1200.0)
        });
        assert!(matches!(hovered.hit,
            Some(Claim::Direct(Hovered::Tree(Hover::Value(found)))) if found == name_path));
        let point = Point::new((cell_rect.x0 + name_rect.x0) / 2.0, cell_rect.center().y);
        let (hovered, _) = place_with(BenchFrame {
            pointer: Some(point),
            ..BenchFrame::of(&doc, 1200.0)
        });
        assert!(matches!(hovered.hit,
            Some(Claim::Direct(Hovered::Tree(Hover::Value(found)))) if found == binder_path));
        let mut selected =
            make_projected_editing_selection(&doc, &libraries, crate::occurrence::of(&name_path));
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
fn declaration_names_draw_metadata_after_them_and_take_new_fields() {
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
    let mut world = crate::test_editor(doc.clone());
    assert!(
        editing_frame(&mut world)
            .landmarks
            .landmark(&extra_path)
            .is_some()
    );
    let pending = pending_edge(
        &crate::editing::Read {
            sources: src(&doc, &libraries),
        },
        &crate::test_document(),
        crate::occurrence::of(&definition),
    )
    .unwrap();
    world.model.selection = Some(pending);
    assert!(editing_frame(&mut world).completion.is_some());
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
        let read = crate::editing::Read { sources };
        let list_path = crate::occurrence::of(&binder_path[..1]);
        let pending = pending_into(&read, &crate::test_document(), &list_path, true).unwrap();
        let missing_path = pending.path().to_vec();
        assert!(crate::occurrence::read(&sources, &missing_path).is_none());
        let mut world = crate::test_editor(doc.clone());
        world.model.selection = Some(pending);
        let frame = editing_frame(&mut world);
        assert!(frame.completion.is_some());
        assert!(frame.landmarks.landmark(&missing_path).is_some());
        assert!(
            crate::occurrence::read(&src(&world.model.doc, &libraries), &missing_path).is_none()
        );

        // A new field in a compact lambda, match, or let must not disappear.
        world.model.selection = pending_edge(&read, &crate::test_document(), Vec::new());
        assert!(editing_frame(&mut world).completion.is_some());
        if binder_path.len() > 2 {
            world.model.selection = pending_edge(
                &read,
                &crate::test_document(),
                crate::occurrence::of(&binder_path[..2]),
            );
            assert!(editing_frame(&mut world).completion.is_some());
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
    let has = |path: &[Step]| bench.landmarks.landmark(&path).is_some();
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
            Value::record([
                (::grap::isa::ISA, control::kind::BINDER.into()),
                (control::BIND, binder.into()),
            ]),
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
        (::grap::isa::ISA, control::kind::CASE.into()),
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
            .map(crate::occurrence::Step::from)
            .collect::<Vec<_>>();
        assert!(bench.landmarks.landmark(&path).is_some());
        assert!(
            !bench
                .landmarks
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
    assert!(bench.landmarks.landmark(&name_path).is_some());
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
                    Value::record([
                        (
                            ::grap::isa::ISA,
                            gid::Value::from(control::kind::UNQUOTE).into(),
                        ),
                        (control::UNQUOTE, expression),
                    ]),
                )],
            )),
            cells: cells.clone(),
        };
        let (bench, _) = place(&doc, None, 1200.0);
        let has = |path: &[Step]| bench.landmarks.landmark(&path).is_some();
        assert!(has(&expression_path));
        let mut followed = expression_path.clone();
        if !direct_reference {
            followed.push(Step::Key(field));
        }
        followed.push(Step::Follow(gid::Resolution::Document));
        assert_eq!(has(&followed), !direct_reference);

        let marker_position = |landmarks: &[Landmark<crate::Editor>]| {
            let wrapper = landmarks.landmark(&expression_path[..1]).unwrap().rect;
            let body = landmarks.landmark(&expression_path).unwrap().rect;
            Point::new((wrapper.x0 + body.x0) / 2.0, wrapper.center().y)
        };
        let marker = marker_position(&bench.landmarks);
        let (hovered, _) = place_with(BenchFrame {
            pointer: Some(marker),
            ..BenchFrame::of(&doc, 1200.0)
        });
        assert!(matches!(hovered.hit,
            Some(Claim::Direct(Hovered::Tree(Hover::Value(found)))) if found == expression_path[..1]));

        let mut driver = Driver::new(crate::test_editor(doc), Driver::VIEWPORT);
        driver.click_at(marker_position(&driver.runner.frame.dispatch.landmarks));
        assert_eq!(driver.selection(), &expression_path[..1]);
    }
}

#[test]
fn quote_prefixes_draw_extra_fields_after_them_and_keep_field_insertion_available() {
    let extra = new_cell_id();
    for root in [
        Value::record([
            (
                ::grap::isa::ISA,
                gid::Value::from(control::kind::UNQUOTE).into(),
            ),
            (control::UNQUOTE, text::value("expression")),
        ]),
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
                    .with(extra, text::value("metadata")),
            )),
        });
        let has_metadata = |frame: &crate::projection::test_support::HoverOutput<crate::Editor>| {
            frame.landmarks.landmark(&[Step::Key(extra)]).is_some()
        };
        assert!(has_metadata(&editing_frame(&mut world)));
        world.model.selection = pending_edge(
            &crate::editing::Read {
                sources: world.sources(),
            },
            &crate::test_document(),
            Vec::new(),
        );
        assert!(editing_frame(&mut world).completion.is_some());
    }
}

#[test]
fn infix_draws_other_fields_after_it_and_adds_fields_there() {
    let extra = new_cell_id();
    let sum = grap::call(
        crate::libraries::grap::numbers::f64::SUM.into(),
        [
            (
                crate::libraries::grap::numbers::vocabulary::LEFT,
                f64::value(1.0),
            ),
            (
                crate::libraries::grap::numbers::vocabulary::RIGHT,
                f64::value(2.0),
            ),
        ],
    );
    let libraries = core_libraries();
    let doc = Document {
        cells: Cells::new(),
        root: Some(sum.clone()),
    };
    let mut world = crate::test_editor(doc.clone());
    world.model.selection = pending_edge(
        &crate::editing::Read {
            sources: src(&doc, &libraries),
        },
        &crate::test_document(),
        Vec::new(),
    );
    assert!(editing_frame(&mut world).completion.is_some());
    let doc = Document {
        cells: Cells::new(),
        root: Some(Value::record(
            sum.as_record()
                .unwrap()
                .with(extra, text::value("metadata")),
        )),
    };
    let mut world = crate::test_editor(doc.clone());
    assert!(
        editing_frame(&mut world)
            .landmarks
            .landmark(&[Step::Key(extra)])
            .is_some()
    );
    // Chosen alone, the operator reads infix with its operands as slots and
    // the first one's picker open.
    let doc = Document {
        cells: Cells::new(),
        root: Some(grap::call(
            crate::libraries::grap::numbers::f64::SUM.into(),
            [],
        )),
    };
    let mut world = crate::test_editor(doc.clone());
    world.model.selection = Some(crate::selection::pending_value(
        &crate::test_document(),
        crate::occurrence::of(&[Step::Key(crate::libraries::grap::numbers::vocabulary::LEFT)]),
    ));
    let frame = editing_frame(&mut world);
    assert!(frame.completion.is_some());
    assert!(
        frame
            .landmarks
            .landmark(&[Step::Key(
                crate::libraries::grap::numbers::vocabulary::RIGHT
            )])
            .is_some()
    );
}

#[test]
fn compact_grap_forms_draw_other_fields_after_them_and_keep_field_insertion_available() {
    let extra = new_cell_id();
    let binder = new_cell_id();
    let mut forms = declarations(binder)
        .into_iter()
        .map(|(root, _)| root)
        .collect::<Vec<_>>();
    forms.push(grap::call(
        control::DO.into(),
        [(control::EXPRESSIONS, Value::list([]))],
    ));
    let libraries = core_libraries();
    for root in forms {
        let doc = Document {
            cells: Cells::new(),
            root: Some(root.clone()),
        };
        let mut world = crate::test_editor(doc.clone());
        world.model.selection = pending_edge(
            &crate::editing::Read {
                sources: src(&doc, &libraries),
            },
            &crate::test_document(),
            Vec::new(),
        );
        assert!(editing_frame(&mut world).completion.is_some());

        let doc = Document {
            cells: Cells::new(),
            root: Some(Value::record(
                root.as_record()
                    .unwrap()
                    .with(extra, text::value("metadata")),
            )),
        };
        let mut world = crate::test_editor(doc.clone());
        assert!(
            editing_frame(&mut world)
                .landmarks
                .landmark(&[Step::Key(extra)])
                .is_some()
        );
    }
}
