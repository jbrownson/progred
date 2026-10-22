use super::*;
use crate::command::DocCommand;
use crate::driver::Driver;
use crate::sample::{PLANET_VIEW, planets};
use crate::test_examples::{pane_root, part};
use gid::Cells;
use std::rc::Rc;

/// Runs `command` as a menu would, and draws the next frame.
fn run(driver: &mut Driver, command: DocCommand) {
    driver
        .runner
        .update_frame(1.0, Driver::VIEWPORT, |editor, dispatch, _| {
            editor.run_doc_command(command, dispatch.geometry(1.0));
            true
        });
    driver.present();
}

fn driver_of(root: Option<Value>, cells: Cells) -> Driver {
    Driver::new(
        crate::test_editor(gid::Document { root, cells }),
        Driver::VIEWPORT,
    )
}

/// The opened pane's id, where `root` is one.
fn id(root: &Root) -> CellId {
    match root.target() {
        Target::Opened { id } => *id,
        target => panic!("not an opened pane: {target:?}"),
    }
}

/// Where the opened pane `root` draws `place`.
fn opened_at(root: &Root, place: crate::location::Location<'_>) -> occurrence::Path {
    vec![
        occurrence::Step::Opened(id(root)),
        occurrence::Step::jump(place),
    ]
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
fn a_path_names_the_view_it_is_drawn_in() {
    let part = part(0, 1);
    let pane = pane_root(&part);
    // A document pane's paths start with its part and a pane step.
    assert_eq!(view_of(&pane), Root::pane(part.clone()));
    assert_eq!(
        view_of(&occurrence::below(&pane, &[Step::Key(gid::new_cell_id())])),
        Root::pane(part.clone())
    );
    // A jump at a part, as a document projection might draw one, is still
    // the document view's: only a pane step starts a pane.
    let mut jumped = occurrence::of(&part);
    jumped.push(occurrence::Step::jump(crate::location::Location::root(
        part.clone(),
    )));
    assert_eq!(view_of(&jumped), Root::document());
    // The part drawn as data is the document view's, as is the root.
    assert_eq!(view_of(&occurrence::of(&part)), Root::document());
    assert_eq!(view_of(&[]), Root::document());
    // An opened pane's paths start with the step naming it.
    let opened = Root::opened(gid::new_cell_id());
    let place = crate::location::Location::root(part);
    assert_eq!(view_of(&opened_at(&opened, place)), opened);
}

#[test]
fn duplicate_views_keep_independent_projection_state() {
    let mut workspace = Workspace::default();
    let place = crate::location::value(&crate::location::Location::root(Vec::new()));
    let one = workspace.open(Side::Left, place.clone());
    let other = workspace.open(Side::Right, place);
    crate::annotations::set_folded(
        &mut workspace.view_mut(&one).unwrap().annotations,
        &[],
        false,
        true,
    );
    let folded = |root: &Root| {
        crate::annotations::folded(workspace.view(root).unwrap().annotations.at(&[]), false)
    };
    assert!(folded(&one));
    assert!(!folded(&other));
    assert!(!folded(workspace.document_root()));
}

#[test]
fn a_root_row_or_column_splits_into_its_parts() {
    let [a, b, c] = [1, 2, 3].map(|n| Value::from(vec![n]));
    // A part that splits again gives its own parts, in order.
    let root = row([column([a.clone(), b.clone()]), c.clone()]);
    let panes = panes(Some(&root));
    assert_eq!(
        panes,
        [
            [part(0, 2), part(0, 2)].concat(),
            [part(0, 2), part(1, 2)].concat(),
            part(1, 2),
        ]
    );
    for (pane, value) in panes.iter().zip([&a, &b, &c]) {
        assert_eq!(crate::test_support::value_at(&root, pane), Some(value));
    }
    // A row with no parts, any other root and no root at all are data.
    assert!(self::panes(Some(&row([]))).is_empty());
    assert!(self::panes(Some(&a)).is_empty());
    assert!(self::panes(None).is_empty());
    // So is a row deeper in the document.
    let nested = Value::record([(gid::new_cell_id(), root)]);
    assert!(self::panes(Some(&nested)).is_empty());
}

#[test]
fn a_part_moves_along_its_row_or_column() {
    let [a, b] = [1, 2].map(|n| Value::from(vec![n]));
    let root = row([a.clone(), column([b.clone(), b.clone()])]);
    let first = part(0, 2);
    // Along a row, left and right.
    assert!(move_part(&root, &first, Move::Left).is_none());
    assert!(move_part(&root, &first, Move::Up).is_none());
    let (moved, path) = move_part(&root, &first, Move::Right).unwrap();
    assert_eq!(crate::test_support::value_at(&moved, &path), Some(&a));
    assert_eq!(panes(Some(&moved)).last(), Some(&path));
    // Along a column, up and down.
    let upper = [part(1, 2), part(0, 2)].concat();
    assert!(move_part(&root, &upper, Move::Right).is_none());
    assert!(move_part(&root, &upper, Move::Up).is_none());
    let (moved, path) = move_part(&root, &upper, Move::Down).unwrap();
    assert_eq!(panes(Some(&moved))[2], path);
    // The root itself has no row to move along.
    assert!(move_part(&root, &[], Move::Right).is_none());
}

#[test]
fn document_panes_reconcile_without_losing_view_state() {
    let entry = || Value::list([]);
    let mut workspace = Workspace::default();
    let root = row([entry(), entry()]);
    workspace.sync(Some(&root));
    let [first, second] = [0, 1].map(|index| Root::pane(part(index, 2)));
    let view = workspace.view_mut(&first).unwrap();
    view.scroll = Vec2::new(4.0, 9.0);
    view.shown = Shown::RAW;
    // A part that's still there keeps its view.
    let fewer = crate::spine::without(&root, &part(1, 2)).unwrap();
    workspace.sync(Some(&fewer));
    assert!(workspace.view(&second).is_none());
    let kept = workspace.view(&first).unwrap();
    assert_eq!(kept.scroll, Vec2::new(4.0, 9.0));
    assert_eq!(kept.shown, Shown::RAW);
    // Plain and Raw show the document's root whole, with no panes.
    workspace.document.shown = Shown::RAW;
    workspace.sync(Some(&root));
    assert!(workspace.view(&first).is_none());
    let geometry = workspace.geometry(Some(&root), Size::new(900.0, 600.0), 1.0);
    assert_eq!(geometry.views.len(), 1);
    assert_eq!(geometry.views[0].view.root, *workspace.document_root());
}

#[test]
fn minimums_override_without_rewriting_requested_shares() {
    let sizes = allocate(&[0.05, 0.95], 1_000.0, 100.0);
    assert_eq!(sizes, [100.0, 900.0]);
    assert_eq!(allocate(&[0.1, 0.9], 100.0, 80.0), [50.0, 50.0]);
}

#[test]
fn divider_drags_move_share_between_neighbours() {
    let mut workspace = Workspace::default();
    let opened = workspace.open(Side::Left, Value::record([]));
    let size = Size::new(1_001.0, 600.0);
    let width = |workspace: &Workspace, root: &Root| {
        workspace
            .geometry(None, size, 1.0)
            .views
            .iter()
            .find(|placed| placed.view.root == *root)
            .unwrap()
            .rect
            .width()
    };
    assert_eq!(width(&workspace, &opened), 500.0);
    let geometry = workspace.geometry(None, size, 1.0);
    let [placed] = geometry.dividers.as_slice() else {
        panic!("one rule, between the pane and the document")
    };
    let divider = placed.divider.clone();
    assert_eq!(
        divider,
        Divider {
            direction: Direction::Across,
            before: Slot::Opened(id(&opened)),
            after: Slot::Document,
        }
    );
    // Dragging a quarter of the width right moves a quarter of the shares.
    assert!(workspace.start_resize(divider.clone(), Vec2::new(500.0, 0.0), placed.rate));
    assert!(workspace.resize(&divider, Vec2::new(750.0, 0.0)));
    assert!(workspace.finish_resize(&divider));
    assert_eq!(width(&workspace, &opened), 750.0);
    // A share too small for the minimum is drawn at the minimum, and kept.
    assert!(workspace.start_resize(divider.clone(), Vec2::new(750.0, 0.0), placed.rate));
    assert!(workspace.resize(&divider, Vec2::new(0.0, 0.0)));
    assert_eq!(workspace.share(&Slot::Opened(id(&opened))), 0.0);
    assert_eq!(width(&workspace, &opened), 180.0);
    assert_eq!(workspace.share(&Slot::Opened(id(&opened))), 0.0);
    // A rule whose part has gone can't be dragged on.
    assert!(workspace.close(&opened));
    workspace.sync(None);
    assert!(!workspace.resize(&divider, Vec2::new(10.0, 0.0)));
}

#[test]
fn opened_panes_sit_beside_the_documents_split_part() {
    let [a, b, c] = [1, 2, 3].map(|n| Value::from(vec![n]));
    let root = row([column([a, b]), c]);
    let mut workspace = Workspace::default();
    workspace.sync(Some(&root));
    let opened = workspace.open(Side::Left, Value::record([]));
    let geometry = workspace.geometry(Some(&root), Size::new(900.0, 600.0), 1.0);
    let rect = |root: &Root| {
        geometry
            .views
            .iter()
            .find(|placed| placed.view.root == *root)
            .map(|placed| placed.rect)
    };
    // The document's part splits as its root does, so its own view isn't
    // drawn.
    assert_eq!(geometry.views.len(), 4);
    assert!(rect(workspace.document_root()).is_none());
    let opened = rect(&opened).unwrap();
    let upper = rect(&Root::pane([part(0, 2), part(0, 2)].concat())).unwrap();
    let lower = rect(&Root::pane([part(0, 2), part(1, 2)].concat())).unwrap();
    let beside = rect(&Root::pane(part(1, 2))).unwrap();
    assert_eq!((opened.x0, opened.y0, opened.y1), (0.0, 0.0, 600.0));
    assert_eq!(upper.x0 - opened.x1, 1.0);
    assert_eq!((upper.x0, upper.x1), (lower.x0, lower.x1));
    assert_eq!(lower.y0 - upper.y1, 1.0);
    assert_eq!(upper.height(), lower.height());
    assert_eq!(beside.x0 - upper.x1, 1.0);
    assert_eq!((beside.y0, beside.y1, beside.x1), (0.0, 600.0, 900.0));
    // A rule between the pane and the document, and one in each split.
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
fn opening_a_pane_shows_the_selected_place_and_leaves_the_document_alone() {
    let [outer, inner] = [CellId::from_u128(1), CellId::from_u128(2)];
    let root = Value::record([(outer, Value::record([(inner, Value::list([]))]))]);
    let mut world = crate::test_editor(gid::Document {
        root: Some(root.clone()),
        cells: Cells::new(),
    });
    crate::annotations::set_folded(
        &mut world.model.workspace.document.annotations,
        &[occurrence::Step::Key(outer)],
        false,
        true,
    );
    let mut driver = Driver::new(world, Driver::VIEWPORT);
    driver.select(&[Step::Key(outer)]);
    run(&mut driver, DocCommand::OpenPane(Side::Left));
    assert_eq!(driver.editor().model.doc.root.as_ref(), Some(&root));
    assert!(!driver.editor().model.dirty());
    assert!(!driver.editor().model.history.can_undo());
    // The pane holds the selected place, not a copy of it, and selects it.
    let opened = driver.editor().model.workspace.opened().next().unwrap();
    let place = crate::location::Location::root(vec![Step::Key(outer)]);
    assert_eq!(opened.entry, crate::location::value(&place));
    let shown = opened_at(&opened.view.root, place);
    assert!(driver.drawn(&shown));
    assert_eq!(driver.selection(), shown);
    // The document view draws the root as it did, folded as it was.
    assert!(driver.drawn(&[Step::Key(outer)]));
    assert!(!driver.drawn(&[Step::Key(outer), Step::Key(inner)]));
}

#[test]
fn a_pane_opened_inside_a_cell_shows_that_place_through_the_cell() {
    use crate::location::Start;
    let [label, cell] = [CellId::from_u128(1), CellId::from_u128(2)];
    let mut cells = Cells::new();
    cells.set_value(
        cell,
        Value::list([
            crate::libraries::f64::value(1.0),
            crate::libraries::f64::value(2.0),
        ]),
    );
    let mut driver = driver_of(Some(Value::record([(label, cell.into())])), cells);
    let first = driver.element(
        &[Step::Key(label), Step::Follow(gid::Resolution::Document)],
        0,
        &[],
    );
    driver.select(&first);
    run(&mut driver, DocCommand::OpenPane(Side::Left));
    let opened = driver.editor().model.workspace.opened().next().unwrap();
    // Anchored at the nearest cell: the steps start inside its definition.
    let place = crate::location::read(&opened.entry).unwrap();
    assert_eq!(
        place.start,
        Start::Cell {
            cell,
            resolution: gid::Resolution::Document
        }
    );
    assert_eq!(place.path.as_ref(), &first[2..]);
    // The pane draws the place through a jump, and an edit there lands in
    // the cell, which the document view reads.
    let shown = opened_at(&opened.view.root, place);
    assert!(driver.drawn(&shown));
    let editor = &mut driver.runner.editor;
    assert!(crate::location::set_value(
        &mut editor.model.doc,
        &editor.stack.libraries,
        &occurrence::location(&shown).unwrap(),
        crate::libraries::f64::value(5.0),
    ));
    assert_eq!(
        editor.sources().resolve_path(&first),
        Some(&crate::libraries::f64::value(5.0))
    );
}

#[test]
fn closing_an_opened_pane_leaves_its_place() {
    let outer = CellId::from_u128(1);
    let root = Value::record([(outer, Value::list([]))]);
    let mut driver = driver_of(Some(root.clone()), Cells::new());
    driver.select(&[Step::Key(outer)]);
    assert!(!driver.editor().menu_availability().close_pane);
    // Opening selects the place in its new pane.
    run(&mut driver, DocCommand::OpenPane(Side::Left));
    assert!(driver.editor().menu_availability().close_pane);
    run(&mut driver, DocCommand::ClosePane);
    assert_eq!(driver.editor().model.workspace.opened().count(), 0);
    assert_eq!(driver.editor().model.doc.root.as_ref(), Some(&root));
    assert!(driver.editor().model.selection.is_none());
    assert!(!driver.editor().model.history.can_undo());
}

#[test]
fn closing_a_document_pane_removes_its_part() {
    let [a, b] = [1, 2].map(|n| Value::from(vec![n]));
    let root = row([a, b.clone()]);
    let mut driver = driver_of(Some(root.clone()), Cells::new());
    driver.select(&pane_root(&part(0, 2)));
    assert!(driver.editor().menu_availability().close_pane);
    run(&mut driver, DocCommand::ClosePane);
    let closed = driver.editor().model.doc.root.clone().unwrap();
    assert_eq!(panes(Some(&closed)), [part(1, 2)]);
    assert_eq!(
        crate::test_support::value_at(&closed, &part(1, 2)),
        Some(&b)
    );
    assert!(driver.editor().model.selection.is_none());
    run(&mut driver, DocCommand::Undo);
    assert_eq!(driver.editor().model.doc.root.as_ref(), Some(&root));
}

#[test]
fn a_location_pane_whose_place_is_gone_shows_an_absent_naming_it() {
    let outer = CellId::from_u128(1);
    let mut driver = driver_of(
        Some(Value::record([(
            outer,
            Value::list([crate::libraries::f64::value(1.0)]),
        )])),
        Cells::new(),
    );
    let element = driver.element(&[Step::Key(outer)], 0, &[]);
    driver.select(&element);
    run(&mut driver, DocCommand::OpenPane(Side::Left));
    let place = crate::location::Location::root(element.clone());
    let mut shown = opened_at(
        &driver
            .editor()
            .model
            .workspace
            .opened()
            .next()
            .unwrap()
            .view
            .root,
        place.clone(),
    );
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
    remove(&mut driver, &element);
    driver.select(&shown);
    assert!(writable(&driver));
    // With the list gone too, nothing can be written there, and the pane
    // says which place it can't find, a value computed in its place.
    remove(&mut driver, &[Step::Key(outer)]);
    shown.push(occurrence::Step::Computed);
    driver.select(&shown);
    assert!(!writable(&driver));
    driver.keystroke(
        ui_events::keyboard::Key::Character("c".into()),
        crate::test_support::command(),
    );
    assert_eq!(driver.copied(), Some(crate::location::not_found(&place)));
}

#[test]
fn a_computed_value_opens_as_a_copy() {
    let key = CellId::from_u128(1);
    let mut world = crate::test_editor(gid::Document {
        root: Some(Value::record([(key, Value::list([]))])),
        cells: Cells::new(),
    });
    let copy = Value::from(b"computed".to_vec());
    let root = world.model.workspace.open(Side::Right, copy.clone());
    // Nothing stores it, so the pane draws it as computed.
    let shown = vec![
        occurrence::Step::Opened(id(&root)),
        occurrence::Step::Computed,
    ];
    assert_eq!(
        world
            .model
            .workspace
            .root_path(&world.sources(), &root, true),
        shown
    );
    world.model.selection = Some(crate::selection::Selection::edge(shown.clone()));
    let driver = Driver::new(world, Driver::VIEWPORT);
    assert!(driver.drawn(&shown));
    assert!(
        !driver
            .editor()
            .model
            .selection
            .as_ref()
            .unwrap()
            .writable(&driver.editor().sources())
    );
}

#[test]
fn moving_a_document_pane_keeps_its_selection_on_what_it_was_on() {
    let root = row([
        Value::list([crate::libraries::f64::value(1.0)]),
        Value::record([]),
    ]);
    let mut driver = driver_of(Some(root), Cells::new());
    let element = |driver: &Driver, part: &[Step]| {
        occurrence::below(
            &pane_root(part),
            &[driver.element(part, 0, &[])[part.len()].clone()],
        )
    };
    let first = part(0, 2);
    driver.select(&element(&driver, &first));
    assert!(driver.editor().menu_availability().move_right);
    assert!(!driver.editor().menu_availability().move_left);
    assert!(!driver.editor().menu_availability().move_up);
    run(&mut driver, DocCommand::MovePane(Move::Right));
    let moved = panes(driver.editor().model.doc.root.as_ref()).remove(1);
    assert_ne!(moved, first);
    // The selection moved with the part.
    assert_eq!(driver.selection(), element(&driver, &moved));
    assert!(driver.drawn(&element(&driver, &moved)));
}

#[test]
fn an_opened_pane_moves_along_the_editors_row() {
    let mut workspace = Workspace::default();
    let opened = workspace.open(Side::Left, Value::record([]));
    let size = Size::new(900.0, 600.0);
    let x = |workspace: &Workspace| {
        workspace
            .geometry(None, size, 1.0)
            .views
            .iter()
            .find(|placed| placed.view.root == opened)
            .unwrap()
            .rect
            .x0
    };
    assert_eq!(x(&workspace), 0.0);
    assert!(!workspace.can_move(&opened, Move::Left, None));
    assert!(!workspace.can_move(&opened, Move::Up, None));
    assert!(workspace.can_move(&opened, Move::Right, None));
    assert!(workspace.move_opened(&opened, Move::Right));
    assert!(x(&workspace) > 0.0);
    assert!(!workspace.can_move(&opened, Move::Right, None));
    assert!(!workspace.can_move(workspace.document_root(), Move::Left, None));
}

#[test]
fn open_library_shows_a_loaded_library_read_only_once() {
    use crate::libraries::presentation;
    let root = Value::record([(CellId::from_u128(1), Value::list([]))]);
    let mut driver = driver_of(Some(root.clone()), Cells::new());
    let place = crate::location::Location::in_cell(
        presentation::ID,
        gid::Resolution::Library(presentation::ID),
        Vec::new(),
    );
    run(&mut driver, DocCommand::OpenLibrary(presentation::ID));
    assert_eq!(driver.editor().model.doc.root.as_ref(), Some(&root));
    let opened = driver.editor().model.workspace.opened().next().unwrap();
    assert_eq!(opened.entry, crate::location::value(&place));
    // The pane draws the library, its cells among it, and selects it.
    let shown = opened_at(&opened.view.root, place);
    assert_eq!(driver.selection(), shown);
    let cells = [
        shown.clone(),
        vec![occurrence::Step::Key(crate::library::vocabulary::CELLS)],
    ]
    .concat();
    assert!(driver.drawn(&cells));
    driver.select(&cells);
    assert!(
        !driver
            .editor()
            .model
            .selection
            .as_ref()
            .unwrap()
            .writable(&driver.editor().sources())
    );
    // Opening it again selects that pane rather than opening another.
    driver.select(&[Step::Key(CellId::from_u128(1))]);
    run(&mut driver, DocCommand::OpenLibrary(presentation::ID));
    assert_eq!(driver.editor().model.workspace.opened().count(), 1);
    assert_eq!(driver.selection(), shown);
}

#[test]
fn an_empty_document_opens_a_library_and_stays_empty() {
    use crate::libraries::presentation;
    let mut driver = driver_of(None, Cells::new());
    let command = DocCommand::OpenLibrary(presentation::ID);
    assert!(driver.editor().menu_availability().doc_enabled(command));
    run(&mut driver, command);
    let opened = &driver
        .editor()
        .model
        .workspace
        .opened()
        .next()
        .unwrap()
        .view
        .root;
    let shown = opened_at(
        opened,
        crate::location::Location::in_cell(
            presentation::ID,
            gid::Resolution::Library(presentation::ID),
            Vec::new(),
        ),
    );
    assert_eq!(driver.selection(), shown);
    // The document is still empty, so its root still offers starting points.
    assert!(driver.editor().model.doc.root.is_none());
    assert!(!driver.editor().model.dirty());
    assert!(driver.drawn::<Step>(&[]));
}
