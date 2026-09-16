use super::*;
use crate::libraries::{f32, number, u64};

#[test]
fn named_numbers_edit_the_name_and_numeric_facet_at_their_own_locations() {
    let cell = new_cell_id();
    let number_path = [Step::Follow(gid::Resolution::Document)];
    let name_path = [number_path[0].clone(), Step::Key(name::vocabulary::NAME)];
    for (before, after) in [
        (f64::value(45.0), f64::value(455.0)),
        (f32::value(45.0), f32::value(455.0)),
        (u64::value(45), u64::value(455)),
    ] {
        let named = crate::display::overlay_value(&before, name::record("tilt", []));
        let mut cells = Cells::new();
        cells.set_value(cell, named);
        let mut world = crate::test_editor(Document {
            root: Some(cell.into()),
            cells,
        });
        for (path, typed) in [(&name_path[..], " angle"), (&number_path[..], "5")] {
            let frame = editing_frame(&mut world, false);
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
        assert_eq!(world.model.doc.root, Some(cell.into()));
        assert_eq!(
            world.model.doc.cells.value(cell),
            Some(&crate::display::overlay_value(
                &after,
                name::record("tilt angle", [])
            )),
        );
    }
}

#[test]
fn numeric_scrubbing_does_not_extend_to_the_name() {
    for number in [f64::value(45.0), f32::value(45.0), u64::value(45)] {
        for name_contact in [true, false] {
            let value = crate::display::overlay_value(&number, name::record("tilt", []));
            let mut world = crate::test_editor(Document {
                root: Some(value.clone()),
                cells: Cells::new(),
            });
            let frame = editing_frame(&mut world, false);
            let path = if name_contact {
                vec![Step::Key(name::vocabulary::NAME)]
            } else {
                vec![]
            };
            let rect = frame
                .descends
                .iter()
                .find(|d| d.path.as_ref() == [Step::Key(name::vocabulary::NAME)])
                .unwrap()
                .rect;
            let point = if name_contact {
                rect.center()
            } else {
                Point::new(rect.x1 + 8.0, rect.center().y)
            };
            let frame = editing_frame_at(&mut world, false, None, Some(point));
            let (_, Claim::Direct(hovered)) = frame.claim.as_ref().unwrap() else {
                panic!("direct hover")
            };
            assert_eq!(*hovered, Hovered::Tree(Hover::Value(Rc::from(path))));
            let mut dispatch =
                placed::DispatchContext::new(Some(crate::test_root()), Some(hovered.clone()));
            frame.resolve_for_dispatch().dispatch_pointer_down_with(
                &mut world,
                &PointerButtonEvent {
                    button: Some(PointerButton::Primary),
                    pointer: PointerInfo {
                        pointer_id: Some(PointerId::PRIMARY),
                        persistent_device_id: None,
                        pointer_type: PointerType::Mouse,
                    },
                    state: PointerState {
                        position: (point.x, point.y).into(),
                        modifiers: Modifiers::META | Modifiers::CONTROL,
                        ..Default::default()
                    },
                },
                &mut dispatch,
            );
            assert_eq!(world.gesture.is_some(), !name_contact);
            world.advance_gesture(&[Point::new(point.x + 48.0, point.y)]);
            let edited = world.model.doc.root.as_ref().unwrap();
            assert_eq!(name::read(edited), Some("tilt"));
            assert_eq!(*edited == value, name_contact);
        }
    }
}

#[test]
fn numeric_name_decoration_keeps_missing_selected_and_unusual_names_accessible() {
    let path = [Step::Key(name::vocabulary::NAME)];
    for number in [
        f64::value(1.0),
        f64::value(f64::INFINITY),
        f32::value(1.0),
        u64::value(1),
    ] {
        let mut world = crate::test_editor(Document {
            root: Some(number.clone()),
            cells: Cells::new(),
        });
        let plain = editing_frame(&mut world, false);
        assert!(!plain.descends.iter().any(|d| d.path.as_ref() == path));
        world.model.selection = Some(pending_value(&crate::test_root(), path.to_vec()));
        assert!(editing_frame(&mut world, false).completion.is_some());
        world.model.selection = None;
        for name in [text::value(""), Value::list([text::value("unusual name")])] {
            Rc::make_mut(&mut world.model.doc).root = Some(crate::display::overlay_value(
                &number,
                Value::record([(name::vocabulary::NAME, name)]),
            ));
            let frame = editing_frame(&mut world, false);
            assert!(frame.descends.iter().any(|d| d.path.as_ref() == path));
        }
    }
}

pub(super) fn calls() -> Vec<(CellId, CellId, Value)> {
    [
        (f64::vocabulary::F64, f64::vocabulary::SUM, f64::value(1.0)),
        (f32::vocabulary::F32, f32::vocabulary::SUM, f32::value(2.0)),
        (u64::vocabulary::U64, u64::vocabulary::LESS, u64::value(3)),
    ]
    .into_iter()
    .map(|(representation, function, value)| {
        (
            representation,
            function,
            grap::call(
                function.into(),
                [
                    (number::vocabulary::LEFT, value.clone()),
                    (number::vocabulary::RIGHT, value),
                ],
            ),
        )
    })
    .chain([(
        f64::vocabulary::F64,
        f64::vocabulary::SIN,
        grap::call(
            f64::vocabulary::SIN.into(),
            [(number::vocabulary::OPERAND, f64::value(0.5))],
        ),
    )])
    .collect()
}

#[test]
fn numeric_operation_labels_preserve_function_hover_and_selection() {
    let function_path = [Step::Key(grap::vocabulary::FUNCTION)];
    for (_, _, call) in calls() {
        let doc = Document {
            root: Some(call),
            cells: Cells::new(),
        };
        let (bench, _) = place(&doc, None, 900.0);
        let head = bench
            .descends
            .iter()
            .find(|d| d.path.as_ref() == function_path)
            .unwrap();
        assert!(
            !bench
                .descends
                .iter()
                .any(|d| d.path.iter().any(|s| matches!(s, Step::Follow(_))))
        );
        for x in [head.rect.x0 + 0.5, head.rect.x1 - 0.5] {
            let (hovered, _) =
                place_with_pointer(&doc, None, 900.0, Some(Point::new(x, head.rect.center().y)));
            assert!(matches!(hovered.hit,
                Some(Claim::Direct(Hovered::Tree(Hover::Value(found)))) if found.as_ref() == function_path));
        }
        let mut editor = crate::test_editor(doc.clone());
        let original = editor.model.doc.clone();
        assert!((head.select)(&mut editor, None));
        assert_eq!(
            editor.model.selection.as_ref().unwrap().path(),
            function_path
        );
        assert!(Rc::ptr_eq(&editor.model.doc, &original));
    }
}

#[test]
fn numeric_operation_labels_read_both_names_from_the_graph() {
    let function_path = [Step::Key(grap::vocabulary::FUNCTION)];
    for (representation, function, call) in calls() {
        let mut doc = Document {
            root: Some(call),
            cells: Cells::new(),
        };
        let width = |doc: &Document| {
            let (bench, _) = place(doc, None, 900.0);
            bench
                .descends
                .iter()
                .find(|d| d.path.as_ref() == function_path)
                .unwrap()
                .rect
                .width()
        };
        let original = width(&doc);
        doc.cells.set_value(
            representation,
            name::record("a longer representation name", []),
        );
        let renamed_type = width(&doc);
        assert!(renamed_type > original);
        doc.cells
            .set_value(function, name::record("a longer operation name", []));
        assert!(width(&doc) > renamed_type);
    }
}

#[test]
fn numeric_calls_keep_consumed_fields_projected_and_extras_available_in_raw() {
    let extra = new_cell_id();
    for (_, _, call) in calls() {
        let call = Value::record(
            call.as_record()
                .unwrap()
                .clone()
                .update(extra, text::value("extra")),
        );
        let doc = Document {
            root: Some(call),
            cells: Cells::new(),
        };
        let (bench, _) = place(&doc, None, 900.0);
        let mut world = crate::test_editor(doc.clone());
        let raw = editing_frame(&mut world, true);
        for field in doc.root.as_ref().unwrap().as_record().unwrap().keys() {
            if *field != extra {
                assert!(
                    bench
                        .descends
                        .iter()
                        .any(|d| d.path.as_ref() == [Step::Key(*field)])
                );
            }
            assert!(
                raw.descends
                    .iter()
                    .any(|d| d.path.as_ref() == [Step::Key(*field)])
            );
        }
    }
}
