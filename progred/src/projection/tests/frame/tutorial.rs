use super::*;

fn configure(world: &mut crate::Editor, slots: &[CellId]) {
    world.stack.projection = crate::web_embed::tutorial_slots(
        Some(
            &slots
                .iter()
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(","),
        ),
        world.stack.projection.clone(),
        &world.stack.libraries,
    )
    .unwrap();
}

fn stop<'a>(
    frame: &'a placed::HoverOutput<crate::Editor>,
    path: &[Step],
) -> &'a Descend<crate::Editor> {
    frame
        .descends
        .iter()
        .find(|target| target.path.as_ref() == path)
        .expect("tutorial slot")
}

fn top_slots(frame: &placed::HoverOutput<crate::Editor>) -> Vec<CellId> {
    frame
        .descends
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
    configure(&mut editor, &slots);
    let mut runner = crate::EditorRunner::new(editor);
    let size = kurbo::Size::new(600.0, 304.0);
    runner.refresh_frame(1.0, size);
    for key in slots {
        let target = runner
            .frame
            .dispatch
            .descends
            .iter()
            .find(|target| target.path.as_ref() == [Step::Key(key)])
            .unwrap();
        assert!(
            target.rect.y1 <= size.height,
            "empty slot is below the embed: {:?}",
            target.rect
        );
    }
}

#[test]
fn tutorial_slots_keep_their_order_after_delete_refill_and_undo() {
    let slots = [new_cell_id(), new_cell_id(), new_cell_id()];
    let original = Value::record([(new_cell_id(), f64::value(2.0))]);
    let mut world = crate::test_editor(Document {
        root: Some(Value::record([
            (slots[0], f64::value(1.0)),
            (slots[1], original.clone()),
            (slots[2], f64::value(3.0)),
        ])),
        cells: Cells::new(),
    });
    configure(&mut world, &slots);
    let frame = editing_frame(&mut world, false);
    assert_eq!(top_slots(&frame), slots);
    let path = [Step::Key(slots[1])];
    assert!((stop(&frame, &path).select)(&mut world, None));
    assert!(world.delete_selected_edge(crate::navigate::Geometry {
        descends: &frame.descends,
        ..Default::default()
    }));
    assert!(world.sources().resolve_path(&path).is_none());
    let frame = editing_frame(&mut world, false);
    assert_eq!(top_slots(&frame), slots);
    let empty = stop(&frame, &path);
    assert!(empty.rect.width() > 0.0 && empty.rect.height() > 0.0);
    assert!((empty.select)(&mut world, None));
    assert!(editing_frame(&mut world, false).completion.is_some());
    assert!(world.commit_completion(f64::value(9.0), None, None));
    assert_eq!(world.sources().resolve_path(&path), Some(&f64::value(9.0)));
    assert_eq!(top_slots(&editing_frame(&mut world, false)), slots);
    assert!(world.model.step_history(true));
    assert!(world.sources().resolve_path(&path).is_none());
    assert!(world.model.step_history(true));
    assert_eq!(world.sources().resolve_path(&path), Some(&original));
    assert_eq!(top_slots(&editing_frame(&mut world, false)), slots);
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
    configure(&mut world, &slots);
    let frame = editing_frame(&mut world, false);
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
    assert!(editing_frame(&mut world, false).completion.is_some());
    assert!(world.commit_completion(f64::value(5.0), None, None));
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
        let frame = settle(editing_frame(&mut world, false));
        (world, frame)
    };
    let bytes = [Step::Key(slot), Step::Key(f64::vocabulary::F64)];
    let shown = |frame: &Bench| frame.descends.iter().any(|d| d.path.as_ref() == bytes);
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
    assert_eq!(world.sources().name(f64::vocabulary::F64), Some("f64"));
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
    let mut world = crate::test_editor(Document {
        root: None,
        cells: Cells::new(),
    });
    configure(&mut world, &slots);
    let frame = editing_frame(&mut world, false);
    assert!(top_slots(&frame).is_empty());
    assert!(world.model.doc.root.is_none());
    assert!((stop(&frame, &[]).select)(&mut world, None));
    assert!(editing_frame(&mut world, false).completion.is_some());
    assert!(world.commit_completion(Value::record([]), None, None));
    assert_eq!(top_slots(&editing_frame(&mut world, false)), slots);
}
