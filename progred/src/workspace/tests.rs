use super::*;
use crate::command::DocCommand;
use crate::display::widget::view::Area;
use crate::driver::Driver;
use crate::library::Libraries;
use crate::sample::{PLANET_VIEW, planets};
use crate::test_examples::part;
use crate::test_support::src;
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

/// A workspace whose document view is the test document's.
fn workspace() -> Workspace {
    Workspace::new(crate::test_document())
}

/// Reconciles `workspace` with a document whose root is `root`.
fn sync(workspace: &mut Workspace, root: Option<&Value>) {
    workspace.sync(&src(&document(root), &Libraries::default()));
}

fn document(root: Option<&Value>) -> gid::Document {
    gid::Document {
        root: root.cloned(),
        cells: Cells::new(),
    }
}

/// A view opened at `side` showing a copy of an empty record.
fn open_copy(workspace: &mut Workspace, side: Side) -> Area {
    Area::whole(workspace.open(side, Entry::Copy(Value::record([]))))
}

/// The view the editor opened, the one that isn't the document's.
fn opened(driver: &Driver) -> Rc<View> {
    let workspace = &driver.editor().model.workspace;
    workspace
        .views()
        .find(|view| **view != workspace.document)
        .cloned()
        .expect("an opened view")
}

/// Where a view of `place` draws it.
fn opened_at(place: crate::location::Location<'_>) -> occurrence::Path {
    View {
        entry: Entry::Place(place.into_owned()),
    }
    .root_path()
}

#[test]
fn a_library_toggles_whichever_way_the_area_lists_its_libraries() {
    let [one, other] = [(); 2].map(|_| gid::new_cell_id());
    let mut workspace = Workspace::default();
    workspace.toggle_library(None, one);
    assert!(!workspace.document_pane().shown.shows(one));
    assert!(workspace.document_pane().shown.shows(other));
    workspace.show(None, Shown::RAW);
    workspace.toggle_library(None, one);
    assert!(workspace.document_pane().shown.shows(one));
    assert!(!workspace.document_pane().shown.shows(other));
    workspace.toggle_library(None, one);
    assert_eq!(workspace.document_pane().shown, Shown::RAW);
}

#[test]
fn a_view_starts_where_its_entry_is() {
    let [field, cell] = [gid::new_cell_id(), gid::new_cell_id()];
    let from_root = crate::location::Location::root(vec![Step::Key(field)]);
    let in_cell = crate::location::Location::in_cell(cell, gid::Resolution::Document, Vec::new());
    let start = |entry| View { entry }.root_path();
    // A place from the document's root starts along its own steps, as the
    // document's view would draw it; one inside a cell, from a jump there;
    // and a copy, as computed.
    assert_eq!(
        start(Entry::Place(from_root)),
        occurrence::of(&[Step::Key(field)])
    );
    assert_eq!(
        start(Entry::Place(in_cell.clone())),
        [occurrence::Step::jump(in_cell)]
    );
    assert_eq!(
        start(Entry::Copy(Value::record([]))),
        [occurrence::Step::Computed]
    );
    // Two views of one place are two views.
    let [one, other] = [(); 2].map(|_| Rc::new(View::of_root()));
    assert_ne!(one, other);
    assert_eq!(one, one.clone());
}

#[test]
fn a_path_is_drawn_in_the_area_of_the_part_it_leads_into() {
    let mut workspace = workspace();
    let part = part(0, 2);
    let document = Area::whole(crate::test_document());
    // While the document is whole, every path is in its whole area.
    assert_eq!(
        workspace.area_of(&crate::test_document(), &occurrence::of(&part)),
        document
    );
    // Once its root splits, a path is in the area of the part it leads into,
    // a jump below the part included.
    sync(
        &mut workspace,
        Some(&row([Value::list([]), Value::list([])])),
    );
    let pane = Area::part(crate::test_document(), part.clone());
    assert_eq!(
        workspace.area_of(&crate::test_document(), &occurrence::of(&part)),
        pane
    );
    let mut jumped = occurrence::below(&occurrence::of(&part), &[Step::Key(gid::new_cell_id())]);
    jumped.push(occurrence::Step::jump(crate::location::Location::root(
        Vec::new(),
    )));
    assert_eq!(workspace.area_of(&crate::test_document(), &jumped), pane);
    // The root itself, which no part leads, is the document's whole area.
    assert_eq!(workspace.area_of(&crate::test_document(), &[]), document);
    // A view whose value doesn't split is one area, whatever its path.
    let opened = open_copy(&mut workspace, Side::Left);
    let below = occurrence::below(&[occurrence::Step::Computed], &part);
    assert_eq!(workspace.area_of(&opened.view, &below), opened);
}

#[test]
fn duplicate_views_keep_independent_projection_state() {
    let mut workspace = workspace();
    let place = Entry::Place(crate::location::Location::root(Vec::new()));
    let one = Area::whole(workspace.open(Side::Left, place.clone()));
    let other = Area::whole(workspace.open(Side::Right, place));
    crate::annotations::set_folded(
        &mut workspace.pane_mut(&one).unwrap().annotations,
        &[],
        false,
        true,
    );
    let folded = |area: &Area| {
        crate::annotations::folded(workspace.pane(area).unwrap().annotations.at(&[]), false)
    };
    assert!(folded(&one));
    assert!(!folded(&other));
    assert!(!folded(&workspace.document_area()));
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
    let mut workspace = workspace();
    let root = row([entry(), entry()]);
    sync(&mut workspace, Some(&root));
    let [first, second] = [0, 1].map(|index| Area::part(crate::test_document(), part(index, 2)));
    let pane = workspace.pane_mut(&first).unwrap();
    pane.scroll = Vec2::new(4.0, 9.0);
    pane.shown = Shown::RAW;
    // A part that's still there keeps its pane.
    let fewer = crate::spine::without(&root, &part(1, 2)).unwrap();
    sync(&mut workspace, Some(&fewer));
    assert!(workspace.pane(&second).is_none());
    let kept = workspace.pane(&first).unwrap();
    assert_eq!(kept.scroll, Vec2::new(4.0, 9.0));
    assert_eq!(kept.shown, Shown::RAW);
    // Plain and Raw show the document's root whole, with no panes.
    workspace.document_pane_mut().shown = Shown::RAW;
    sync(&mut workspace, Some(&root));
    assert!(workspace.pane(&first).is_none());
    let (document, libraries) = (document(Some(&root)), Libraries::default());
    let geometry = workspace.geometry(&src(&document, &libraries), Size::new(900.0, 600.0), 1.0);
    assert_eq!(geometry.panes.len(), 1);
    assert_eq!(geometry.panes[0].pane.area, workspace.document_area());
}

#[test]
fn minimums_override_without_rewriting_requested_shares() {
    let sizes = allocate(&[0.05, 0.95], 1_000.0, 100.0);
    assert_eq!(sizes, [100.0, 900.0]);
    assert_eq!(allocate(&[0.1, 0.9], 100.0, 80.0), [50.0, 50.0]);
}

#[test]
fn divider_drags_move_share_between_neighbours() {
    let mut workspace = workspace();
    let opened = open_copy(&mut workspace, Side::Left);
    let size = Size::new(1_001.0, 600.0);
    let (empty, libraries) = (document(None), Libraries::default());
    let width = |workspace: &Workspace, area: &Area| {
        workspace
            .geometry(&src(&empty, &libraries), size, 1.0)
            .panes
            .iter()
            .find(|placed| placed.pane.area == *area)
            .unwrap()
            .rect
            .width()
    };
    assert_eq!(width(&workspace, &opened), 500.0);
    let geometry = workspace.geometry(&src(&empty, &libraries), size, 1.0);
    let [placed] = geometry.dividers.as_slice() else {
        panic!("one rule, between the pane and the document")
    };
    let divider = placed.divider.clone();
    assert_eq!(
        divider,
        Divider {
            direction: Direction::Across,
            before: Slot::Layout(vec![0]),
            after: Slot::Layout(vec![1]),
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
    assert_eq!(workspace.share(&Slot::Layout(vec![0])), 0.0);
    assert_eq!(width(&workspace, &opened), 180.0);
    assert_eq!(workspace.share(&Slot::Layout(vec![0])), 0.0);
    // A rule whose part has gone can't be dragged on.
    assert!(workspace.close(&opened.view));
    sync(&mut workspace, None);
    assert!(!workspace.resize(&divider, Vec2::new(10.0, 0.0)));
}

#[test]
fn opened_panes_sit_beside_the_documents_split_part() {
    let [a, b, c] = [1, 2, 3].map(|n| Value::from(vec![n]));
    let root = row([column([a, b]), c]);
    let mut workspace = workspace();
    sync(&mut workspace, Some(&root));
    let opened = open_copy(&mut workspace, Side::Left);
    let (document, libraries) = (document(Some(&root)), Libraries::default());
    let geometry = workspace.geometry(&src(&document, &libraries), Size::new(900.0, 600.0), 1.0);
    let rect = |area: &Area| {
        geometry
            .panes
            .iter()
            .find(|placed| placed.pane.area == *area)
            .map(|placed| placed.rect)
    };
    // The document's view splits as its root does, so its whole area isn't
    // drawn.
    assert_eq!(geometry.panes.len(), 4);
    assert!(rect(&workspace.document_area()).is_none());
    let opened = rect(&opened).unwrap();
    let upper = rect(&Area::part(
        crate::test_document(),
        [part(0, 2), part(0, 2)].concat(),
    ))
    .unwrap();
    let lower = rect(&Area::part(
        crate::test_document(),
        [part(0, 2), part(1, 2)].concat(),
    ))
    .unwrap();
    let beside = rect(&Area::part(crate::test_document(), part(1, 2))).unwrap();
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
        &mut world.model.workspace.document_pane_mut().annotations,
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
    // The view holds the selected place, not a copy of it, and selects it,
    // at the place's own path.
    let opened = opened(&driver);
    let place = crate::location::Location::root(vec![Step::Key(outer)]);
    assert_eq!(opened.entry, Entry::Place(place.clone()));
    let shown = opened_at(place);
    assert_eq!(shown, occurrence::of(&[Step::Key(outer)]));
    assert!(driver.drawn_in(&opened, &shown));
    assert_eq!(
        *driver.editor().model.selection.as_ref().unwrap().view(),
        opened
    );
    assert_eq!(driver.selection(), shown);
    // It draws the place unfolded, while the document's view draws the root
    // as it did, folded as it was.
    let inside = [Step::Key(outer), Step::Key(inner)];
    assert!(driver.drawn_in(&opened, &inside));
    let document = driver.editor().model.workspace.document.clone();
    assert!(driver.drawn_in(&document, &[Step::Key(outer)]));
    assert!(!driver.drawn_in(&document, &inside));
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
    let opened = opened(&driver);
    // Anchored at the nearest cell: the steps start inside its definition.
    let Entry::Place(place) = opened.entry.clone() else {
        panic!("a place")
    };
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
    let shown = opened_at(place);
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
    assert_eq!(driver.editor().model.workspace.views().count(), 1);
    assert_eq!(driver.editor().model.doc.root.as_ref(), Some(&root));
    assert!(driver.editor().model.selection.is_none());
    assert!(!driver.editor().model.history.can_undo());
}

#[test]
fn closing_a_document_pane_removes_its_part() {
    let [a, b] = [1, 2].map(|n| Value::from(vec![n]));
    let root = row([a, b.clone()]);
    let mut driver = driver_of(Some(root.clone()), Cells::new());
    driver.select(&occurrence::of(&part(0, 2)));
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
    let mut shown = opened_at(place.clone());
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
    let view = world.model.workspace.open(Side::Right, Entry::Copy(copy));
    // Nothing stores it, so the view draws it as computed.
    let shown = vec![occurrence::Step::Computed];
    assert_eq!(
        root_path(&world.sources(), &Area::whole(view.clone()), true),
        shown
    );
    world.model.selection = Some(crate::selection::Selection::edge(&view, shown.clone()));
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
    let element = |driver: &Driver, part: &[Step]| driver.element(part, 0, &[]);
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
fn a_view_moves_along_the_editors_row() {
    let mut workspace = workspace();
    let opened = open_copy(&mut workspace, Side::Left);
    let size = Size::new(900.0, 600.0);
    let (empty, libraries) = (document(None), Libraries::default());
    let sources = src(&empty, &libraries);
    let x = |workspace: &Workspace| {
        workspace
            .geometry(&sources, size, 1.0)
            .panes
            .iter()
            .find(|placed| placed.pane.area == opened)
            .unwrap()
            .rect
            .x0
    };
    assert_eq!(x(&workspace), 0.0);
    assert!(!workspace.can_move(&sources, &opened, Move::Left));
    assert!(!workspace.can_move(&sources, &opened, Move::Up));
    assert!(workspace.can_move(&sources, &opened, Move::Right));
    assert!(workspace.move_view(&opened.view, Move::Right));
    assert!(x(&workspace) > 0.0);
    assert!(!workspace.can_move(&sources, &opened, Move::Right));
    // The document's view moves as any other does.
    assert!(!workspace.can_move(&sources, &workspace.document_area(), Move::Left));
    assert!(workspace.can_move(&sources, &workspace.document_area(), Move::Right));
}

#[test]
fn the_documents_view_never_closes() {
    let mut workspace = workspace();
    let document = workspace.document.clone();
    assert!(!workspace.close(&document));
    let opened = open_copy(&mut workspace, Side::Right);
    assert!(!workspace.close(&document));
    assert!(workspace.close(&opened.view));
    assert_eq!(workspace.views().collect::<Vec<_>>(), [&document]);
}

#[test]
fn an_opened_view_on_a_row_splits_too() {
    let [a, b] = [1, 2].map(|n| Value::from(vec![n]));
    let mut workspace = workspace();
    let opened = workspace.open(Side::Left, Entry::Copy(row([a.clone(), b])));
    sync(&mut workspace, None);
    let (empty, libraries) = (document(None), Libraries::default());
    let geometry = workspace.geometry(&src(&empty, &libraries), Size::new(900.0, 600.0), 1.0);
    // Its parts are its own areas, beside the document's whole one.
    let areas: Vec<_> = geometry
        .panes
        .iter()
        .map(|placed| placed.pane.area.clone())
        .collect();
    assert_eq!(
        areas,
        [
            Area::part(opened.clone(), part(0, 2)),
            Area::part(opened.clone(), part(1, 2)),
            workspace.document_area(),
        ]
    );
    // A part draws below the view's root, where its value is.
    let first = Area::part(opened.clone(), part(0, 2));
    let path = root_path(&src(&empty, &libraries), &first, true);
    assert_eq!(
        path,
        occurrence::below(&[occurrence::Step::Computed], &part(0, 2))
    );
    assert_eq!(workspace.area_of(&opened, &path), first);
    assert_eq!(
        opened.value(&src(&empty, &libraries), first.path()),
        Some(&a)
    );
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
    let opened = opened(&driver);
    assert_eq!(opened.entry, Entry::Place(place.clone()));
    // The pane draws the library, its cells among it, and selects it.
    let shown = opened_at(place);
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
    assert_eq!(driver.editor().model.workspace.views().count(), 2);
    assert_eq!(driver.selection(), shown);
}

#[test]
fn an_empty_document_opens_a_library_and_stays_empty() {
    use crate::libraries::presentation;
    let mut driver = driver_of(None, Cells::new());
    let command = DocCommand::OpenLibrary(presentation::ID);
    assert!(driver.editor().menu_availability().doc_enabled(command));
    run(&mut driver, command);
    let shown = opened_at(crate::location::Location::in_cell(
        presentation::ID,
        gid::Resolution::Library(presentation::ID),
        Vec::new(),
    ));
    assert_eq!(driver.selection(), shown);
    // The document is still empty, so its root still offers starting points.
    assert!(driver.editor().model.doc.root.is_none());
    assert!(!driver.editor().model.dirty());
    assert!(driver.drawn::<Step>(&[]));
}
