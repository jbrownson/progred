use crate::display::test_support::Landmarks;
use crate::display::widget::navigation::Landmark;
use crate::driver::Driver;
use crate::frame::Hovered;
use crate::hover::Hover;
use crate::identity::short_id;
use crate::libraries::{f64, name, text};
use crate::projection::test_support::{Bench, editing_frame, settle};
use crate::selection::pending_value;
use crate::test_support::{EMPTY_SLOTS, positions, set_tutorial_slots};
use gid::{CellId, Cells, Document, Step, Value, new_cell_id};
use kurbo::Point;
use puri::draw::DrawCmd;
use puri::hover::Claim;
use ui_events::keyboard::NamedKey;

fn stop<'a>(
    frame: &'a crate::projection::test_support::HoverOutput<crate::Editor>,
    path: &[Step],
) -> &'a Landmark<crate::Editor> {
    frame.landmarks.landmark(path).expect("tutorial slot")
}

fn top_slots(frame: &crate::projection::test_support::HoverOutput<crate::Editor>) -> Vec<CellId> {
    frame
        .landmarks
        .iter()
        .filter_map(|target| match target.path.as_ref() {
            [Step::Key(key)] => Some(*key),
            _ => None,
        })
        .collect()
}

#[test]
fn empty_tutorial_slots_stay_visible_in_a_short_embed() {
    let (doc, names) = crate::gid_text::parse(EMPTY_SLOTS).unwrap();
    let slots = [names["first"], names["second"], names["third"]];
    let mut editor = crate::test_editor(doc);
    editor.drawn_menu = false;
    set_tutorial_slots(&mut editor, &slots);
    let size = kurbo::Size::new(600.0, 304.0);
    let driver = Driver::new(editor, size);
    for key in slots {
        let rect = driver.rect(&[Step::Key(key)]);
        assert!(
            rect.y1 <= size.height,
            "empty slot is below the embed: {rect:?}"
        );
    }
}

#[test]
fn empty_slots_make_values_through_the_picker() {
    let (doc, names) = crate::gid_text::parse(EMPTY_SLOTS).unwrap();
    let mut editor = crate::test_editor_with_stack(
        doc,
        crate::stack::load_selected(&[
            name::ID,
            text::ID,
            crate::libraries::blob::ID,
            crate::libraries::number::ID,
            f64::ID,
        ])
        .unwrap(),
    );
    set_tutorial_slots(
        &mut editor,
        &["first", "second", "third"].map(|key| names[key]),
    );
    let mut driver = Driver::new(editor, Driver::VIEWPORT);
    // `[` opens the list it makes, so 7 goes inside it.
    for (slot, typed) in [("first", "42"), ("second", "\"hello\""), ("third", "[7")] {
        driver.select(&[Step::Key(names[slot])]);
        driver.type_text(typed);
        driver.press(NamedKey::Enter);
    }
    let root = driver.value(&[]);
    let values = root.as_record().unwrap();
    assert_eq!(values.get(&names["first"]).and_then(f64::read), Some(42.0));
    assert_eq!(
        values.get(&names["second"]).and_then(text::read),
        Some("hello")
    );
    assert_eq!(
        values.get(&names["third"]),
        Some(&Value::list([f64::value(7.0)]))
    );
}

#[test]
fn tutorial_slots_keep_their_order_after_delete_refill_and_undo() {
    let slots = [new_cell_id(), new_cell_id(), new_cell_id()];
    let original = Value::record([(new_cell_id(), f64::value(2.0))]);
    let mut editor = crate::editor_of(Value::record([
        (slots[0], f64::value(1.0)),
        (slots[1], original.clone()),
        (slots[2], f64::value(3.0)),
    ]));
    set_tutorial_slots(&mut editor, &slots);
    let mut driver = Driver::new(editor, Driver::VIEWPORT);
    let top_slots = |driver: &Driver| {
        driver
            .drawn_under(&[])
            .into_iter()
            .filter_map(|(path, _)| match path.as_ref() {
                [Step::Key(key)] => Some(*key),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(top_slots(&driver), slots);
    let path = [Step::Key(slots[1])];
    driver.select(&path);
    driver.press(NamedKey::Delete);
    assert!(driver.editor().sources().resolve_path(&path).is_none());
    assert_eq!(top_slots(&driver), slots);
    let empty = driver.rect(&path);
    assert!(empty.width() > 0.0 && empty.height() > 0.0);
    assert!(driver.picker_entries().is_some());
    assert!(driver.paste(&f64::value(9.0)));
    assert_eq!(driver.value(&path), f64::value(9.0));
    assert_eq!(top_slots(&driver), slots);
    assert!(driver.undo());
    assert!(driver.editor().sources().resolve_path(&path).is_none());
    assert!(driver.undo());
    assert_eq!(driver.value(&path), original);
    assert_eq!(top_slots(&driver), slots);
}

#[test]
fn tutorial_slots_have_no_gap_insertions_and_do_not_override_children() {
    let slots = [new_cell_id(), new_cell_id()];
    let list = Value::list([f64::value(1.0), f64::value(2.0)]);
    let positions = positions(&list);
    let (a, b) = (new_cell_id(), new_cell_id());
    let mut cells = Cells::new();
    cells.set_value(a, name::record("A", []));
    cells.set_value(b, name::record("B", []));
    let mut world = crate::test_editor(Document {
        root: Some(Value::record([
            (slots[0], list),
            (
                slots[1],
                Value::record([(a, f64::value(3.0)), (b, f64::value(4.0))]),
            ),
        ])),
        cells,
    });
    set_tutorial_slots(&mut world, &slots);
    let frame = editing_frame(&mut world);
    let first = stop(&frame, &[Step::Key(slots[0])]).rect;
    let second = stop(&frame, &[Step::Key(slots[1])]).rect;
    assert!(first.y1 < second.y0);
    let point = Point::new(first.x0 + 4.0, (first.y1 + second.y0) / 2.0);
    if let Some((_, claim)) = frame.hover_geometry.probe(Some(point), None, 0.0) {
        assert!(
            matches!(claim, Claim::Direct(Hovered::Tree(Hover::Value(path))) if path.is_empty())
        );
    }
    for (slot, children) in [
        (
            slots[0],
            positions
                .iter()
                .cloned()
                .map(Step::Element)
                .collect::<Vec<_>>(),
        ),
        (slots[1], vec![Step::Key(a), Step::Key(b)]),
    ] {
        let rects = children
            .into_iter()
            .map(|step| stop(&frame, &[Step::Key(slot), step]).rect)
            .collect::<Vec<_>>();
        assert_eq!(rects[0].center().y, rects[1].center().y);
        assert!(rects[0].x1 < rects[1].x0);
    }
    let position = gid::position::between(positions.first(), positions.get(1)).unwrap();
    let path = vec![Step::Key(slots[0]), Step::Element(position)];
    world.model.selection = Some(pending_value(&crate::test_root(), path.clone()));
    assert!(editing_frame(&mut world).completion.is_some());
    assert!(crate::completion::commit(
        &mut world.session(),
        f64::value(5.0),
        None,
        None
    ));
    assert_eq!(world.sources().resolve_path(&path), Some(&f64::value(5.0)));
}

#[test]
fn peeling_switches_projections_off_and_names_last() {
    let slot = new_cell_id();
    let doc = Document {
        root: Some(Value::record([(slot, f64::value(7.0))])),
        cells: Cells::new(),
    };
    let ids = |libraries: &[CellId]| {
        libraries
            .iter()
            .map(|id| id.to_string())
            .collect::<Vec<_>>()
            .join(",")
    };
    let page = ids(&[name::ID, text::ID, f64::ID]);
    let peel = |projections: &[CellId], names: bool| {
        let stack = crate::web_embed::peeled(
            Some(&page),
            &ids(projections),
            Some(&slot.to_string()),
            names,
        )
        .unwrap();
        let mut world = crate::test_editor_with_stack(doc.clone(), stack);
        let frame = settle(editing_frame(&mut world), &world.sources());
        (world, frame)
    };
    let bytes = [Step::Key(slot), Step::Key(f64::vocabulary::F64)];
    let shown = |frame: &Bench| frame.landmarks.landmark(&bytes).is_some();
    let glyphs = |frame: &Bench| {
        frame
            .list
            .0
            .iter()
            .map(|command| match command {
                DrawCmd::GlyphRun(run) => run.glyphs.len(),
                _ => 0,
            })
            .sum::<usize>()
    };
    let (_, drawn) = peel(&[name::ID, text::ID, f64::ID], true);
    assert!(!shown(&drawn), "f64 draws the number");
    let (world, stored) = peel(&[name::ID, text::ID], true);
    assert!(shown(&stored), "without f64's projection, its bytes show");
    assert_eq!(
        crate::libraries::name::of(&world.sources(), f64::vocabulary::F64),
        Some("f64")
    );
    // Only the key's label changes: its name gives way to its identity.
    let (_, unnamed) = peel(&[name::ID, text::ID], false);
    assert!(shown(&unnamed));
    assert_eq!(
        glyphs(&unnamed) - glyphs(&stored),
        short_id(f64::vocabulary::F64).chars().count() - "f64".len()
    );
    assert!(crate::web_embed::peeled(Some(&page), "", None, false).is_err());
}

#[test]
fn tutorial_slots_fall_back_to_the_ordinary_picker_when_the_root_is_deleted() {
    let slots = [new_cell_id(), new_cell_id()];
    let mut world = crate::test_editor(Document::default());
    set_tutorial_slots(&mut world, &slots);
    let frame = editing_frame(&mut world);
    assert!(top_slots(&frame).is_empty());
    assert!(world.model.doc.root.is_none());
    assert!((stop(&frame, &[]).select)(&mut world, None));
    assert!(editing_frame(&mut world).completion.is_some());
    assert!(crate::completion::commit(
        &mut world.session(),
        Value::record([]),
        None,
        None
    ));
    assert_eq!(top_slots(&editing_frame(&mut world)), slots);
}

/// A stack of `libraries` whose projection shows `slots` as a tutorial
/// page does.
fn tutorial_stack(libraries: &[CellId], slots: &str) -> crate::stack::Stack {
    let mut stack = crate::stack::load_selected(libraries).unwrap();
    stack.projection =
        crate::web_embed::tutorial_slots(Some(slots), stack.projection, &stack.libraries).unwrap();
    stack
}

#[test]
fn tutorial_plain_and_base_slots_show_a_call_without_its_callee() {
    use crate::libraries::{absent, blob, grap as grap_library, number};
    use ::grap::vocabulary::FUNCTION;
    let (doc, names) = crate::gid_text::parse(
        r#"{
          "binders": {
            "name": 02e562654d6d0828d3a7559e6f75fffe,
            "function": 751fca4373debdd0b7e6eb73e08d684b,
            "params": 195b378d0d31d90ab0d7366c15346b70,
            "body": 986143866eda2e2fbf9ab8484357a0c9,
            "f64": ed11fde03b7c2c1ba2fccc3cdba5d561,
            "tree": 7f46dd29d8450afe5b1ef07d72098f91,
            "x": 9aa131305c35532eeb45a876bc8fdc22,
            "call": d3ff654ad635e00b1a21b6ff849231b2,
            "first": 9940ece27410c72a5308a544890ccc71,
            "second": f717b766d250a7b86c5eb842885c4417,
            "third": 5e716c07490849f072b4e9017dd6230d,
          },
          "cells": {
            x: {name: "x"},
            tree: {name: "tree", params: [x], body: x},
            call: {function: tree, x: {f64: 0x0000000000004e40}},
          },
          "root": {first: call, second: call, third: tree},
        }"#,
    )
    .unwrap();
    let mut world = crate::test_editor_with_stack(
        doc,
        tutorial_stack(
            &[
                name::ID,
                text::ID,
                blob::ID,
                absent::ID,
                number::ID,
                f64::ID,
                grap_library::ID,
            ],
            &format!(
                "{}:plain,{}:raw,{}",
                names["first"], names["second"], names["third"]
            ),
        ),
    );
    let frame = editing_frame(&mut world);
    let placed = |path: &[Step]| frame.landmarks.landmark(&path).is_some();
    let opened = |path: &[Step]| {
        frame
            .landmarks
            .iter()
            .any(|d| d.path.len() > path.len() && d.path.starts_with(path))
    };
    for slot in ["first", "second"] {
        let callee = [
            Step::Key(names[slot]),
            Step::Follow(gid::Resolution::Document),
            Step::Key(FUNCTION),
        ];
        assert!(placed(&callee), "{slot} shows its call's function");
        assert!(!opened(&callee), "{slot} doesn't inline the definition");
        assert!(placed(&[
            Step::Key(names[slot]),
            Step::Follow(gid::Resolution::Document),
            Step::Key(names["x"]),
        ]));
    }
    // A slot showing the definition itself still opens it.
    assert!(opened(&[Step::Key(names["third"])]));
}

#[test]
fn a_tutorial_slot_without_grap_shows_a_call_by_its_functions_name() {
    use crate::libraries::{blob, number};
    use ::grap::vocabulary::FUNCTION;
    let (doc, names) =
        crate::gid_text::parse(include_str!("../../../fixtures/functions.gid")).unwrap();
    let mut world = crate::test_editor_with_stack(
        doc,
        tutorial_stack(
            &[name::ID, text::ID, blob::ID, number::ID, f64::ID],
            &names["second"].to_string(),
        ),
    );
    let frame = editing_frame(&mut world);
    let callee = [
        Step::Key(names["second"]),
        Step::Key(::grap::vocabulary::EVALUATE),
        Step::Key(FUNCTION),
    ];
    assert!(frame.landmarks.landmark(&callee).is_some());
    assert!(
        !frame
            .landmarks
            .iter()
            .any(|d| d.path.len() > callee.len() && d.path.starts_with(&callee)),
        "the call shows (scale), not scale's whole definition"
    );
}

#[test]
fn swapping_libraries_keeps_the_document_and_selection() {
    use crate::libraries::{blob, number};
    let (doc, names) = crate::gid_text::parse(include_str!("../../../fixtures/model.gid")).unwrap();
    let slots = names["first"].to_string();
    let mut driver = Driver::new(
        crate::test_editor_with_stack(
            doc,
            tutorial_stack(&[name::ID, text::ID, blob::ID, number::ID, f64::ID], &slots),
        ),
        kurbo::Size::new(620.0, 400.0),
    );
    let planet = [
        Step::Key(names["first"]),
        Step::Follow(gid::Resolution::Document),
        Step::Key(names["planet"]),
    ];
    let bytes: Vec<Step> = [&planet[..], &[Step::Key(text::vocabulary::UTF8)]].concat();
    driver.select(&planet);
    assert!(
        !driver.drawn(&bytes),
        "the text library draws the planet as a line of text"
    );
    let document = driver.editor().model.doc.clone();
    driver.stack(tutorial_stack(&[blob::ID], &slots));
    assert!(std::rc::Rc::ptr_eq(&driver.editor().model.doc, &document));
    assert_eq!(driver.selection(), planet);
    assert!(
        driver.drawn(&bytes),
        "without it, the same value is a record holding bytes"
    );
}
