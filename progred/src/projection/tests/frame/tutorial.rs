use super::*;

fn configure(world: &mut crate::Editor, slots: &[CellId]) {
    world.stack.projection = crate::web_embed::tutorial_slots(
        Some(
            &slots
                .iter()
                .map(|id| id.simple().to_string())
                .collect::<Vec<_>>()
                .join(","),
        ),
        world.stack.projection.clone(),
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
