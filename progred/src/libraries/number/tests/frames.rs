use super::*;
use crate::display::test_support::press_at;
use crate::display::test_support::{Landmarks, ResolveForDispatch};
use crate::frame::Hovered;
use crate::hover::Hover;
use crate::libraries::{f32, f64, text, u64};
use crate::projection::test_support::*;
use crate::selection::pending_value;
use crate::test_support::key_down;
use crate::test_values::numeric_calls;
use gid::{Cells, Document, Step, new_cell_id};
use kurbo::Point;
use puri::handler::test_support::DispatchShorthands;
use puri::hover::Claim;
use ui_events::keyboard::{Key, Modifiers};

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
        let named = before.overlaid(name::record("tilt", []));
        let mut cells = Cells::new();
        cells.set_value(cell, named);
        let mut world = crate::test_editor(Document {
            root: Some(cell.into()),
            cells,
        });
        for (path, typed) in [(&name_path[..], " angle"), (&number_path[..], "5")] {
            let frame = editing_frame(&mut world);
            let target = frame.landmarks.landmark(path).unwrap();
            assert!((target.select)(&mut world, None));
            let frame = editing_frame(&mut world);
            assert!(frame.resolve_for_dispatch().dispatch_key(
                &mut world,
                &key_down(Key::Character(typed.into()), Modifiers::empty()),
            ));
        }
        assert_eq!(world.model.doc.root, Some(cell.into()));
        assert_eq!(
            world.model.doc.cells.value(cell),
            Some(&after.overlaid(name::record("tilt angle", []))),
        );
    }
}

#[test]
fn numeric_scrubbing_includes_the_representation_but_not_the_name() {
    for number in [f64::value(45.0), f32::value(45.0), u64::value(45)] {
        for (contact, pick) in [
            ("name", true),
            ("digits", true),
            ("representation", true),
            ("representation", false),
        ] {
            let name_contact = contact == "name";
            let scrub = pick && !name_contact;
            let value = number.overlaid(name::record("tilt", []));
            let cell = new_cell_id();
            let number_path = vec![Step::Follow(gid::Resolution::Document)];
            let name_path = vec![number_path[0].clone(), Step::Key(name::vocabulary::NAME)];
            let mut cells = Cells::new();
            cells.set_value(cell, value.clone());
            let mut world = crate::test_editor(Document {
                root: Some(cell.into()),
                cells,
            });
            let frame = editing_frame(&mut world);
            let path = if name_contact {
                name_path.clone()
            } else {
                number_path.clone()
            };
            let rect = frame.landmarks.landmark(&name_path).unwrap().rect;
            let number_rect = frame.landmarks.landmark(&number_path).unwrap().rect;
            let point = match contact {
                "name" => rect.center(),
                "digits" => Point::new(rect.x1 + 8.0, rect.center().y),
                _ => Point::new(number_rect.x1 - 0.5, number_rect.y1 - 0.5),
            };
            let frame = framed(
                &mut world,
                Framing {
                    pointer: Some(point),
                    ..Framing::default()
                },
            );
            let Some((_, Claim::Direct(hovered))) = frame.claim.as_ref() else {
                panic!("direct hover on {contact}")
            };
            assert_eq!(
                *hovered,
                Hovered::Tree(Hover::Value(Rc::from(path.clone())))
            );
            let mut dispatch =
                crate::frame::DispatchContext::new(Some(crate::test_root()), Some(hovered.clone()));
            frame.resolve_for_dispatch().dispatch_pointer_down_with(
                &mut world,
                &press_at(
                    point,
                    if pick {
                        crate::test_support::command()
                    } else {
                        Modifiers::empty()
                    },
                ),
                &mut dispatch,
            );
            assert_eq!(world.gesture.is_some(), scrub, "{contact}");
            if !name_contact {
                assert_eq!(world.model.selection.as_ref().unwrap().path(), path);
            }
            assert_eq!(world.model.doc.cells.value(cell), Some(&value));
            world.advance_gesture(&[Point::new(point.x + 48.0, point.y)]);
            assert_eq!(world.model.doc.root, Some(cell.into()));
            let edited = world.model.doc.cells.value(cell).unwrap();
            assert_eq!(name::read(edited), Some("tilt"));
            assert_eq!(*edited == value, !scrub);
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
        let mut world = crate::editor_of(number.clone());
        let plain = editing_frame(&mut world);
        assert!(!plain.landmarks.landmark(&path).is_some());
        world.model.selection = Some(pending_value(
            &crate::test_root(),
            Default::default(),
            path.to_vec(),
        ));
        assert!(editing_frame(&mut world).completion.is_some());
        world.model.selection = None;
        for name in [text::value(""), Value::list([text::value("unusual name")])] {
            Rc::make_mut(&mut world.model.doc).root =
                Some(number.overlaid(Value::record([(name::vocabulary::NAME, name)])));
            let frame = editing_frame(&mut world);
            assert!(frame.landmarks.landmark(&path).is_some());
        }
    }
}

#[test]
fn numeric_operation_labels_preserve_function_hover_and_selection() {
    let function_path = [Step::Key(grap::vocabulary::FUNCTION)];
    for (_, _, call) in numeric_calls() {
        let doc = Document {
            root: Some(call),
            cells: Cells::new(),
        };
        let (bench, _) = place(&doc, None, 900.0);
        let head = bench.landmarks.landmark(&function_path).unwrap();
        assert!(
            !bench
                .landmarks
                .iter()
                .any(|d| d.path.iter().any(|s| matches!(s, Step::Follow(_))))
        );
        for x in [head.rect.x0 + 0.5, head.rect.x1 - 0.5] {
            let (hovered, _) = place_with(BenchFrame {
                pointer: Some(Point::new(x, head.rect.center().y)),
                ..BenchFrame::of(&doc, 900.0)
            });
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
    for (representation, function, call) in numeric_calls() {
        let mut doc = Document {
            root: Some(call),
            cells: Cells::new(),
        };
        let width = |doc: &Document| {
            let (bench, _) = place(doc, None, 900.0);
            bench
                .landmarks
                .landmark(&function_path)
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
    for (_, _, call) in numeric_calls() {
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
        let raw = framed(
            &mut world,
            Framing {
                raw: true,
                ..Framing::default()
            },
        );
        for field in doc.root.as_ref().unwrap().as_record().unwrap().keys() {
            if *field != extra {
                assert!(bench.landmarks.landmark(&[Step::Key(*field)]).is_some());
            }
            assert!(raw.landmarks.landmark(&[Step::Key(*field)]).is_some());
        }
    }
}
