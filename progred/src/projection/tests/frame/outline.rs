use super::*;
use crate::libraries::presentation::vocabulary::{OUTLINE, RESULT};

fn document() -> (Document, CellId, CellId, CellId) {
    let (a, b, extra) = (new_cell_id(), new_cell_id(), new_cell_id());
    let mut cells = Cells::new();
    for (id, label) in [(a, "A"), (b, "B"), (extra, "Extra")] {
        cells.set_value(id, name::record(label, []));
    }
    (
        Document {
            root: Some(Value::record([
                (OUTLINE, Value::list([b.into(), a.into()])),
                (a, Value::list([f64::value(1.0), f64::value(2.0)])),
                (b, f64::value(3.0)),
                (extra, f64::value(4.0)),
            ])),
            cells,
        },
        a,
        b,
        extra,
    )
}

fn frame(world: &mut crate::Editor) -> placed::HoverOutput<crate::Editor> {
    let annotations = world.model.workspace.document.annotations.clone();
    editing_frame_with_annotations(world, false, None, None, &annotations)
}

fn stop<'a>(
    frame: &'a placed::HoverOutput<crate::Editor>,
    path: &[Step],
) -> &'a Descend<crate::Editor> {
    frame
        .descends
        .iter()
        .find(|d| d.path.as_ref() == path)
        .expect("outline occurrence")
}

fn entry_paths(value: &Value, parent: &[Step], key: CellId) -> Vec<Path> {
    value
        .as_record()
        .unwrap()
        .get(&OUTLINE)
        .unwrap()
        .as_list()
        .unwrap()
        .iter()
        .filter(|(_, v)| v.as_cell() == Some(key))
        .map(|(p, _)| {
            parent
                .iter()
                .cloned()
                .chain([Step::Key(OUTLINE), Step::Element(p.clone())])
                .collect()
        })
        .collect()
}

fn entry(world: &crate::Editor, parent: &[Step], key: CellId) -> Path {
    entry_paths(world.sources().resolve_path(parent).unwrap(), parent, key).remove(0)
}

fn body(entry: &[Step], key: CellId) -> Path {
    entry.iter().cloned().chain([Step::Key(key)]).collect()
}

fn key(key: Key) -> KeyboardEvent {
    KeyboardEvent {
        key,
        state: KeyState::Down,
        ..Default::default()
    }
}

fn hidden(world: &crate::Editor, path: &[Step]) -> bool {
    crate::annotations::collapsed(&world.model.workspace.document.annotations, path, false)
}

fn click_heading(world: &mut crate::Editor, path: &[Step], modifiers: Modifiers) {
    let frame = frame(world);
    let rect = stop(&frame, path).rect;
    let point = Point::new(rect.x0 + 4.0, rect.y0 + 4.0);
    click_frame(world, frame, point, path, modifiers);
}

fn click_frame(
    world: &mut crate::Editor,
    frame: placed::HoverOutput<crate::Editor>,
    point: Point,
    path: &[Step],
    modifiers: Modifiers,
) {
    let target = Hovered::Tree(Hover::Value(Rc::from(path)));
    assert!(matches!(frame.hover_geometry.probe(Some(point), None, 0.0),
        Some((_, Claim::Direct(ref found))) if *found == target));
    let mut input = placed::DispatchContext::new(Some(crate::test_root()), Some(target));
    assert!(frame.resolve_for_dispatch().dispatch_pointer_down_with(
        world,
        &PointerButtonEvent {
            button: Some(PointerButton::Primary),
            pointer: PointerInfo {
                pointer_id: Some(PointerId::PRIMARY),
                persistent_device_id: None,
                pointer_type: PointerType::Mouse
            },
            state: PointerState {
                position: (point.x, point.y).into(),
                modifiers,
                ..Default::default()
            },
        },
        &mut input,
    ));
}

#[test]
fn cam_outline_leaves_panes_in_the_collapsed_extras() {
    let (doc, _) = crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let panes = crate::libraries::workspace::vocabulary::PANES;
    assert!(entry_paths(doc.root.as_ref().unwrap(), &[], panes).is_empty());
    assert!(!crate::workspace::declarations(doc.root.as_ref()).is_empty());
    let mut world = crate::test_editor(doc);
    let entries = world
        .sources()
        .resolve_path(&[Step::Key(OUTLINE)])
        .unwrap()
        .as_list()
        .unwrap()
        .clone();
    for (position, value) in entries {
        world.set_collapsed(
            &crate::test_root(),
            &[
                Step::Key(OUTLINE),
                Step::Element(position),
                Step::Key(value.as_cell().unwrap()),
            ],
            false,
            Some(true),
        );
    }
    let path = [Step::Key(panes)];
    assert!(hidden(&world, &path));
    let f = frame(&mut world);
    stop(&f, &path);
    assert!(
        !f.descends
            .iter()
            .any(|d| d.path.starts_with(&path) && d.path.len() > path.len())
    );
}

#[test]
fn outline_field_label_selects_the_list_and_reads_the_field_name() {
    let (mut doc, a, b, extra) = document();
    let Value::Record(fields) = doc.root.as_mut().unwrap() else {
        unreachable!()
    };
    fields.remove(&extra);
    let mut world = crate::test_editor(doc);
    let f = frame(&mut world);
    let root_rect = stop(&f, &[]).rect;
    let list_rect = stop(&f, &[Step::Key(OUTLINE)]).rect;
    assert_eq!(
        root_rect.x0, list_rect.x0,
        "the outline adds no outer indent"
    );
    let (_, before) = place(&world.model.doc, None, 500.0);
    assert!(
        root_rect.y0 < list_rect.y0,
        "the record includes the field label outside the list"
    );
    let label_point = Point::new(root_rect.x0 + 4.0, root_rect.y0 + 4.0);
    click_frame(
        &mut world,
        f,
        label_point,
        &[Step::Key(OUTLINE)],
        Modifiers::empty(),
    );
    assert_eq!(
        world.model.selection.as_ref().unwrap().path(),
        [Step::Key(OUTLINE)]
    );
    assert!(stop(&frame(&mut world), &[Step::Key(OUTLINE)]).rect.y0 > root_rect.y0);

    // Override the ordinary definition: the label must follow that name,
    // without changing the geometry or identity of the section list.
    world.model.selection = None;
    Rc::make_mut(&mut world.model.doc).cells.set_value(
        OUTLINE,
        name::record("A much longer document organization handle", []),
    );
    let renamed = frame(&mut world);
    let (_, after) = place(&world.model.doc, None, 500.0);
    assert!(
        after.width > before.width,
        "the field label measures the updated name"
    );
    assert_eq!(stop(&renamed, &[Step::Key(OUTLINE)]).rect, list_rect);
    click_frame(
        &mut world,
        renamed,
        label_point,
        &[Step::Key(OUTLINE)],
        Modifiers::empty(),
    );

    let f = frame(&mut world);
    let select_all = KeyboardEvent {
        modifiers: if cfg!(target_os = "macos") {
            Modifiers::META
        } else {
            Modifiers::CONTROL
        },
        ..key(Key::Character("a".into()))
    };
    let target = crate::navigate::step_selection(
        &f.descends,
        None, // This fixture places one projection without the pane wrapper.
        world.model.selection.as_ref(),
        18.0,
        &select_all,
    )
    .expect("select all reaches the root");
    assert!((target.select)(&mut world, None));
    assert!(world.model.selection.as_ref().unwrap().path().is_empty());

    // Removing the outline field exposes the remaining data structurally.
    assert!((stop(&f, &[Step::Key(OUTLINE)]).select)(&mut world, None));
    assert!(world.delete_selected_edge(crate::navigate::Geometry {
        descends: &f.descends,
        ..Default::default()
    }));
    assert!(
        world
            .sources()
            .resolve_path(&[Step::Key(OUTLINE)])
            .is_none()
    );
    let structural = frame(&mut world);
    for field in [a, b] {
        stop(&structural, &[Step::Key(field)]);
    }
}

#[test]
fn outline_library_tint_stops_at_the_heading() {
    let key = crate::libraries::workspace::vocabulary::PANES;
    let mut world = crate::test_editor(Document {
        root: Some(Value::record([
            (OUTLINE, Value::list([key.into()])),
            (key, Value::list([f64::value(1.0), f64::value(2.0)])),
        ])),
        cells: Cells::new(),
    });
    let heading_path = entry(&world, &[], key);
    let body_path = body(&heading_path, key);
    world.set_collapsed(&crate::test_root(), &body_path, false, Some(false));
    let f = frame(&mut world);
    let heading_rect = stop(&f, &heading_path).rect;
    let body_rect = stop(&f, &body_path).rect;
    assert!(heading_rect.y1 < body_rect.y0);
    assert!(
        stop(&f, &body_path)
            .scope
            .writable(&world.sources(), &body_path)
    );

    fn tints(commands: &[DrawCmd]) -> Vec<Rect> {
        commands
            .iter()
            .flat_map(|command| match command {
                DrawCmd::Clip { children, .. } => tints(children),
                DrawCmd::Fill {
                    shape: Shape::RoundedRect(rect),
                    brush: Brush::Solid(color),
                    ..
                } if *color == Color::new([0.13, 0.14, 0.16, 0.05]) => vec![rect.rect()],
                _ => vec![],
            })
            .collect()
    }
    let drawing = settle(f).list;
    let tints = tints(&drawing.0);
    assert_eq!(tints.len(), 1);
    assert!(tints[0].contains(heading_rect.center()));
    assert!(tints[0].y1 < body_rect.y0);
}

#[test]
fn outline_orders_list_occurrences_and_edits_their_source_fields() {
    let (doc, a, b, extra) = document();
    let mut world = crate::test_editor(doc);
    let (a_path, b_path) = (
        body(&entry(&world, &[], a), a),
        body(&entry(&world, &[], b), b),
    );
    let f = frame(&mut world);
    assert!(stop(&f, &b_path).rect.y0 < stop(&f, &a_path).rect.y0);
    assert!(stop(&f, &a_path).rect.y1 < stop(&f, &[Step::Key(extra)]).rect.y0);
    assert_eq!(
        stop(&f, &b_path).scope.source(&b_path).unwrap().as_ref(),
        [Step::Key(b)]
    );
    assert!((stop(&f, &b_path).select)(&mut world, None));
    assert!(
        frame(&mut world)
            .resolve_for_dispatch()
            .dispatch_key(&mut world, &key(Key::Character("5".into())))
    );
    assert_eq!(
        world.sources().resolve_path(&[Step::Key(b)]),
        Some(&f64::value(35.0))
    );
    let raw = editing_frame(&mut world, true);
    for field in [a, b, extra, OUTLINE] {
        stop(&raw, &[Step::Key(field)]);
    }
}

#[test]
fn repeated_sections_have_independent_folds_and_shared_edits() {
    let (mut doc, a, b, _) = document();
    let Value::Record(fields) = doc.root.as_mut().unwrap() else {
        unreachable!()
    };
    fields.insert(OUTLINE, Value::list([a.into(), a.into(), b.into()]));
    let mut world = crate::test_editor(doc);
    let entries = entry_paths(world.model.doc.root.as_ref().unwrap(), &[], a);
    let bodies = entries.iter().map(|p| body(p, a)).collect::<Vec<_>>();
    let before = world.model.doc.clone();
    click_heading(&mut world, &entries[0], Modifiers::empty());
    assert!(hidden(&world, &bodies[0]));
    assert!(!hidden(&world, &bodies[1]));
    let f = frame(&mut world);
    assert!(!f.descends.iter().any(|d| d.path.as_ref() == bodies[0]));
    stop(&f, &bodies[1]);
    assert!(Rc::ptr_eq(&before, &world.model.doc));
    assert!(!world.model.dirty());
    assert!(world.model.step_history(true));
    assert!(!hidden(&world, &bodies[0]));
    assert!(world.model.step_history(false));
    assert!(hidden(&world, &bodies[0]));
    click_heading(&mut world, &entries[0], Modifiers::empty());

    let f = frame(&mut world);
    assert!((stop(&f, &bodies[1]).select)(&mut world, None));
    let replacement = Value::list([f64::value(9.0)]);
    assert!(world.paste_value(replacement.clone()));
    let f = frame(&mut world);
    for path in &bodies {
        assert_eq!(
            stop(&f, path).scope.read(&world.sources(), path),
            Some(&replacement)
        );
    }
    assert!(world.model.step_history(true));
    assert_eq!(world.model.selection.as_ref().unwrap().path(), bodies[1]);
}

#[test]
fn computed_outline_keeps_independent_read_only_occurrences() {
    let (mut doc, a, _, _) = document();
    let value = doc.root.take().unwrap();
    let e = entry_paths(&value, &[Step::Key(RESULT)], a).remove(0);
    let source_entry = entry_paths(&value, &[Step::Key(grap::vocabulary::EVALUATE)], a).remove(0);
    let result_body = body(&e, a);
    let source_body = body(&source_entry, a);
    let expected = value.as_record().unwrap().get(&a).unwrap().clone();
    doc.root = Some(Value::record([(grap::vocabulary::EVALUATE, value)]));
    let mut world = crate::test_editor(doc);
    let before = world.model.doc.clone();
    let f = frame(&mut world);
    let positions: Vec<_> = expected.as_list().unwrap().keys().cloned().collect();
    let element = |position| {
        result_body
            .iter()
            .cloned()
            .chain([Step::Element(position)])
            .collect::<Path>()
    };
    let first = stop(&f, &element(positions[0].clone())).rect;
    let second = stop(&f, &element(positions[1].clone())).rect;
    let missing =
        element(gid::position::between(Some(&positions[0]), Some(&positions[1])).unwrap());
    assert!(
        !matches!(
            f.hover_geometry.probe(Some(Point::new(first.x0 + 4.0, (first.y1 + second.y0) / 2.0)), None, 0.0),
            Some((_, Claim::Direct(Hovered::Tree(Hover::Value(path))))) if path.as_ref() == missing
        ),
        "computed lists have no insertion target"
    );
    assert!((stop(&f, &result_body).select)(&mut world, None));
    assert!(
        world
            .model
            .selection
            .as_ref()
            .unwrap()
            .source_path()
            .is_none()
    );
    assert!(!world.paste_value(Value::record([])));
    let mut copy = key(Key::Character("c".into()));
    copy.modifiers = if cfg!(target_os = "macos") {
        Modifiers::META
    } else {
        Modifiers::CONTROL
    };
    assert!(
        frame(&mut world)
            .resolve_for_dispatch()
            .dispatch_key(&mut world, &copy)
    );
    assert_eq!(world.clipboard_structure(), Some(expected));
    click_heading(&mut world, &e, Modifiers::empty());
    let f = frame(&mut world);
    assert!(!f.descends.iter().any(|d| d.path.as_ref() == result_body));
    stop(&f, &source_body);
    assert!(Rc::ptr_eq(&before, &world.model.doc));
}

#[test]
fn outline_column_gaps_open_pending_before_between_and_after_items() {
    for index in 0_usize..=2 {
        let (doc, a, b, _) = document();
        let mut world = crate::test_editor(doc);
        let entries = [entry(&world, &[], b), entry(&world, &[], a)];
        for path in &entries {
            click_heading(&mut world, path, Modifiers::empty());
        }
        world.model.selection = None;
        let positions: Vec<_> = entries
            .iter()
            .map(|path| match path.last().unwrap() {
                Step::Element(position) => position.clone(),
                _ => unreachable!(),
            })
            .collect();
        let inserted = gid::position::between(
            index.checked_sub(1).map(|i| &positions[i]),
            positions.get(index),
        )
        .unwrap();
        let path = vec![Step::Key(OUTLINE), Step::Element(inserted.clone())];
        let f = frame(&mut world);
        let list = stop(&f, &[Step::Key(OUTLINE)]).rect;
        let top = index
            .checked_sub(1)
            .map_or(list.y0, |i| stop(&f, &entries[i]).rect.y1);
        let bottom = entries.get(index).map_or(list.y1, |p| stop(&f, p).rect.y0);
        assert!(bottom > top);
        let before = world.model.doc.clone();
        click_frame(
            &mut world,
            f,
            Point::new(list.x0 + 4.0, (top + bottom) / 2.0),
            &path,
            Modifiers::empty(),
        );
        assert!(
            Rc::ptr_eq(&before, &world.model.doc),
            "opening a pending doesn't edit the document"
        );
        assert_eq!(world.model.selection.as_ref().unwrap().path(), path);
        assert!(frame(&mut world).completion.is_some());
        assert!(world.commit_completion(a.into(), None, None));
        let list = world
            .sources()
            .resolve_path(&[Step::Key(OUTLINE)])
            .unwrap()
            .as_list()
            .unwrap()
            .clone();
        assert_eq!(list.len(), 3);
        assert_eq!(list.get(&inserted), Some(&Value::Cell(a)));
    }
}

#[test]
fn outline_body_column_gap_inserts_through_its_jump() {
    let (doc, a, _, _) = document();
    let mut world = crate::test_editor(doc);
    let section = body(&entry(&world, &[], a), a);
    let positions: Vec<_> = world
        .sources()
        .resolve_path(&[Step::Key(a)])
        .unwrap()
        .as_list()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    let inserted = gid::position::between(Some(&positions[0]), Some(&positions[1])).unwrap();
    let element = |position| {
        section
            .iter()
            .cloned()
            .chain([Step::Element(position)])
            .collect::<Path>()
    };
    let path = element(inserted.clone());
    let f = frame(&mut world);
    let first = stop(&f, &element(positions[0].clone())).rect;
    let second = stop(&f, &element(positions[1].clone())).rect;
    click_frame(
        &mut world,
        f,
        Point::new(first.x0 + 4.0, (first.y1 + second.y0) / 2.0),
        &path,
        Modifiers::empty(),
    );
    assert_eq!(
        world
            .model
            .selection
            .as_ref()
            .unwrap()
            .source_path()
            .as_deref(),
        Some([Step::Key(a), Step::Element(inserted.clone())].as_slice())
    );
    let f = frame(&mut world);
    assert!(f.completion.is_some());
    let pending = stop(&f, &path).rect;
    let first = stop(&f, &element(positions[0].clone())).rect;
    let second = stop(&f, &element(positions[1].clone())).rect;
    for (before, after, y) in [
        (&positions[0], &inserted, (first.y1 + pending.y0) / 2.0),
        (&inserted, &positions[1], (pending.y1 + second.y0) / 2.0),
    ] {
        let missing = element(gid::position::between(Some(before), Some(after)).unwrap());
        assert!(
            !matches!(
                f.hover_geometry.probe(Some(Point::new(first.x0 + 4.0, y)), None, 0.0),
                Some((_, Claim::Direct(Hovered::Tree(Hover::Value(path))))) if path.as_ref() == missing
            ),
            "both gaps beside a pending stay inactive"
        );
    }
    assert!(world.commit_completion(f64::value(7.0), None, None));
    assert_eq!(
        world
            .sources()
            .resolve_path(&[Step::Key(a), Step::Element(inserted)]),
        Some(&f64::value(7.0))
    );
}

#[test]
fn outline_preserves_pending_insertions_in_both_lists() {
    let (doc, a, _, _) = document();
    let mut world = crate::test_editor(doc);
    let e = entry(&world, &[], a);
    let section = body(&e, a);
    let pos = world
        .sources()
        .resolve_path(&[Step::Key(a)])
        .unwrap()
        .as_list()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    let item: Path = section
        .iter()
        .cloned()
        .chain([Step::Element(pos)])
        .collect();
    for target in [item, e] {
        let f = frame(&mut world);
        assert!((stop(&f, &target).select)(&mut world, None));
        assert!(world.insert_key(
            crate::navigate::Geometry {
                descends: &f.descends,
                ..Default::default()
            },
            &key(Key::Named(NamedKey::Enter))
        ));
        let path = world.model.selection.as_ref().unwrap().path().to_vec();
        let f = frame(&mut world);
        assert!(f.completion.is_some());
        stop(&f, &path);
        assert!(world.commit_completion(
            if path.len() == 2 {
                a.into()
            } else {
                f64::value(7.0)
            },
            None,
            None
        ));
    }
    assert_eq!(
        world
            .sources()
            .resolve_path(&[Step::Key(a)])
            .unwrap()
            .as_list()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        world
            .sources()
            .resolve_path(&[Step::Key(OUTLINE)])
            .unwrap()
            .as_list()
            .unwrap()
            .len(),
        3
    );
}

#[test]
fn outline_headings_delete_and_pick_references_not_bodies() {
    let (doc, a, _, extra) = document();
    let mut world = crate::test_editor(doc);
    let e = entry(&world, &[], a);
    world.model.selection = Some(pending_value(&crate::test_root(), vec![Step::Key(extra)]));
    click_heading(&mut world, &e, Modifiers::META | Modifiers::CONTROL);
    assert_eq!(
        world.sources().resolve_path(&[Step::Key(extra)]),
        Some(&Value::Cell(a))
    );
    assert!(!hidden(&world, &body(&e, a)));
    click_heading(&mut world, &e, Modifiers::empty());
    let before = world
        .sources()
        .resolve_path(&[Step::Key(a)])
        .unwrap()
        .clone();
    let f = frame(&mut world);
    assert!(world.delete_selected_edge(crate::navigate::Geometry {
        descends: &f.descends,
        ..Default::default()
    }));
    assert!(world.sources().resolve_path(&e).is_none());
    assert_eq!(world.sources().resolve_path(&[Step::Key(a)]), Some(&before));
    stop(&frame(&mut world), &[Step::Key(a)]);
}

#[test]
fn nested_outline_composes_jumps_and_cell_follows() {
    let (mut doc, a, b, _) = document();
    let (parent, alias) = (new_cell_id(), new_cell_id());
    doc.cells.set_value(alias, doc.root.take().unwrap());
    doc.root = Some(Value::record([
        (OUTLINE, Value::list([parent.into()])),
        (parent, alias.into()),
        (b, f64::value(99.0)),
    ]));
    let mut world = crate::test_editor(doc);
    let outer = body(&entry(&world, &[], parent), parent);
    let followed: Path = outer
        .into_iter()
        .chain([Step::Follow(gid::Resolution::Document)])
        .collect();
    let inner = world
        .sources()
        .resolve_path(&[Step::Key(parent), Step::Follow(gid::Resolution::Document)])
        .unwrap();
    let e = entry_paths(inner, &followed, a).remove(0);
    let p = body(&e, a);
    let f = frame(&mut world);
    assert!((stop(&f, &p).select)(&mut world, None));
    assert!(world.paste_value(Value::list([f64::value(12.0)])));
    assert_eq!(
        world.sources().resolve_path(&[
            Step::Key(parent),
            Step::Follow(gid::Resolution::Document),
            Step::Key(a)
        ]),
        Some(&Value::list([f64::value(12.0)]))
    );
    assert_eq!(
        world.sources().resolve_path(&[Step::Key(b)]),
        Some(&f64::value(99.0))
    );
}

#[test]
fn outline_missing_field_is_editable_and_empty_outline_is_reachable() {
    let a = new_cell_id();
    let mut world = crate::test_editor(Document {
        root: Some(Value::record([(OUTLINE, Value::list([a.into()]))])),
        cells: Cells::new(),
    });
    let p = body(&entry(&world, &[], a), a);
    let f = frame(&mut world);
    assert!((stop(&f, &p).select)(&mut world, None));
    assert!(frame(&mut world).completion.is_some());
    assert!(world.commit_completion(f64::value(4.0), None, None));
    assert_eq!(
        world.sources().resolve_path(&[Step::Key(a)]),
        Some(&f64::value(4.0))
    );

    world.model.selection = pending_edge(&crate::test_root(), &world.sources(), Vec::new());
    assert!(frame(&mut world).completion.is_some());
    let Value::Record(fields) = Rc::make_mut(&mut world.model.doc).root.as_mut().unwrap() else {
        unreachable!()
    };
    fields.insert(OUTLINE, Value::list([]));
    world.model.selection = None;
    let f = frame(&mut world);
    let empty = stop(&f, &[Step::Key(OUTLINE)]);
    assert!(empty.rect.width() > 0.0 && empty.rect.height() > 0.0);
}
