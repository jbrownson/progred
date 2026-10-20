use super::*;
use crate::sample::{PLANET_VIEW, planets};
use gid::Cells;
use std::rc::Rc;

impl Workspace {
    fn side(&self, root: &Root) -> Option<Side> {
        self.pane(root).map(|(side, ..)| side)
    }
}

fn add_pane(workspace: &mut Workspace, side: Side) -> Root {
    let column = workspace.column_mut(side);
    let position = gid::position::between(
        column
            .panes
            .last()
            .and_then(|pane| match pane.view.root.target() {
                Target::Pane { path } => match path.last() {
                    Some(Step::Element(position)) => Some(position),
                    _ => None,
                },
                _ => None,
            }),
        None,
    )
    .unwrap();
    let root = Root::pane(pane_path(side, position));
    column.panes.push(Pane {
        view: View::new(root.clone()),
        height: 1.0,
    });
    for pane in &mut column.panes {
        pane.height = 1.0;
    }
    normalize(&mut column.panes);
    root
}

#[test]
fn a_library_toggles_whichever_way_the_area_lists_its_libraries() {
    let [one, other] = [(); 2].map(|_| gid::new_cell_id());
    let mut workspace = Workspace::default();
    workspace.toggle_library(None, one);
    assert!(!workspace.document.shown.shows(one));
    assert!(workspace.document.shown.shows(other));
    workspace.show(None, Shown::RAW);
    workspace.toggle_library(None, one);
    assert!(workspace.document.shown.shows(one));
    assert!(!workspace.document.shown.shows(other));
    workspace.toggle_library(None, one);
    assert_eq!(workspace.document.shown, Shown::RAW);
}

#[test]
fn roots_are_view_identity_not_cell_identity() {
    let left = Root::pane(Vec::new());
    let other = Root::pane(Vec::new());
    assert_ne!(left, other);
    assert_eq!(left, left.clone());
}

#[test]
fn duplicate_views_keep_independent_projection_state() {
    let mut workspace = Workspace::default();
    let one = add_pane(&mut workspace, Side::Left);
    let other = add_pane(&mut workspace, Side::Right);

    crate::annotations::set_folded(
        &mut workspace.view_mut(&one).unwrap().annotations,
        &[],
        false,
        true,
    );

    assert!(crate::annotations::folded(
        workspace.view(&one).unwrap().annotations.at(&[]),
        false
    ));
    assert!(!crate::annotations::folded(
        workspace.view(&other).unwrap().annotations.at(&[]),
        false
    ));
    assert!(!crate::annotations::folded(
        workspace.document.annotations.at(&[]),
        false
    ));
}

#[test]
fn panes_are_values_and_their_edits_are_document_edits() {
    let cell = CellId::from_u128(1);
    let values = [
        Value::from(cell),
        Value::from(b"blob".to_vec()),
        Value::list([]),
        Value::record([]),
    ];
    let root = values.iter().fold(Value::record([]), |root, value| {
        append(&root, Side::Left, value.clone()).unwrap().0
    });
    let locations = declarations(Some(&root));
    assert_eq!(locations.len(), values.len());
    for (location, value) in locations.iter().zip(&values) {
        assert_eq!(
            crate::test_support::value_at(&root, &location.path),
            Some(value)
        );
        assert!(matches!(location.path.last(), Some(Step::Element(_))));
    }
    let (moved, path) = move_value(&root, &locations[1].path, Move::Up).unwrap();
    assert_eq!(declarations(Some(&moved))[0].path, path);
    assert_eq!(
        crate::test_support::value_at(&moved, &path),
        Some(&values[1])
    );
    assert_eq!(
        crate::test_support::value_at(&moved, &locations[0].path),
        Some(&values[0])
    );
    let (moved, path) = move_value(&moved, &path, Move::Right).unwrap();
    assert_eq!(declarations(Some(&moved)).last().unwrap().side, Side::Right);
    let mut doc = gid::Document {
        root: Some(moved),
        cells: Cells::new(),
    };
    doc.cells
        .set_value(cell, Value::from(b"definition".to_vec()));
    let mut doc = Rc::new(doc);
    let before = doc.clone();
    assert!(crate::location::delete_edge(
        &mut doc,
        &Default::default(),
        &crate::location::Location::root(&path)
    ));
    assert_eq!(declarations(doc.root.as_ref()).len(), 3);
    assert_eq!(doc.cells.value(cell), before.cells.value(cell));
    let mut history = crate::history::History::default();
    history.record(before.clone());
    let restored = history.undo(doc).unwrap();
    assert_eq!(restored.root, before.root);
}

#[test]
fn opening_a_pane_wraps_a_bare_root_and_preserves_malformed_containers() {
    let value = Value::from(b"data".to_vec());
    let malformed = Value::record([(vocabulary::PANES, value.clone())]);
    assert!(!can_open(Some(&malformed)));
    assert!(append(&malformed, Side::Left, value.clone()).is_none());
    // A root without the workspace's fields becomes a new workspace's main
    // value, and stays one: the next pane opens beside the first.
    for root in [
        value.clone(),
        Value::record([(CellId::from_u128(1), value.clone())]),
    ] {
        assert!(can_open(Some(&root)));
        let (workspace, wrapped) = workspace_root(&root);
        assert!(wrapped);
        let (opened, path) = append(&workspace, Side::Left, value.clone()).unwrap();
        assert_eq!(
            crate::test_support::value_at(&opened, &[Step::Key(vocabulary::MAIN)]),
            Some(&root)
        );
        assert_eq!(crate::test_support::value_at(&opened, &path), Some(&value));
        assert_eq!(workspace_root(&opened), (opened.clone(), false));
    }
    assert!(!can_open(None));
}

#[test]
fn declared_panes_reconcile_without_losing_view_state() {
    let one = Declaration {
        side: Side::Left,
        path: vec![Step::Key(CellId::from_u128(1))],
    };
    let two = Declaration {
        side: Side::Left,
        path: vec![Step::Key(CellId::from_u128(2))],
    };
    let mut workspace = Workspace::default();
    let panes_path = [Step::Key(vocabulary::PANES)];
    workspace.sync_declared(&[one.clone(), two.clone()]);
    assert!(crate::annotations::folded(
        workspace.document.annotations.at(&panes_path),
        false
    ));
    assert!(workspace.document.annotations.at(&one.path).is_none());
    assert!(workspace.document.annotations.at(&two.path).is_none());
    crate::annotations::set_folded(
        &mut workspace.document.annotations,
        &panes_path,
        false,
        false,
    );
    let root = workspace.left.panes[0].view.root.clone();
    workspace.left.panes[0].view.scroll = Vec2::new(4.0, 9.0);
    workspace.left.panes[0].view.shown = Shown::RAW;

    workspace.sync_declared(&[two, one.clone()]);
    let moved = &workspace.left.panes[1].view;
    assert_eq!(moved.root, root);
    assert_eq!(moved.scroll, Vec2::new(4.0, 9.0));
    assert_eq!(moved.shown, Shown::RAW);
    assert!(!crate::annotations::folded(
        workspace.document.annotations.at(&panes_path),
        false
    ));
    assert!(workspace.can_move(&root, Move::Up));

    workspace.sync_declared(&[]);
    assert!(workspace.left.panes.is_empty());
    workspace.sync_declared(&[one]);
    assert!(!crate::annotations::folded(
        workspace.document.annotations.at(&panes_path),
        false
    ));
}

#[test]
fn minimums_override_without_rewriting_requested_percentages() {
    let sizes = allocate(&[0.05, 0.95], 1_000.0, 100.0);
    assert_eq!(sizes, [100.0, 900.0]);
    assert_eq!(allocate(&[0.1, 0.9], 100.0, 80.0), [50.0, 50.0]);
}

#[test]
fn divider_drags_update_requests_without_baking_in_minimums() {
    let mut workspace = Workspace::default();
    let upper = add_pane(&mut workspace, Side::Left);
    let lower = add_pane(&mut workspace, Side::Left);

    let columns = Divider::Columns(Side::Left);
    workspace.left_width = 0.05;
    workspace.start_resize(columns.clone(), Vec2::new(180.0, 0.0));
    assert!(workspace.resize(&columns, Vec2::new(180.0, 0.0), Size::new(1_000.0, 600.0),));
    assert_eq!(workspace.left_width, 0.05);
    let geometry = workspace.geometry(Size::new(1_000.0, 600.0), 1.0);
    let left_width = geometry
        .views
        .iter()
        .find(|placed| placed.view.root == upper)
        .unwrap()
        .rect
        .width();
    assert_eq!(left_width, 180.0);
    assert_eq!(workspace.left_width, 0.05);

    workspace.finish_resize(&columns);
    let panes = Divider::Panes {
        side: Side::Left,
        before: upper,
        after: lower,
    };
    workspace.start_resize(panes.clone(), Vec2::new(0.0, 300.0));
    assert!(workspace.resize(&panes, Vec2::new(0.0, 450.0), Size::new(1_000.0, 600.0),));
    assert_eq!(workspace.left.panes[0].height, 0.75);
    assert_eq!(workspace.left.panes[1].height, 0.25);
}

#[test]
fn geometry_keeps_document_full_height_and_stacks_each_side() {
    let mut workspace = Workspace::default();
    let document = workspace.document.root.clone();
    add_pane(&mut workspace, Side::Left);
    add_pane(&mut workspace, Side::Left);
    add_pane(&mut workspace, Side::Right);
    let geometry = workspace.geometry(Size::new(900.0, 600.0), 1.0);
    let document = geometry
        .views
        .iter()
        .find(|placed| placed.view.root == document)
        .unwrap();
    assert_eq!((document.rect.y0, document.rect.y1), (0.0, 600.0));
    let left: Vec<_> = geometry
        .views
        .iter()
        .filter(|placed| workspace.side(&placed.view.root) == Some(Side::Left))
        .collect();
    assert_eq!(left.len(), 2);
    assert_eq!(left[0].rect.height(), left[1].rect.height());
    assert_eq!(geometry.dividers.len(), 3);
}

#[test]
fn only_the_root_declares_libraries() {
    let (world, names) = planets(PLANET_VIEW);
    let sources = world.sources();
    assert_eq!(crate::workspace::libraries(&sources).len(), 1);
    let mut nested = (*world.model.doc).clone();
    nested.root = Some(Value::record([(
        names["mars"],
        nested.root.clone().unwrap(),
    )]));
    let nested_sources = crate::sources::Sources {
        doc: &nested,
        libraries: &world.stack.libraries,
    };
    assert!(crate::workspace::libraries(&nested_sources).is_empty());
}

#[test]
fn the_document_view_starts_at_a_workspace_roots_main_value() {
    let panes = Value::record([(
        vocabulary::PANES,
        Value::record([(vocabulary::LEFT, Value::list([]))]),
    )]);
    let main = vec![Step::Key(vocabulary::MAIN)];
    let with = |fields: &[(CellId, Value)]| {
        Value::record(
            panes
                .as_record()
                .unwrap()
                .iter()
                .map(|(key, value)| (*key, value.clone()))
                .chain(fields.iter().cloned()),
        )
    };
    let data = Value::record([]);
    assert_eq!(
        main_path(Some(&with(&[(vocabulary::MAIN, data.clone())]))),
        Some(main.clone())
    );
    // Only the workspace's own fields: its main value was deleted.
    assert_eq!(main_path(Some(&panes)), Some(main.clone()));
    assert_eq!(
        main_path(Some(&with(&[(vocabulary::LIBRARIES, Value::list([]))]))),
        Some(main)
    );
    // Any other root draws whole.
    assert_eq!(
        main_path(Some(&with(&[(gid::new_cell_id(), data.clone())]))),
        None
    );
    assert_eq!(main_path(Some(&data)), None);
    assert_eq!(main_path(Some(&Value::list([]))), None);
    assert_eq!(main_path(None), None);
}

#[test]
fn opening_a_pane_on_a_bare_root_wraps_it_and_keeps_its_folds() {
    use crate::driver::Driver;
    let [outer, inner] = [CellId::from_u128(1), CellId::from_u128(2)];
    let root = Value::record([(outer, Value::record([(inner, Value::list([]))]))]);
    let mut world = crate::test_editor(gid::Document {
        root: Some(root.clone()),
        cells: Cells::new(),
    });
    crate::annotations::set_folded(
        &mut world.model.workspace.document.annotations,
        &[Step::Key(outer)],
        false,
        true,
    );
    let mut driver = Driver::new(world, Driver::VIEWPORT);
    driver.select(&[Step::Key(outer)]);
    driver
        .runner
        .update_frame(1.0, Driver::VIEWPORT, |editor, dispatch, _| {
            editor.run_doc_command(
                crate::command::DocCommand::OpenPane(Side::Left),
                dispatch.geometry(1.0),
            );
            true
        });
    driver.present();
    let opened = driver.editor().model.doc.root.clone().unwrap();
    assert_eq!(
        crate::test_support::value_at(&opened, &[Step::Key(vocabulary::MAIN)]),
        Some(&root)
    );
    let main = |steps: &[Step]| [&[Step::Key(vocabulary::MAIN)][..], steps].concat();
    // The pane shows the selected place, which the wrapping moved under
    // `main`, rather than a copy of it.
    let panes = declarations(Some(&opened));
    assert_eq!(panes.len(), 1);
    assert_eq!(
        crate::test_support::value_at(&opened, &panes[0].path),
        Some(&crate::location::value(&crate::location::Location::root(
            main(&[Step::Key(outer)])
        )))
    );
    // The document view draws the same value, now at `main`, folded as it
    // was, and none of the workspace's own fields.
    let in_document = |path: &[Step]| {
        driver
            .runner
            .frame
            .dispatch
            .landmarks
            .iter()
            .any(|landmark| {
                landmark.root.as_ref() == Some(&crate::test_root())
                    && landmark.path.as_ref() == path
            })
    };
    assert!(in_document(&main(&[Step::Key(outer)])));
    assert!(!in_document(&main(&[Step::Key(outer), Step::Key(inner)])));
    assert!(!driver.drawn(&[Step::Key(vocabulary::PANES)]));
}

#[test]
fn a_pane_opened_inside_a_cell_shows_that_place_through_the_cell() {
    use crate::driver::Driver;
    use crate::location::{Location, Start};
    let [label, cell] = [CellId::from_u128(1), CellId::from_u128(2)];
    let mut cells = Cells::new();
    cells.set_value(
        cell,
        Value::list([
            crate::libraries::f64::value(1.0),
            crate::libraries::f64::value(2.0),
        ]),
    );
    let mut world = crate::test_editor(gid::Document {
        root: Some(Value::record([(label, cell.into())])),
        cells,
    });
    let first = cell_positions(&world, cell)[0].clone();
    world.model.selection = Some(crate::selection::Selection::edge(
        &crate::test_root(),
        crate::editing::Scope::default(),
        vec![
            Step::Key(label),
            Step::Follow(gid::Resolution::Document),
            Step::Element(first.clone()),
        ],
    ));
    let mut driver = Driver::new(world, Driver::VIEWPORT);
    driver
        .runner
        .update_frame(1.0, Driver::VIEWPORT, |editor, dispatch, _| {
            editor.run_doc_command(
                crate::command::DocCommand::OpenPane(Side::Left),
                dispatch.geometry(1.0),
            );
            true
        });
    driver.present();
    let opened = driver.editor().model.doc.root.clone().unwrap();
    let declaration = declarations(Some(&opened)).remove(0).path;
    let entry = crate::test_support::value_at(&opened, &declaration).unwrap();
    // Anchored at the nearest cell: the steps start inside its definition.
    let place = crate::location::read(entry).unwrap();
    assert_eq!(
        place.start,
        Start::Cell {
            cell,
            resolution: gid::Resolution::Document
        }
    );
    assert_eq!(place.path.as_ref(), [Step::Element(first.clone())]);
    // The pane draws the place through its entry's own cell, and an edit
    // there lands in the cell, which the document view reads.
    let shown = location_path(entry, &declaration).unwrap();
    assert!(driver.drawn(&shown));
    let editor = &mut driver.runner.editor;
    assert!(crate::location::set_value(
        &mut editor.model.doc,
        &editor.stack.libraries,
        &Location::root(shown),
        crate::libraries::f64::value(5.0),
    ));
    // Opening the pane wrapped the bare root under `main`.
    assert_eq!(
        editor.sources().resolve_path(&[
            Step::Key(vocabulary::MAIN),
            Step::Key(label),
            Step::Follow(gid::Resolution::Document),
            Step::Element(first),
        ]),
        Some(&crate::libraries::f64::value(5.0))
    );
}

#[test]
fn closing_a_location_pane_removes_its_entry_and_leaves_its_place() {
    use crate::command::DocCommand;
    use crate::driver::Driver;
    let outer = CellId::from_u128(1);
    let root = Value::record([(outer, Value::list([]))]);
    let mut driver = Driver::new(
        crate::test_editor(gid::Document {
            root: Some(root.clone()),
            cells: Cells::new(),
        }),
        Driver::VIEWPORT,
    );
    let run = |driver: &mut Driver, command: DocCommand| {
        driver
            .runner
            .update_frame(1.0, Driver::VIEWPORT, |editor, dispatch, _| {
                editor.run_doc_command(command, dispatch.geometry(1.0));
                true
            });
        driver.present();
    };
    driver.select(&[Step::Key(outer)]);
    assert!(!driver.editor().menu_availability().close_pane);
    // Opening selects the place in its new pane.
    run(&mut driver, DocCommand::OpenPane(Side::Left));
    assert!(driver.editor().menu_availability().close_pane);
    run(&mut driver, DocCommand::ClosePane);
    let closed = driver.editor().model.doc.root.clone().unwrap();
    assert!(declarations(Some(&closed)).is_empty());
    assert_eq!(
        crate::test_support::value_at(&closed, &[Step::Key(vocabulary::MAIN)]),
        Some(&root)
    );
    assert!(driver.editor().model.selection.is_none());
    run(&mut driver, DocCommand::Undo);
    assert_eq!(
        declarations(driver.editor().model.doc.root.as_ref()).len(),
        1
    );
}

#[test]
fn a_location_pane_whose_place_is_gone_shows_an_absent_naming_it() {
    use crate::driver::Driver;
    let outer = CellId::from_u128(1);
    let mut driver = Driver::new(
        crate::test_editor(gid::Document {
            root: Some(Value::record([(
                outer,
                Value::list([crate::libraries::f64::value(1.0)]),
            )])),
            cells: Cells::new(),
        }),
        Driver::VIEWPORT,
    );
    let element = driver.element(&[Step::Key(outer)], 0, &[]);
    driver.select(&element);
    driver
        .runner
        .update_frame(1.0, Driver::VIEWPORT, |editor, dispatch, _| {
            editor.run_doc_command(
                crate::command::DocCommand::OpenPane(Side::Left),
                dispatch.geometry(1.0),
            );
            true
        });
    driver.present();
    let place = [&[Step::Key(vocabulary::MAIN)][..], &element].concat();
    let remove = |driver: &mut Driver, path: &[Step]| {
        let editor = &mut driver.runner.editor;
        let root = crate::spine::without(editor.model.doc.root.as_ref().unwrap(), path).unwrap();
        Rc::make_mut(&mut editor.model.doc).root = Some(root);
        driver.present();
    };
    let writable = |driver: &Driver| {
        driver
            .editor()
            .model
            .selection
            .as_ref()
            .unwrap()
            .writable(&driver.editor().sources())
    };
    // With only the element deleted, its place in the list is still there to
    // write, as any empty place is.
    remove(&mut driver, &place);
    driver.select(&place);
    assert!(writable(&driver));
    // With the list gone too, nothing can be written there, and the pane
    // says which place it can't find.
    remove(
        &mut driver,
        &[Step::Key(vocabulary::MAIN), Step::Key(outer)],
    );
    driver.select(&place);
    assert!(!writable(&driver));
    driver.keystroke(
        ui_events::keyboard::Key::Character("c".into()),
        crate::test_support::command(),
    );
    assert_eq!(
        driver.copied(),
        Some(crate::location::not_found(
            &crate::location::Location::root(place)
        ))
    );
}

fn cell_positions(world: &crate::Editor, cell: CellId) -> Vec<gid::Position> {
    world
        .model
        .doc
        .cells
        .value(cell)
        .and_then(Value::as_list)
        .unwrap()
        .keys()
        .cloned()
        .collect()
}

#[test]
fn a_root_of_only_the_workspace_fields_draws_an_empty_main() {
    use crate::driver::Driver;
    let (root, _) = append(&Value::record([]), Side::Left, Value::list([])).unwrap();
    let driver = Driver::new(
        crate::test_editor(gid::Document {
            root: Some(root),
            cells: Cells::new(),
        }),
        Driver::VIEWPORT,
    );
    assert!(driver.drawn(&[Step::Key(vocabulary::MAIN)]));
    // The record around it is drawn as nothing of its own, so it can't be
    // selected.
    assert!(!driver.drawn(&[]));
    assert!(!driver.drawn(&[Step::Key(vocabulary::PANES)]));
}
