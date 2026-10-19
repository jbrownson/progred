use super::*;
use crate::display::test_support::Landmarks;
use crate::display::widget::navigation::Landmark;
use crate::driver::Driver;
use crate::frame::Hovered;
use crate::hover::Hover;
use crate::libraries::presentation::vocabulary::{OUTLINE, RESULT};
use crate::libraries::{f64, name};
use crate::location::Location;
use crate::projection::test_support::*;
use crate::selection::{pending_edge, pending_value};
use gid::{Cells, Document, Path, new_cell_id};
use kurbo::{Point, Rect};
use peniko::Brush;
use puri::draw::{DrawCmd, Shape};
use ui_events::keyboard::{Key, Modifiers, NamedKey};
use winit::keyboard::ModifiersState;

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

fn stop<'a>(
    frame: &'a crate::projection::test_support::HoverOutput<crate::Editor>,
    path: &[Step],
) -> &'a Landmark<crate::Editor> {
    frame.landmarks.landmark(path).expect("outline occurrence")
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

fn hidden(world: &crate::Editor, path: &[Step]) -> bool {
    crate::annotations::folded(world.model.workspace.document.annotations.at(path), false)
}

/// Click `path` at `point`, where the pointer lands on it.
fn click(driver: &mut Driver, point: Point, path: &[Step]) {
    assert_eq!(
        driver.hover_at(point),
        Some(Hovered::Tree(Hover::Value(Rc::from(path))))
    );
    driver.click_at(point);
}

/// Click the heading at `path` near its top-left corner.
fn click_heading(driver: &mut Driver, path: &[Step]) {
    let rect = driver.rect(path);
    click(driver, Point::new(rect.x0 + 4.0, rect.y0 + 4.0), path);
}

/// The landmark the driven frame draws at `path`.
fn drawn<'a>(driver: &'a Driver, path: &[Step]) -> &'a Landmark<crate::Editor> {
    driver
        .runner
        .frame
        .dispatch
        .landmarks
        .landmark(path)
        .expect("outline occurrence")
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
    let first_heading = vec![
        Step::Key(OUTLINE),
        Step::Element(entries.keys().next().unwrap().clone()),
    ];
    for (position, value) in entries {
        world.session().set_collapsed(
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
    let mut driver = Driver::new(world, Driver::VIEWPORT);
    for (from, key, to) in [
        (vec![], NamedKey::ArrowRight, vec![Step::Key(OUTLINE)]),
        (vec![], NamedKey::ArrowDown, first_heading.clone()),
        (
            vec![Step::Key(OUTLINE)],
            NamedKey::ArrowDown,
            first_heading.clone(),
        ),
        (first_heading, NamedKey::ArrowUp, vec![]),
        (vec![Step::Key(OUTLINE)], NamedKey::ArrowLeft, vec![]),
    ] {
        driver.select(&from);
        driver.press(key);
        assert_eq!(driver.selection(), to);
    }
    // The document draws the collapsed panes field but nothing inside it;
    // the panes it declares draw their own views.
    assert!(driver.drawn(&path));
    assert!(
        !driver
            .runner
            .frame
            .dispatch
            .landmarks
            .iter()
            .any(|landmark| {
                landmark.root.as_ref() == Some(&crate::test_root())
                    && landmark.path.starts_with(&path)
                    && landmark.path.len() > path.len()
            })
    );
}

#[test]
fn outline_arrows_enter_sections_visit_visible_bodies_and_reach_extras_last() {
    use crate::driver::Driver;
    for with_extras in [false, true] {
        let (mut doc, a, b, extra) = document();
        if !with_extras {
            let Value::Record(fields) = doc.root.as_mut().unwrap() else {
                unreachable!()
            };
            fields.remove(&extra);
        }
        let editor = crate::test_editor(doc.clone());
        let a_heading = entry(&editor, &[], a);
        let b_heading = entry(&editor, &[], b);
        let a_body = body(&a_heading, a);
        let b_body = body(&b_heading, b);
        let items = editor
            .sources()
            .resolve_path(&[Step::Key(a)])
            .unwrap()
            .as_list()
            .unwrap()
            .keys()
            .map(|position| {
                a_body
                    .iter()
                    .cloned()
                    .chain([Step::Element(position.clone())])
                    .collect::<Path>()
            })
            .collect::<Vec<_>>();
        // The document with B's body folded or not, and A's open.
        let driver = |b_folded| {
            let mut editor = crate::test_editor(doc.clone());
            let root = editor.model.workspace.document_root().clone();
            for (path, folded) in [(&a_body, false), (&b_body, b_folded)] {
                editor
                    .session()
                    .set_collapsed(&root, path, false, Some(folded));
            }
            Driver::new(editor, kurbo::Size::new(700.0, 600.0))
        };
        let arrow = |driver: &mut Driver, key, path: &Path| {
            assert!(driver.keystroke(Key::Named(key), Modifiers::empty()));
            assert_eq!(driver.selection(), *path);
        };
        let mut open = driver(false);
        // Right retains the whole-list stop. Down skips that enclosing stop;
        // Up from the first heading returns to the outermost whole value.
        for (direction, path) in [
            (NamedKey::ArrowDown, vec![]),
            (NamedKey::ArrowRight, vec![Step::Key(OUTLINE)]),
            (NamedKey::ArrowLeft, vec![]),
            (NamedKey::ArrowDown, b_heading.clone()),
            (NamedKey::ArrowUp, vec![]),
        ] {
            arrow(&mut open, direction, &path);
        }
        open.press(NamedKey::Escape);
        let mut expected = vec![
            vec![],
            b_heading.clone(),
            b_body.clone(),
            a_heading.clone(),
            a_body.clone(),
        ];
        expected.extend(items);
        if with_extras {
            expected.push(vec![Step::Key(extra)]);
        }
        for path in &expected {
            arrow(&mut open, NamedKey::ArrowDown, path);
        }
        // Including the extras boundary: backward travel re-enters the last
        // child, rather than skipping the outline's contents to its whole stop.
        for path in expected.iter().rev().skip(1) {
            arrow(&mut open, NamedKey::ArrowUp, path);
        }
        // Collapsed bodies contribute no stops; moving onto a heading doesn't
        // run its pointer activation handler or unfold it.
        let mut folded = driver(true);
        for path in [vec![], b_heading, a_heading] {
            arrow(&mut folded, NamedKey::ArrowDown, &path);
        }
        assert!(hidden(folded.editor(), &b_body));
    }
}

#[test]
fn outline_field_label_selects_the_list_and_reads_the_field_name() {
    let (mut doc, a, b, extra) = document();
    let Value::Record(fields) = doc.root.as_mut().unwrap() else {
        unreachable!()
    };
    fields.remove(&extra);
    let mut driver = Driver::new(crate::test_editor(doc.clone()), Driver::VIEWPORT);
    let root_rect = driver.rect(&[]);
    let list_rect = driver.rect(&[Step::Key(OUTLINE)]);
    assert_eq!(
        root_rect.x0, list_rect.x0,
        "the outline adds no outer indent"
    );
    let (_, before) = place(&doc, None, 500.0);
    assert!(
        root_rect.y0 < list_rect.y0,
        "the record includes the field label outside the list"
    );
    let label_point = Point::new(root_rect.x0 + 4.0, root_rect.y0 + 4.0);
    click(&mut driver, label_point, &[Step::Key(OUTLINE)]);
    assert_eq!(driver.selection(), [Step::Key(OUTLINE)]);
    assert!(driver.rect(&[Step::Key(OUTLINE)]).y0 > root_rect.y0);

    // Override the ordinary definition: the label must follow that name,
    // without changing the geometry or identity of the section list.
    doc.cells.set_value(
        OUTLINE,
        name::record("A much longer document organization handle", []),
    );
    let (_, after) = place(&doc, None, 500.0);
    assert!(
        after.width > before.width,
        "the field label measures the updated name"
    );
    let mut renamed = Driver::new(crate::test_editor(doc), Driver::VIEWPORT);
    assert_eq!(renamed.rect(&[Step::Key(OUTLINE)]), list_rect);
    click(&mut renamed, label_point, &[Step::Key(OUTLINE)]);
    renamed.select(&[]);

    // Removing the outline field exposes the remaining data structurally.
    renamed.select(&[Step::Key(OUTLINE)]);
    renamed.press(NamedKey::Delete);
    assert!(
        renamed
            .editor()
            .sources()
            .resolve_path(&[Step::Key(OUTLINE)])
            .is_none()
    );
    for field in [a, b] {
        assert!(renamed.drawn(&[Step::Key(field)]));
    }
}

#[test]
fn outline_library_tint_stops_at_the_heading() {
    let key = crate::libraries::workspace::vocabulary::PANES;
    let mut world = crate::editor_of(Value::record([
        (OUTLINE, Value::list([key.into()])),
        (key, Value::list([f64::value(1.0), f64::value(2.0)])),
    ]));
    let heading_path = entry(&world, &[], key);
    let body_path = body(&heading_path, key);
    world
        .session()
        .set_collapsed(&crate::test_root(), &body_path, false, Some(false));
    let f = editing_frame(&mut world);
    let heading_rect = stop(&f, &heading_path).rect;
    let body_rect = stop(&f, &body_path).rect;
    assert!(heading_rect.y1 < body_rect.y0);
    assert!(
        stop(&f, &body_path)
            .scope
            .view(world.sources())
            .writable(&body_path)
    );

    fn tints(commands: &[DrawCmd]) -> Vec<Rect> {
        crate::display::test_support::painted(commands)
            .filter_map(|command| match command {
                DrawCmd::Fill {
                    shape: Shape::RoundedRect(rect),
                    brush: Brush::Solid(color),
                    ..
                } if *color
                    == crate::display::widget::style::Theme::Light
                        .palette()
                        .readonly_ground =>
                {
                    Some(rect.rect())
                }
                _ => None,
            })
            .collect()
    }
    let drawing = settle(f, &world.sources()).list;
    let tints = tints(&drawing.0);
    assert_eq!(tints.len(), 1);
    assert!(tints[0].contains(heading_rect.center()));
    assert!(tints[0].y1 < body_rect.y0);
}

#[test]
fn outline_orders_list_occurrences_and_edits_their_source_fields() {
    let (doc, a, b, extra) = document();
    let mut driver = Driver::new(crate::test_editor(doc), Driver::VIEWPORT);
    let (a_path, b_path) = (
        body(&entry(driver.editor(), &[], a), a),
        body(&entry(driver.editor(), &[], b), b),
    );
    assert!(driver.rect(&b_path).y0 < driver.rect(&a_path).y0);
    assert!(driver.rect(&a_path).y1 < driver.rect(&[Step::Key(extra)]).y0);
    assert_eq!(
        drawn(&driver, &b_path).scope.location(&b_path),
        Some(Location::root(vec![Step::Key(b)]))
    );
    driver.select(&b_path);
    driver.type_text("5");
    assert_eq!(driver.value(&[Step::Key(b)]), f64::value(35.0));
    let raw = framed(
        &mut driver.runner.editor,
        Framing {
            raw: true,
            ..Framing::default()
        },
    );
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
    let entries = entry_paths(doc.root.as_ref().unwrap(), &[], a);
    let bodies = entries.iter().map(|p| body(p, a)).collect::<Vec<_>>();
    let mut driver = Driver::new(crate::test_editor(doc), Driver::VIEWPORT);
    let before = driver.editor().model.doc.clone();
    click_heading(&mut driver, &entries[0]);
    assert!(hidden(driver.editor(), &bodies[0]));
    assert!(!hidden(driver.editor(), &bodies[1]));
    assert!(!driver.drawn(&bodies[0]));
    assert!(driver.drawn(&bodies[1]));
    assert!(Rc::ptr_eq(&before, &driver.editor().model.doc));
    assert!(!driver.editor().model.dirty());
    assert!(driver.undo());
    assert!(!hidden(driver.editor(), &bodies[0]));
    assert!(driver.redo());
    assert!(hidden(driver.editor(), &bodies[0]));
    click_heading(&mut driver, &entries[0]);

    driver.select(&bodies[1]);
    let replacement = Value::list([f64::value(9.0)]);
    assert!(driver.paste(&replacement));
    for path in &bodies {
        assert_eq!(
            drawn(&driver, path)
                .scope
                .view(driver.editor().sources())
                .resolve_path(path),
            Some(&replacement)
        );
    }
    assert!(driver.undo());
    assert_eq!(driver.selection(), bodies[1]);
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
    let mut driver = Driver::new(crate::test_editor(doc), Driver::VIEWPORT);
    let before = driver.editor().model.doc.clone();
    let positions: Vec<_> = expected.as_list().unwrap().keys().cloned().collect();
    let element = |position| {
        result_body
            .iter()
            .cloned()
            .chain([Step::Element(position)])
            .collect::<Path>()
    };
    let first = driver.rect(&element(positions[0].clone()));
    let second = driver.rect(&element(positions[1].clone()));
    let missing =
        element(gid::position::between(Some(&positions[0]), Some(&positions[1])).unwrap());
    assert_ne!(
        driver.hover_at(Point::new(first.x0 + 4.0, (first.y1 + second.y0) / 2.0)),
        Some(Hovered::Tree(Hover::Value(Rc::from(missing)))),
        "computed lists have no insertion target"
    );
    driver.select(&result_body);
    assert!(
        driver
            .editor()
            .model
            .selection
            .as_ref()
            .unwrap()
            .location()
            .is_none()
    );
    assert!(!driver.paste(&Value::record([])));
    assert!(driver.keystroke(Key::Character("c".into()), crate::modifiers::command()));
    assert_eq!(driver.copied(), Some(expected));
    click_heading(&mut driver, &e);
    assert!(!driver.drawn(&result_body));
    assert!(driver.drawn(&source_body));
    assert!(Rc::ptr_eq(&before, &driver.editor().model.doc));
}

#[test]
fn outline_column_gaps_open_pending_before_between_and_after_items() {
    for index in 0_usize..=2 {
        let (doc, a, b, _) = document();
        let mut driver = Driver::new(crate::test_editor(doc), Driver::VIEWPORT);
        let entries = [
            entry(driver.editor(), &[], b),
            entry(driver.editor(), &[], a),
        ];
        for path in &entries {
            click_heading(&mut driver, path);
        }
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
        let list = driver.rect(&[Step::Key(OUTLINE)]);
        let top = index
            .checked_sub(1)
            .map_or(list.y0, |i| driver.rect(&entries[i]).y1);
        let bottom = entries.get(index).map_or(list.y1, |p| driver.rect(p).y0);
        assert!(bottom > top);
        let before = driver.editor().model.doc.clone();
        click(
            &mut driver,
            Point::new(list.x0 + 4.0, (top + bottom) / 2.0),
            &path,
        );
        assert!(
            Rc::ptr_eq(&before, &driver.editor().model.doc),
            "opening a pending doesn't edit the document"
        );
        assert_eq!(driver.selection(), path);
        assert!(driver.picker_entries().is_some());
        assert!(driver.paste(&a.into()));
        let list = driver.value(&[Step::Key(OUTLINE)]);
        let list = list.as_list().unwrap();
        assert_eq!(list.len(), 3);
        assert_eq!(list.get(&inserted), Some(&Value::Cell(a)));
    }
}

#[test]
fn outline_body_column_gap_inserts_through_its_jump() {
    let (doc, a, _, _) = document();
    let mut driver = Driver::new(crate::test_editor(doc), Driver::VIEWPORT);
    let section = body(&entry(driver.editor(), &[], a), a);
    let positions = driver.positions(&[Step::Key(a)]);
    let inserted = gid::position::between(Some(&positions[0]), Some(&positions[1])).unwrap();
    let element = |position| {
        section
            .iter()
            .cloned()
            .chain([Step::Element(position)])
            .collect::<Path>()
    };
    let path = element(inserted.clone());
    let first = driver.rect(&element(positions[0].clone()));
    let second = driver.rect(&element(positions[1].clone()));
    click(
        &mut driver,
        Point::new(first.x0 + 4.0, (first.y1 + second.y0) / 2.0),
        &path,
    );
    assert_eq!(
        driver.editor().model.selection.as_ref().unwrap().location(),
        Some(Location::root(vec![
            Step::Key(a),
            Step::Element(inserted.clone())
        ]))
    );
    assert!(driver.picker_entries().is_some());
    let pending = driver.rect(&path);
    let first = driver.rect(&element(positions[0].clone()));
    let second = driver.rect(&element(positions[1].clone()));
    for (before, after, y) in [
        (&positions[0], &inserted, (first.y1 + pending.y0) / 2.0),
        (&inserted, &positions[1], (pending.y1 + second.y0) / 2.0),
    ] {
        let missing = element(gid::position::between(Some(before), Some(after)).unwrap());
        assert_ne!(
            driver.hover_at(Point::new(first.x0 + 4.0, y)),
            Some(Hovered::Tree(Hover::Value(Rc::from(missing)))),
            "both gaps beside a pending stay inactive"
        );
    }
    assert!(driver.paste(&f64::value(7.0)));
    assert_eq!(
        driver.value(&[Step::Key(a), Step::Element(inserted)]),
        f64::value(7.0)
    );
}

#[test]
fn outline_preserves_pending_insertions_in_both_lists() {
    let (doc, a, _, _) = document();
    let mut driver = Driver::new(crate::test_editor(doc), Driver::VIEWPORT);
    let e = entry(driver.editor(), &[], a);
    let item = [
        body(&e, a),
        vec![Step::Element(driver.positions(&[Step::Key(a)])[0].clone())],
    ]
    .concat();
    for target in [item, e] {
        driver.select(&target);
        driver.press(NamedKey::Enter);
        let path = driver.selection();
        assert!(driver.picker_entries().is_some());
        assert!(driver.drawn(&path));
        assert!(driver.paste(&if path.len() == 2 {
            a.into()
        } else {
            f64::value(7.0)
        }));
    }
    for list in [a, OUTLINE] {
        assert_eq!(driver.positions(&[Step::Key(list)]).len(), 3);
    }
}

#[test]
fn outline_headings_delete_and_pick_references_not_bodies() {
    let (doc, a, _, extra) = document();
    let mut editor = crate::test_editor(doc);
    let e = entry(&editor, &[], a);
    editor.model.selection = Some(pending_value(
        &crate::test_root(),
        Default::default(),
        vec![Step::Key(extra)],
    ));
    let mut driver = Driver::new(editor, Driver::VIEWPORT);
    // The command key, on either platform, picks the heading's reference.
    driver.hold(ModifiersState::CONTROL | ModifiersState::SUPER);
    click_heading(&mut driver, &e);
    driver.hold(ModifiersState::empty());
    assert_eq!(driver.value(&[Step::Key(extra)]), Value::Cell(a));
    assert!(!hidden(driver.editor(), &body(&e, a)));
    click_heading(&mut driver, &e);
    let before = driver.value(&[Step::Key(a)]);
    driver.press(NamedKey::Delete);
    assert!(driver.editor().sources().resolve_path(&e).is_none());
    assert_eq!(driver.value(&[Step::Key(a)]), before);
    assert!(driver.drawn(&[Step::Key(a)]));
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
    let mut driver = Driver::new(crate::test_editor(doc), Driver::VIEWPORT);
    let outer = body(&entry(driver.editor(), &[], parent), parent);
    let followed: Path = outer
        .into_iter()
        .chain([Step::Follow(gid::Resolution::Document)])
        .collect();
    let inner = driver.value(&[Step::Key(parent), Step::Follow(gid::Resolution::Document)]);
    let e = entry_paths(&inner, &followed, a).remove(0);
    driver.select(&body(&e, a));
    assert!(driver.paste(&Value::list([f64::value(12.0)])));
    assert_eq!(
        driver.value(&[
            Step::Key(parent),
            Step::Follow(gid::Resolution::Document),
            Step::Key(a)
        ]),
        Value::list([f64::value(12.0)])
    );
    assert_eq!(driver.value(&[Step::Key(b)]), f64::value(99.0));
}

#[test]
fn outline_missing_field_is_editable_and_empty_outline_is_reachable() {
    let a = new_cell_id();
    let mut world = crate::editor_of(Value::record([(OUTLINE, Value::list([a.into()]))]));
    let p = body(&entry(&world, &[], a), a);
    let f = editing_frame(&mut world);
    assert!((stop(&f, &p).select)(&mut world, None));
    assert!(editing_frame(&mut world).completion.is_some());
    assert!(crate::completion::commit(
        &mut world.session(),
        f64::value(4.0),
        None,
        None
    ));
    assert_eq!(
        world.sources().resolve_path(&[Step::Key(a)]),
        Some(&f64::value(4.0))
    );

    world.model.selection = pending_edge(
        &crate::test_root(),
        &crate::editing::Scope::default().view(world.sources()),
        Vec::new(),
    );
    assert!(editing_frame(&mut world).completion.is_some());
    let Value::Record(fields) = Rc::make_mut(&mut world.model.doc).root.as_mut().unwrap() else {
        unreachable!()
    };
    fields.insert(OUTLINE, Value::list([]));
    world.model.selection = None;
    let f = editing_frame(&mut world);
    let empty = stop(&f, &[Step::Key(OUTLINE)]);
    assert!(empty.rect.width() > 0.0 && empty.rect.height() > 0.0);
}
