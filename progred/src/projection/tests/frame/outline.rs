use super::*;
use crate::libraries::presentation::vocabulary::OUTLINE;

fn document() -> (Document, CellId, CellId, CellId) {
    let a = new_cell_id();
    let b = new_cell_id();
    let extra = new_cell_id();
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

#[test]
fn outline_keeps_order_real_locations_and_unlisted_fields() {
    let (doc, a, b, extra) = document();
    let mut world = crate::test_editor(doc);
    let frame = editing_frame(&mut world, false);
    let at = |key| {
        frame
            .descends
            .iter()
            .find(|d| d.path.as_ref() == [Step::Key(key)])
            .unwrap()
    };
    assert!(at(b).rect.y0 < at(a).rect.y0);
    assert!(at(a).rect.y1 < at(extra).rect.y0);
    assert!(at(OUTLINE).rect.y1 < at(b).rect.y0);
    // Editing a section edits the original field, not a projected copy.
    assert!((at(b).select)(&mut world, None));
    assert!(
        editing_frame(&mut world, false)
            .resolve_for_dispatch()
            .dispatch_key(
                &mut world,
                &KeyboardEvent {
                    key: Key::Character("5".into()),
                    state: KeyState::Down,
                    ..Default::default()
                },
            )
    );
    assert_eq!(
        world
            .model
            .doc
            .root
            .as_ref()
            .unwrap()
            .as_record()
            .unwrap()
            .get(&b),
        Some(&f64::value(35.0))
    );
    // Raw remains the ordinary record, not an outline.
    let raw = editing_frame(&mut world, true);
    for key in [a, b, extra, OUTLINE] {
        assert!(
            raw.descends
                .iter()
                .any(|d| d.path.as_ref() == [Step::Key(key)])
        );
    }
}

#[test]
fn computed_outline_toggles_only_its_own_occurrence() {
    use crate::libraries::presentation::vocabulary::RESULT;
    let (mut doc, a, _, _) = document();
    let value = doc.root.take().unwrap();
    let position = value
        .as_record()
        .unwrap()
        .get(&OUTLINE)
        .unwrap()
        .as_list()
        .unwrap()
        .iter()
        .find_map(|(position, value)| (value.as_cell() == Some(a)).then(|| position.clone()))
        .unwrap();
    doc.root = Some(Value::record([(grap::vocabulary::EVALUATE, value)]));
    let mut world = crate::test_editor(doc);
    let before = world.model.doc.clone();
    let result_section = [Step::Key(RESULT), Step::Key(a)];
    let source_section = [Step::Key(grap::vocabulary::EVALUATE), Step::Key(a)];
    let tab = [
        Step::Key(RESULT),
        Step::Key(OUTLINE),
        Step::Element(position),
    ];
    let frame = outline_frame(&mut world);
    assert!(
        frame
            .descends
            .iter()
            .any(|d| d.path.as_ref() == result_section)
    );
    click_tab(&mut world, &tab, Modifiers::empty());
    let frame = outline_frame(&mut world);
    assert!(
        !frame
            .descends
            .iter()
            .any(|d| d.path.as_ref() == result_section)
    );
    assert!(
        frame
            .descends
            .iter()
            .any(|d| d.path.as_ref() == source_section)
    );
    assert_eq!(world.model.selection.as_ref().unwrap().path(), tab);
    assert!(Rc::ptr_eq(&before, &world.model.doc));
    click_tab(&mut world, &tab, Modifiers::empty());
    let frame = outline_frame(&mut world);
    assert!(
        frame
            .descends
            .iter()
            .any(|d| d.path.as_ref() == result_section)
    );
}

#[test]
fn outline_keeps_record_and_vertical_list_insertion_available() {
    let (doc, a, _, _) = document();
    let mut world = crate::test_editor(doc);
    world.model.selection = pending_edge(&crate::test_root(), &world.sources(), Vec::new());
    assert!(editing_frame(&mut world, false).completion.is_some());
    let position = world
        .model
        .doc
        .root
        .as_ref()
        .unwrap()
        .as_record()
        .unwrap()
        .get(&a)
        .unwrap()
        .as_list()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    crate::editing::insert(
        &mut world,
        &crate::test_root(),
        &[Step::Key(a), Step::Element(position)],
    );
    let pending = world.model.selection.as_ref().unwrap().path().to_vec();
    let frame = editing_frame(&mut world, false);
    assert!(frame.completion.is_some());
    assert!(frame.descends.iter().any(|d| d.path.as_ref() == pending));

    let tab = tab_path(&world, &[], a);
    crate::editing::insert(&mut world, &crate::test_root(), &tab);
    let pending = world.model.selection.as_ref().unwrap().path().to_vec();
    assert_eq!(pending[0], Step::Key(OUTLINE));
    let frame = outline_frame(&mut world);
    assert!(frame.completion.is_some());
    assert!(frame.descends.iter().any(|d| d.path.as_ref() == pending));
}

fn outline_frame(world: &mut crate::Editor) -> placed::HoverOutput<crate::Editor> {
    let annotations = world.model.workspace.document.annotations.clone();
    editing_frame_with_annotations(world, false, None, None, &annotations)
}

fn tab_path(world: &crate::Editor, parent: &[Step], key: CellId) -> Path {
    let mut path = parent.to_vec();
    path.push(Step::Key(OUTLINE));
    let position = world
        .sources()
        .resolve_path(&path)
        .unwrap()
        .as_list()
        .unwrap()
        .iter()
        .find_map(|(position, value)| (value.as_cell() == Some(key)).then(|| position.clone()))
        .unwrap();
    path.push(Step::Element(position));
    path
}

fn click_tab(world: &mut crate::Editor, path: &[Step], modifiers: Modifiers) {
    let frame = outline_frame(world);
    let point = frame
        .descends
        .iter()
        .find(|d| d.path.as_ref() == path)
        .expect("the tab is the actual outline element")
        .rect
        .center();
    let target = Hovered::Tree(Hover::Value(Rc::from(path)));
    assert!(matches!(
        frame.hover_geometry.probe(Some(point), None, 0.0),
        Some((_, Claim::Direct(ref found))) if *found == target
    ));
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

fn hidden(world: &crate::Editor, path: &[Step]) -> bool {
    crate::annotations::collapsed(&world.model.workspace.document.annotations, path, false)
}

#[test]
fn outline_tabs_select_real_elements_and_toggle_undoable_view_state() {
    let (doc, a, b, _) = document();
    // Both a list and a scalar section can be hidden.
    for key in [a, b] {
        let mut world = crate::test_editor(doc.clone());
        let tab = tab_path(&world, &[], key);
        click_tab(&mut world, &tab, Modifiers::empty());
        assert_eq!(world.model.selection.as_ref().unwrap().path(), tab);
        assert!(hidden(&world, &[Step::Key(key)]));
        assert!(!world.model.dirty());
        let frame = outline_frame(&mut world);
        assert!(frame.descends.iter().any(|d| d.path.as_ref() == tab));
        assert!(
            !frame
                .descends
                .iter()
                .any(|d| d.path.first() == Some(&Step::Key(key)))
        );
        assert!(world.model.step_history(true));
        assert!(!hidden(&world, &[Step::Key(key)]));
        assert!(world.model.step_history(false));
        assert!(hidden(&world, &[Step::Key(key)]));
        click_tab(&mut world, &tab, Modifiers::empty());
        assert!(!hidden(&world, &[Step::Key(key)]));
        assert!(
            outline_frame(&mut world)
                .descends
                .iter()
                .any(|d| d.path.as_ref() == [Step::Key(key)])
        );
        assert!(!world.model.dirty());
    }
}

#[test]
fn outline_tabs_keep_deletion_and_picking_as_list_operations() {
    let (doc, a, _, extra) = document();
    let mut world = crate::test_editor(doc);
    let tab = tab_path(&world, &[], a);
    // Picking a tab supplies the field identity, without toggling its body.
    world.model.selection = Some(pending_value(&crate::test_root(), vec![Step::Key(extra)]));
    click_tab(&mut world, &tab, Modifiers::META | Modifiers::CONTROL);
    assert_eq!(
        world.sources().resolve_path(&[Step::Key(extra)]),
        Some(&Value::Cell(a))
    );
    assert!(!hidden(&world, &[Step::Key(a)]));

    click_tab(&mut world, &tab, Modifiers::empty());
    assert_eq!(world.model.selection.as_ref().unwrap().path(), tab);
    let before = world
        .sources()
        .resolve_path(&[Step::Key(a)])
        .unwrap()
        .clone();
    let frame = outline_frame(&mut world);
    assert!(world.delete_selected_edge(crate::navigate::Geometry {
        descends: &frame.descends,
        ..Default::default()
    }));
    assert!(world.sources().resolve_path(&tab).is_none());
    assert_eq!(world.sources().resolve_path(&[Step::Key(a)]), Some(&before));
    // Removing a tab leaves the field in the ordinary extras record.
    assert!(
        outline_frame(&mut world)
            .descends
            .iter()
            .any(|d| d.path.as_ref() == [Step::Key(a)])
    );
}

#[test]
fn outline_tabs_are_local_to_their_record_and_keep_selected_content_accessible() {
    let (mut doc, a, b, _) = document();
    let parent = new_cell_id();
    doc.root = Some(Value::record([
        (parent, doc.root.take().unwrap()),
        (b, f64::value(9.0)),
    ]));
    let mut world = crate::test_editor(doc);
    let parent = [Step::Key(parent)];
    let tab = tab_path(&world, &parent, a);
    let section: Vec<_> = parent.into_iter().chain([Step::Key(a)]).collect();
    click_tab(&mut world, &tab, Modifiers::empty());
    assert!(hidden(&world, &section));
    assert!(!hidden(&world, &[Step::Key(a)]));
    assert!(
        !outline_frame(&mut world)
            .descends
            .iter()
            .any(|d| d.path.as_ref() == section)
    );

    // An external selection of hidden content must not leave an invisible editor.
    world.model.selection = Some(Selection::edge(&crate::test_root(), section.clone()));
    assert!(
        outline_frame(&mut world)
            .descends
            .iter()
            .any(|d| d.path.as_ref() == section)
    );
    click_tab(&mut world, &tab, Modifiers::empty());
    assert_eq!(world.model.selection.as_ref().unwrap().path(), tab);
    assert!(
        !outline_frame(&mut world)
            .descends
            .iter()
            .any(|d| d.path.as_ref() == section)
    );
}

#[test]
fn outline_without_extras_still_accepts_new_fields_and_missing_sections() {
    let a = new_cell_id();
    let mut world = crate::test_editor(Document {
        root: Some(Value::record([(OUTLINE, Value::list([a.into()]))])),
        cells: Cells::new(),
    });
    let frame = outline_frame(&mut world);
    assert!(
        frame
            .descends
            .iter()
            .any(|d| d.path.as_ref() == [Step::Key(a)])
    );
    let tab = tab_path(&world, &[], a);
    click_tab(&mut world, &tab, Modifiers::empty());
    assert!(
        !outline_frame(&mut world)
            .descends
            .iter()
            .any(|d| d.path.as_ref() == [Step::Key(a)])
    );
    world.model.selection = pending_edge(&crate::test_root(), &world.sources(), Vec::new());
    assert!(outline_frame(&mut world).completion.is_some());
}
