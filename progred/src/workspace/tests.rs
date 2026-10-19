use super::*;
use crate::sample::{PLANET_VIEW, planets};
use gid::Cells;

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
fn plain_and_raw_each_toggle_back_to_the_standard_projection() {
    let mut workspace = Workspace::default();
    let mut toggle = |projection| {
        workspace.toggle_projection(None, projection);
        workspace.document.projection
    };
    assert_eq!(toggle(Projection::Plain), Projection::Plain);
    assert_eq!(toggle(Projection::Raw), Projection::Raw);
    assert_eq!(toggle(Projection::Plain), Projection::Plain);
    assert_eq!(toggle(Projection::Plain), Projection::Standard);
    assert_eq!(toggle(Projection::Raw), Projection::Raw);
    assert_eq!(toggle(Projection::Raw), Projection::Standard);
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

    crate::annotations::set_collapsed(
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
        assert_eq!(crate::spine::get(&root, &location.path), Some(value));
        assert!(matches!(location.path.last(), Some(Step::Element(_))));
    }
    let (moved, path) = move_value(&root, &locations[1].path, Move::Up).unwrap();
    assert_eq!(declarations(Some(&moved))[0].path, path);
    assert_eq!(crate::spine::get(&moved, &path), Some(&values[1]));
    assert_eq!(
        crate::spine::get(&moved, &locations[0].path),
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
fn opening_panes_preserves_malformed_containers_and_nonrecord_roots() {
    let value = Value::from(b"data".to_vec());
    for root in [
        value.clone(),
        Value::record([(vocabulary::PANES, value.clone())]),
    ] {
        assert!(!can_open(Some(&root)));
        assert!(append(&root, Side::Left, value.clone()).is_none());
    }
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
    crate::annotations::set_collapsed(
        &mut workspace.document.annotations,
        &panes_path,
        false,
        false,
    );
    let root = workspace.left.panes[0].view.root.clone();
    workspace.left.panes[0].view.scroll = Vec2::new(4.0, 9.0);
    workspace.left.panes[0].view.projection = Projection::Raw;

    workspace.sync_declared(&[two, one.clone()]);
    let moved = &workspace.left.panes[1].view;
    assert_eq!(moved.root, root);
    assert_eq!(moved.scroll, Vec2::new(4.0, 9.0));
    assert_eq!(moved.projection, Projection::Raw);
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
