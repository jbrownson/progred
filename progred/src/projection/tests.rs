use super::*;
use crate::annotations::Annotations;
use crate::hover::hover_secondary;
use crate::selection::payload as selection_payload;
use gid::Position;
use progred_libraries::{absent, f64, name, text};
use ui_events::ScrollDelta;
use ui_events::keyboard::{KeyState, Modifiers};
use ui_events::pointer::{
    PointerButton, PointerButtonEvent, PointerId, PointerInfo, PointerScrollEvent, PointerState,
    PointerType, PointerUpdate,
};

#[test]
fn projection_target_appends_relative_steps() {
    let parent = gid::new_cell_id();
    let field = gid::new_cell_id();
    let hooks = Hooks::<Vec<Path>> {
        select: Rc::new(|selections, path| selections.push(path)),
        select_payload: Rc::new(|selections, path, _| selections.push(path)),
        start_edit: Rc::new(|_, _, _| {}),
        toggle: Rc::new(|_, _| {}),
        edit: Rc::new(|_| None),
        pick: Rc::new(|_, _| false),
        insert: Rc::new(|_, _| {}),
        delete: Rc::new(|_| false),
        apply: Rc::new(|_, _, _, _| false),
        point: Rc::new(|_, _, _, _, _| false),
        commit_offer: Rc::new(|_, _| {}),
    };
    let target = projection_target(&[Step::Key(parent)], &hooks, vec![Step::Key(field)]);
    assert_eq!(
        target.hover,
        Hover::Value(Rc::from(vec![Step::Key(parent), Step::Key(field)]))
    );
    let mut selections = Vec::new();
    assert!((target.select)(&mut selections));
    assert!((target.select_with)(&mut selections, Value::record([])));
    assert_eq!(
        selections,
        [
            vec![Step::Key(parent), Step::Key(field)],
            vec![Step::Key(parent), Step::Key(field)]
        ]
    );
}

fn contextual_probe(
    _: &progred_display::ProjectionInput<'_, (), Hover>,
) -> Option<progred_display::Layout<(), Hover>> {
    Some(progred_display::text("contextual"))
}

fn ambient_probe(
    _: &progred_display::ProjectionInput<'_, (), Hover>,
) -> Option<progred_display::Layout<(), Hover>> {
    Some(progred_display::text("ambient"))
}

fn declining_probe(
    _: &progred_display::ProjectionInput<'_, (), Hover>,
) -> Option<progred_display::Layout<(), Hover>> {
    None
}

#[test]
fn contextual_projection_precedes_and_falls_through_to_the_ambient_projection() {
    struct NoEval;
    impl progred_display::Env for NoEval {
        fn evaluate(&self, _: &Value) -> (Value, usize) {
            panic!("projection evaluated")
        }
    }

    let ambient = Projection::new([ambient_probe as progred_display::Partial<(), Hover>]);
    let value = Value::record([]);
    let target = |_| progred_display::ProjectionTarget {
        select: Rc::new(|_: &mut ()| false),
        select_with: Rc::new(|_: &mut (), _| false),
        hover: Hover::Value(Rc::from([])),
    };
    let apply = |projection: &Projection<()>| {
        projection
            .apply(
                &NoEval,
                &value,
                true,
                None,
                None,
                progred_display::ProjectionTargets::new(&target),
            )
            .unwrap()
    };
    let text = |layout| match layout {
        progred_display::Layout::Leaf(puri::Leaf::Text { text, .. }) => text,
        _ => panic!("probe returns text"),
    };

    assert_eq!(
        text(apply(
            &contextual_projection(
                Some(&ambient),
                Some(vec![
                    contextual_probe as progred_display::Partial<(), Hover>
                ]),
            )
            .unwrap()
        )),
        "contextual"
    );
    assert_eq!(
        text(apply(
            &contextual_projection(
                Some(&ambient),
                Some(vec![declining_probe as progred_display::Partial<(), Hover>]),
            )
            .unwrap()
        )),
        "ambient"
    );
}

fn src<'a>(doc: &'a Document, library: &'a Cells) -> Sources<'a> {
    Sources { doc, library }
}

#[test]
fn a_projection_absence_carries_its_reason_explicitly() {
    let reason = gid::new_cell_id();
    assert!(projection_is_absent(&absent::with_reason(reason)));
    assert!(!projection_is_absent(&Value::from(reason)));
}

fn make_selection(doc: &Document, library: &Cells, path: Path) -> Selection {
    Selection::edge(&src(doc, library), path)
}

/// Select through the action installed by the projected navigation
/// landmark, as the shell does after an arrow step.
fn make_projected_selection(doc: &Document, library: &Cells, path: Path) -> Selection {
    type World = Vec<(Path, progred_display::LineEdit)>;

    let stack = crate::stack::load::<World>();
    let projection_library = library.clone().merged(stack.library.clone());
    let styles = crate::styles::editor(1.0);
    let annotations = Annotations::default();
    let mut fonts = parley::FontContext::new();
    let mut layouts = parley::LayoutContext::new();
    let mut cache = puri::text::TextCache::default();
    let mut tcx = TextCtx {
        fonts: &mut fonts,
        layouts: &mut layouts,
        scale: 1.0,
        cache: &mut cache,
    };
    let measured = project::<World, crate::frame::Paint>(
        ProjectDescription {
            sources: Sources {
                doc,
                library: &projection_library,
            },
            root: doc.root.as_ref(),
            root_path: &[],
            selection: None,
            source_selection: None,
            annotations: &annotations,
            raw: false,
            styles: &styles,
            width: 500.0,
            root_projection: None,
            projection: Some(&stack.projection),
            foreign: &stack.foreign,
        },
        &mut tcx,
        Hooks {
            select: Rc::new(|_, _| {}),
            select_payload: Rc::new(|_, _, _| {}),
            start_edit: Rc::new(|selected, path, line| selected.push((path, line))),
            toggle: Rc::new(|_, _| {}),
            edit: Rc::new(|_| None),
            pick: Rc::new(|_, _| false),
            insert: Rc::new(|_, _| {}),
            delete: Rc::new(|_| false),
            apply: Rc::new(|_, _, _, _| false),
            point: Rc::new(|_, _, _, _, _| false),
            commit_offer: Rc::new(|_, _| {}),
        },
    );
    let height = measured.extent.height().max(1.0);
    let placed = measured::place(
        measured,
        Placement::root(Rect::new(0.0, 0.0, 500.0, height)),
    );
    let mut selected = World::new();
    if let Some(target) = placed
        .descends
        .iter()
        .find(|target| target.path.as_ref() == &path)
    {
        (target.select)(&mut selected);
    }
    match selected.pop() {
        Some((path, line)) => Selection::from_line(&src(doc, library), path, line),
        None => make_selection(doc, library, path),
    }
}

fn make_editing_selection(doc: &Document, library: &Cells, path: Path) -> Selection {
    struct NoEval;
    impl progred_display::Env for NoEval {
        fn evaluate(&self, _: &Value) -> (Value, usize) {
            panic!("line projection evaluated")
        }
    }

    let value = src(doc, library).resolve(&path).expect("selected value");
    let stack = crate::stack::load::<()>();
    let layout = {
        let target = |_| progred_display::ProjectionTarget {
            select: Rc::new(|_: &mut ()| false),
            select_with: Rc::new(|_: &mut (), _| false),
            hover: Hover::Value(Rc::from(path.clone())),
        };
        stack.projection.apply(
            &NoEval,
            value,
            true,
            None,
            None,
            progred_display::ProjectionTargets::new(&target),
        )
    }
    .expect("value projection");
    // A scrubbable number wraps its line in the scrub claim; the
    // line itself remains the editable value.
    let layout = match layout {
        progred_display::Layout::OnScrub { child, .. } => *child,
        layout => layout,
    };
    let progred_display::Layout::LineEdit(line) = layout else {
        panic!("value is not line editable")
    };
    Selection::from_line(&src(doc, library), path, line)
}

fn toggle_fold(sources: &Sources, collapse: &mut Annotations, path: &[Step]) -> bool {
    toggle_collapse(sources, collapse, path)
}

fn set_fold(sources: &Sources, collapse: &mut Annotations, path: &[Step], closed: bool) -> bool {
    set_collapse(sources, collapse, path, closed)
}

fn key(s: &str) -> Step {
    Step::Key(crate::test_values::label(s))
}

/// A one-cell document: the root links a cell holding `fields`.
fn doc_of(fields: Vec<(CellId, Value)>) -> (Document, CellId) {
    let mut cells = Cells::new();
    let cell = new_cell_id();
    cells.set_value(cell, Value::record(fields));
    (
        Document {
            root: Some(Value::from(cell)),
            cells,
        },
        cell,
    )
}

/// The ordered positions of a list value's elements.
fn positions(value: &Value) -> Vec<Position> {
    value.as_list().unwrap().keys().cloned().collect()
}

const LINE: f64 = 16.0;

fn stop(path: Vec<Step>, x0: f64, y0: f64, x1: f64, y1: f64) -> Descend<()> {
    Descend {
        root: None,
        path: Rc::from(path),
        rect: Rect::new(x0, y0, x1, y1),
        select: Rc::new(|_| true),
    }
}

fn arrow(named: NamedKey) -> KeyboardEvent {
    KeyboardEvent {
        key: Key::Named(named),
        state: KeyState::Down,
        modifiers: Modifiers::empty(),
        ..Default::default()
    }
}

fn stepped(ds: &[Descend<()>], from: Option<Vec<Step>>, named: NamedKey) -> Option<Path> {
    let selection = from.map(crate::selection::bare_edge);
    step_selection(ds, None, selection.as_ref(), LINE, &arrow(named))
        .map(|descend| descend.path.to_vec())
}

#[test]
fn arrows_walk_rows_down_and_lines_across() {
    // A block record: field `a` hugs a flat record on line one,
    // field `b` drops a two-row block, field `e` closes. Settled
    // in placement order — children before parents.
    let a = || vec![key("a")];
    let a1 = || vec![key("a"), key("p")];
    let a2 = || vec![key("a"), key("q")];
    let b = || vec![key("b")];
    let c = || vec![key("b"), key("c")];
    let d = || vec![key("b"), key("d")];
    let e = || vec![key("e")];
    let ds = vec![
        stop(a1(), 60.0, 2.0, 80.0, 18.0),
        stop(a2(), 100.0, 2.0, 120.0, 18.0),
        stop(a(), 40.0, 2.0, 140.0, 18.0),
        stop(c(), 60.0, 24.0, 80.0, 40.0),
        stop(d(), 60.0, 44.0, 80.0, 60.0),
        stop(b(), 20.0, 22.0, 280.0, 62.0),
        stop(e(), 40.0, 66.0, 80.0, 82.0),
        stop(vec![], 0.0, 0.0, 300.0, 84.0),
    ];
    // Nothing selected: any arrow lands on the root.
    assert_eq!(stepped(&ds, None, NamedKey::ArrowDown), Some(vec![]));
    // Down walks every row in reading order, entering the open
    // block; up reverses it exactly.
    let rows = [vec![], a(), b(), c(), d(), e()];
    for pair in rows.windows(2) {
        let (above, below) = (&pair[0], &pair[1]);
        assert_eq!(
            stepped(&ds, Some(above.clone()), NamedKey::ArrowDown),
            Some(below.clone())
        );
        assert_eq!(
            stepped(&ds, Some(below.clone()), NamedKey::ArrowUp),
            Some(above.clone())
        );
    }
    assert_eq!(stepped(&ds, Some(e()), NamedKey::ArrowDown), None);
    assert_eq!(stepped(&ds, Some(vec![]), NamedKey::ArrowUp), None);
    // Right walks the hugged line's content; the walk ends with
    // the line, and left retraces it back out to the row.
    assert_eq!(stepped(&ds, Some(a()), NamedKey::ArrowRight), Some(a1()));
    assert_eq!(stepped(&ds, Some(a1()), NamedKey::ArrowRight), Some(a2()));
    assert_eq!(stepped(&ds, Some(a2()), NamedKey::ArrowRight), None);
    assert_eq!(stepped(&ds, Some(a2()), NamedKey::ArrowLeft), Some(a1()));
    assert_eq!(stepped(&ds, Some(a1()), NamedKey::ArrowLeft), Some(a()));
    // Left from a row widens to the parent; the root has none.
    assert_eq!(stepped(&ds, Some(a()), NamedKey::ArrowLeft), Some(vec![]));
    assert_eq!(stepped(&ds, Some(vec![]), NamedKey::ArrowLeft), None);
    // Mid-line, down exits to the next row and up collects to the
    // line's own stop.
    assert_eq!(stepped(&ds, Some(a2()), NamedKey::ArrowDown), Some(b()));
    assert_eq!(stepped(&ds, Some(a1()), NamedKey::ArrowUp), Some(a()));
    // A dropped block is entered by down, never right.
    assert_eq!(stepped(&ds, Some(b()), NamedKey::ArrowRight), None);
}

#[test]
fn navigation_declines_modified_keys_releases_and_other_keys() {
    let ds = vec![
        stop(vec![key("a")], 0.0, 2.0, 60.0, 18.0),
        stop(vec![], 0.0, 0.0, 300.0, 40.0),
    ];
    let shifted = KeyboardEvent {
        modifiers: Modifiers::SHIFT,
        ..arrow(NamedKey::ArrowDown)
    };
    assert!(step_selection(&ds, None, None, LINE, &shifted).is_none());
    let released = KeyboardEvent {
        state: KeyState::Up,
        ..arrow(NamedKey::ArrowDown)
    };
    assert!(step_selection(&ds, None, None, LINE, &released).is_none());
    assert!(step_selection(&ds, None, None, LINE, &arrow(NamedKey::Escape)).is_none());
}

#[test]
fn set_collapse_is_directional_and_stays_sparse() {
    let lib = Cells::new();
    let (doc, _) = doc_of(vec![(
        crate::test_values::label("a"),
        crate::test_values::text("1"),
    )]);
    let sources = src(&doc, &lib);
    let mut collapse = Annotations::default();
    assert!(set_fold(&sources, &mut collapse, &[], true));
    assert!(!set_fold(&sources, &mut collapse, &[], true));
    assert!(set_fold(&sources, &mut collapse, &[], false));
    assert!(!set_fold(&sources, &mut collapse, &[], false));
    // Matching the default stores nothing.
    assert!(collapse.at(&[]).is_none());
    // A leaf has nothing to fold.
    let leaf = vec![Step::Follow, key("a")];
    assert!(!set_fold(&sources, &mut collapse, &leaf, true));
}

#[test]
fn line_projection_descriptions_mount_the_rust_editor() {
    let lib = Cells::new();
    let (mut doc, cell) = doc_of(vec![
        (
            crate::test_values::label("name"),
            crate::test_values::text("old"),
        ),
        (
            crate::test_values::label("x"),
            crate::test_values::text("1.5"),
        ),
    ]);
    let edit = |doc: &Document, path: Vec<Step>| make_editing_selection(doc, &lib, path);
    assert_eq!(
        edit(&doc, vec![Step::Follow, key("name")])
            .edit()
            .map(LineEditState::text),
        Some("old")
    );
    assert_eq!(
        edit(&doc, vec![Step::Follow, key("x")])
            .edit()
            .map(LineEditState::text),
        Some("1.5")
    );
    // Missing fields, links, and blobs carry no editor.
    assert!(
        make_selection(&doc, &lib, vec![Step::Follow, key("missing")])
            .edit()
            .is_none()
    );
    assert!(make_selection(&doc, &lib, vec![]).edit().is_none());
    doc.cells.set_value(
        cell,
        Value::record([(crate::test_values::label("b"), Value::from(vec![0xff_u8]))]),
    );
    assert!(
        make_selection(&doc, &lib, vec![Step::Follow, key("b")])
            .edit()
            .is_none()
    );
    // A cell holding text edits at its Follow path.
    doc.cells.set_value(cell, crate::test_values::text("held"));
    assert_eq!(
        edit(&doc, vec![Step::Follow])
            .edit()
            .map(LineEditState::text),
        Some("held")
    );
    // A simple name convention is just another text field.
    doc.cells.set_value(cell, name::record("roof", []));
    assert_eq!(
        edit(&doc, vec![Step::Follow, Step::Key(name::vocabulary::NAME),],)
            .edit()
            .map(LineEditState::text),
        Some("roof")
    );
}

#[test]
fn a_line_control_installs_its_navigation_selection() {
    let lib = Cells::new();
    let (doc, _) = doc_of(vec![(
        crate::test_values::label("name"),
        crate::test_values::text("old"),
    )]);
    let selected = make_projected_selection(&doc, &lib, vec![Step::Follow, key("name")]);
    assert_eq!(selected.edit().map(LineEditState::text), Some("old"));
}

#[test]
fn edits_write_through_to_the_field() {
    let lib = Cells::new();
    let (mut doc, _) = doc_of(vec![(
        crate::test_values::label("name"),
        crate::test_values::text("old"),
    )]);
    let path = vec![Step::Follow, key("name")];
    let mut selection = make_editing_selection(&doc, &lib, path.clone());
    selection.edit_mut().unwrap().set_text("new");
    write_through(
        &mut doc,
        &lib,
        &crate::stack::load::<()>().foreign,
        &mut selection,
    );
    assert_eq!(
        src(&doc, &lib).resolve(&path),
        Some(&crate::test_values::text("new"))
    );
    // A selection without an editor writes nothing.
    let mut plain = make_selection(&doc, &lib, vec![Step::Follow, key("missing")]);
    assert!(!write_through(
        &mut doc,
        &lib,
        &crate::stack::load::<()>().foreign,
        &mut plain
    ));
    assert_eq!(
        src(&doc, &lib).resolve(&path),
        Some(&crate::test_values::text("new"))
    );
}

#[test]
fn compact_f64_values_edit_as_decimal_text() {
    let lib = Cells::new();
    let cell = new_cell_id();
    let mut cells = Cells::new();
    cells.set_value(cell, f64::value(2.5));
    let mut doc = Document {
        root: Some(Value::from(cell)),
        cells,
    };
    let path = vec![Step::Follow];
    let mut selection = make_editing_selection(&doc, &lib, path.clone());
    assert_eq!(selection.edit().map(LineEditState::text), Some("2.5"));
    selection.edit_mut().unwrap().set_text("7.25");
    assert!(write_through(
        &mut doc,
        &lib,
        &crate::stack::load::<()>().foreign,
        &mut selection
    ));
    assert_eq!(
        src(&doc, &lib).resolve(&path).and_then(f64::read),
        Some(7.25)
    );

    selection.edit_mut().unwrap().set_text("not a number");
    assert!(!write_through(
        &mut doc,
        &lib,
        &crate::stack::load::<()>().foreign,
        &mut selection
    ));
    assert_eq!(
        src(&doc, &lib).resolve(&path).and_then(f64::read),
        Some(7.25)
    );
}

#[test]
fn editing_an_f64_keeps_unrelated_fields() {
    let lib = Cells::new();
    let cell = new_cell_id();
    let unit = crate::test_values::label("unit");
    let mut cells = Cells::new();
    cells.set_value(
        cell,
        Value::record(
            f64::value(2.5)
                .as_record()
                .unwrap()
                .clone()
                .update(unit, crate::test_values::text("mm")),
        ),
    );
    let mut doc = Document {
        root: Some(Value::from(cell)),
        cells,
    };
    let path = vec![Step::Follow];
    let mut selection = make_editing_selection(&doc, &lib, path.clone());
    selection.edit_mut().unwrap().set_text("8");
    assert!(write_through(
        &mut doc,
        &lib,
        &crate::stack::load::<()>().foreign,
        &mut selection
    ));
    let value = src(&doc, &lib).resolve(&path).unwrap();
    assert_eq!(f64::read(value), Some(8.0));
    assert_eq!(
        value
            .as_record()
            .and_then(|fields| fields.get(&unit))
            .and_then(text::read),
        Some("mm")
    );
}

#[test]
fn element_edits_rebuild_the_list_at_the_owning_cell() {
    let lib = Cells::new();
    let (mut doc, _) = doc_of(vec![(
        crate::test_values::label("dash"),
        Value::list([crate::test_values::text("2"), crate::test_values::text("3")]),
    )]);
    let list_path = vec![Step::Follow, key("dash")];
    let ps = positions(src(&doc, &lib).resolve(&list_path).unwrap());
    let element = vec![Step::Follow, key("dash"), Step::Element(ps[1].clone())];

    // Editing an element writes the whole rebuilt list at the
    // owning cell; the sibling keeps its position and value.
    let mut selection = make_editing_selection(&doc, &lib, element.clone());
    selection.edit_mut().unwrap().set_text("9");
    assert!(write_through(
        &mut doc,
        &lib,
        &crate::stack::load::<()>().foreign,
        &mut selection
    ));
    assert_eq!(
        src(&doc, &lib).resolve(&element),
        Some(&crate::test_values::text("9"))
    );
    assert_eq!(
        src(&doc, &lib).resolve(&list_path),
        Some(&Value::list([
            crate::test_values::text("2"),
            crate::test_values::text("9")
        ]))
    );
    assert_eq!(positions(src(&doc, &lib).resolve(&list_path).unwrap()), ps);
}

#[test]
fn set_value_writes_fields_elements_roots_and_bare_cells() {
    let lib = Cells::new();
    let (mut doc, cell) = doc_of(vec![(
        crate::test_values::label("x"),
        crate::test_values::text("1"),
    )]);
    assert!(set_value(
        &mut doc,
        &lib,
        &[Step::Follow, key("x")],
        crate::test_values::text("2")
    ));
    assert_eq!(
        src(&doc, &lib).resolve(&[Step::Follow, key("x")]),
        Some(&crate::test_values::text("2"))
    );

    // A fresh Key step INSERTS a field; a deep spine rebuilds
    // through nested records and lists.
    assert!(set_value(
        &mut doc,
        &lib,
        &[Step::Follow, key("at")],
        Value::record([(
            crate::test_values::label("row"),
            crate::test_values::text("top")
        )]),
    ));
    assert!(set_value(
        &mut doc,
        &lib,
        &[Step::Follow, key("at"), key("row")],
        crate::test_values::text("bottom")
    ));
    assert_eq!(
        src(&doc, &lib).resolve(&[Step::Follow, key("at"), key("row")]),
        Some(&crate::test_values::text("bottom"))
    );

    // The whole cell value is addressable at Follow: conversion
    // is one set.
    assert!(set_value(
        &mut doc,
        &lib,
        &[Step::Follow],
        Value::list([crate::test_values::text("a")])
    ));
    assert_eq!(
        src(&doc, &lib).resolve(&[Step::Follow]),
        Some(&Value::list([crate::test_values::text("a")]))
    );

    // A bare cell takes its first value through the empty spine;
    // deeper steps into nothing decline.
    let bare = new_cell_id();
    doc.root = Some(Value::from(bare));
    assert!(!set_value(
        &mut doc,
        &lib,
        &[Step::Follow, key("x")],
        crate::test_values::text("v")
    ));
    assert!(set_value(
        &mut doc,
        &lib,
        &[Step::Follow],
        Value::record([(
            crate::test_values::label("x"),
            crate::test_values::text("v")
        )])
    ));
    assert_eq!(
        src(&doc, &lib).resolve(&[Step::Follow, key("x")]),
        Some(&crate::test_values::text("v"))
    );

    // An inline record at the root writes on the root spine — no
    // cell involved.
    doc.root = Some(Value::record([(
        crate::test_values::label("shape"),
        Value::from(cell),
    )]));
    assert!(set_value(
        &mut doc,
        &lib,
        &[key("title")],
        crate::test_values::text("scene")
    ));
    assert_eq!(
        src(&doc, &lib).resolve(&[key("title")]),
        Some(&crate::test_values::text("scene"))
    );
    assert!(set_value(
        &mut doc,
        &lib,
        &[],
        crate::test_values::text("root")
    ));
    assert_eq!(doc.root, Some(crate::test_values::text("root")));
    // Text is a record convention, so a structural write can
    // enrich it. The text facet remains, and so does the extra
    // field.
    assert!(set_value(
        &mut doc,
        &lib,
        &[key("x")],
        crate::test_values::text("0")
    ));
    assert_eq!(
        doc.root
            .as_ref()
            .and_then(Value::as_record)
            .and_then(|fields| fields.get(&crate::test_values::label("x"))),
        Some(&crate::test_values::text("0"))
    );
    assert!(text::read(doc.root.as_ref().unwrap()).is_some());
}

#[test]
fn external_cells_decline_writes_and_bare_cells_accept() {
    let mut lib = Cells::new();
    let lib_cell = new_cell_id();
    lib.set_value(
        lib_cell,
        name::record(
            "convention",
            [(
                crate::test_values::label("a"),
                crate::test_values::text("1"),
            )],
        ),
    );
    let mut doc = Document {
        root: Some(Value::from(lib_cell)),
        cells: Cells::new(),
    };
    // The library's cell declines writes wholesale.
    assert!(!set_value(
        &mut doc,
        &lib,
        &[Step::Follow, key("a")],
        crate::test_values::text("2")
    ));
    assert!(!set_value(
        &mut doc,
        &lib,
        &[Step::Follow, Step::Key(name::vocabulary::NAME),],
        crate::test_values::text("mine")
    ));
    assert!(!delete_edge(&mut doc, &lib, &[Step::Follow, key("a")]));
    assert!(!delete_edge(&mut doc, &lib, &[Step::Follow]));
    // Forking — the document taking the cell over — writes.
    doc.cells.set_value(
        lib_cell,
        Value::record([(
            crate::test_values::label("a"),
            crate::test_values::text("1"),
        )]),
    );
    assert!(set_value(
        &mut doc,
        &lib,
        &[Step::Follow, key("a")],
        crate::test_values::text("2")
    ));
}

#[test]
fn write_through_opens_one_step_per_editor_life() {
    let lib = Cells::new();
    let (mut doc, _) = doc_of(vec![(
        crate::test_values::label("name"),
        crate::test_values::text("a"),
    )]);
    let path = vec![Step::Follow, key("name")];
    let mut selection = make_editing_selection(&doc, &lib, path);

    // First write opens the step; the rest of the run is silent,
    // as are no-op rewrites.
    selection.edit_mut().unwrap().set_text("ab");
    assert!(write_through(
        &mut doc,
        &lib,
        &crate::stack::load::<()>().foreign,
        &mut selection
    ));
    selection.edit_mut().unwrap().set_text("abc");
    assert!(!write_through(
        &mut doc,
        &lib,
        &crate::stack::load::<()>().foreign,
        &mut selection
    ));
    assert!(!write_through(
        &mut doc,
        &lib,
        &crate::stack::load::<()>().foreign,
        &mut selection
    ));

    // Breaking the run (a save) makes the next write a new step.
    break_edit_run(Some(&mut selection));
    selection.edit_mut().unwrap().set_text("abcd");
    assert!(write_through(
        &mut doc,
        &lib,
        &crate::stack::load::<()>().foreign,
        &mut selection
    ));

    // A re-minted editor is a new run by construction.
    let mut fresh = make_editing_selection(&doc, &lib, vec![Step::Follow, key("name")]);
    fresh.edit_mut().unwrap().set_text("x");
    assert!(write_through(
        &mut doc,
        &lib,
        &crate::stack::load::<()>().foreign,
        &mut fresh
    ));
}

#[test]
fn delete_unlinks_fields_and_elements_and_bares_cells() {
    let lib = Cells::new();
    let child = new_cell_id();
    let (mut doc, cell) = doc_of(vec![
        (crate::test_values::label("child"), Value::from(child)),
        (
            crate::test_values::label("dash"),
            Value::list([crate::test_values::text("2"), crate::test_values::text("3")]),
        ),
    ]);
    doc.cells.set_value(child, name::record("c", []));

    assert!(!delete_edge(
        &mut doc,
        &lib,
        &[Step::Follow, key("missing")]
    ));

    // Unlinking a field drops the link; the linked cell floats in
    // the table for the orphan pool.
    assert!(delete_edge(&mut doc, &lib, &[Step::Follow, key("child")]));
    assert_eq!(src(&doc, &lib).resolve(&[Step::Follow, key("child")]), None);
    assert!(doc.cells.value(child).is_some());

    // An element step rebuilds the list without it.
    let dash = vec![Step::Follow, key("dash")];
    let ps = positions(src(&doc, &lib).resolve(&dash).unwrap());
    assert!(delete_edge(
        &mut doc,
        &lib,
        &[Step::Follow, key("dash"), Step::Element(ps[0].clone())]
    ));
    assert_eq!(
        src(&doc, &lib).resolve(&dash),
        Some(&Value::list([crate::test_values::text("3")]))
    );

    // A trailing Follow removes the cell's value: valueless
    // again, and a second delete declines.
    assert!(delete_edge(&mut doc, &lib, &[Step::Follow]));
    assert!(doc.cells.value(cell).is_none());
    assert!(src(&doc, &lib).resolve(&[]).is_some());
    assert!(!delete_edge(&mut doc, &lib, &[Step::Follow]));

    // The empty path empties the document.
    assert!(delete_edge(&mut doc, &lib, &[]));
    assert!(doc.root.is_none());
    assert!(!delete_edge(&mut doc, &lib, &[]));
}

#[test]
fn pendings_normalize_through_links_and_gate_on_authority() {
    let mut lib = Cells::new();
    let lib_cell = new_cell_id();
    lib.set_value(
        lib_cell,
        Value::record([(
            crate::test_values::label("a"),
            crate::test_values::text("1"),
        )]),
    );
    let bare = new_cell_id();
    let (mut doc, _) = doc_of(vec![
        (crate::test_values::label("at"), Value::record([])),
        (
            crate::test_values::label("tags"),
            Value::list([crate::test_values::text("x")]),
        ),
        (crate::test_values::label("lib"), Value::from(lib_cell)),
        (crate::test_values::label("material"), Value::from(bare)),
        (
            crate::test_values::label("s"),
            crate::test_values::text("leaf"),
        ),
    ]);
    doc.root = doc.root.clone();
    let sources = src(&doc, &lib);

    // A link to a record cell pends its field under Follow; an
    // inline record pends at its own path.
    let on_cell = pending_edge(&sources, vec![]).unwrap();
    assert_eq!(on_cell.path(), &[Step::Follow]);
    let inline = pending_edge(&sources, vec![Step::Follow, key("at")]).unwrap();
    assert_eq!(inline.path(), &[Step::Follow, key("at")]);

    // Lists, external cells, and bare cells decline fields. Text
    // is a record convention, so adding a field enriches it and
    // makes its structure visible.
    assert!(pending_edge(&sources, vec![Step::Follow, key("tags")]).is_none());
    assert!(pending_edge(&sources, vec![Step::Follow, key("s")]).is_some());
    assert!(pending_edge(&sources, vec![Step::Follow, key("lib")]).is_none());
    assert!(pending_edge(&sources, vec![Step::Follow, key("material")]).is_none());

    // A bare cell pends its first value at Follow — the
    // within-gesture's meaning there.
    let filling = pending_follow(&sources, &[Step::Follow, key("material")]).unwrap();
    assert_eq!(
        filling.path(),
        &[Step::Follow, key("material"), Step::Follow]
    );
    assert!(pending_follow(&sources, &[Step::Follow, key("lib")]).is_none());
    assert!(pending_follow(&sources, &[Step::Follow, key("at")]).is_none());

    // Into a list through its link, appended at the end.
    let into = pending_into(&sources, &[Step::Follow, key("tags")]).unwrap();
    assert!(matches!(into.path().last(), Some(Step::Element(_))));
    assert_eq!(into.path().len(), 3);

    // The within chord: fields on records, elements into lists,
    // first values into bare cells.
    assert!(pending_insert(&sources, &[], false).is_some());
    assert!(pending_insert(&sources, &[Step::Follow, key("tags")], false).is_some());
    assert!(pending_insert(&sources, &[Step::Follow, key("material")], false).is_some());
    assert!(pending_insert(&sources, &[Step::Follow, key("s")], false).is_some());
}

#[test]
fn queries_resolve_text_and_blobs() {
    assert_eq!(resolve_query("hello"), crate::test_values::text("hello"));
    assert_eq!(
        resolve_query("\"quoted\""),
        crate::test_values::text("quoted")
    );
    assert_eq!(resolve_query("\"open"), crate::test_values::text("open"));
    assert_eq!(resolve_query("\"0xff\""), crate::test_values::text("0xff"));
    assert_eq!(resolve_query("0xff00"), Value::from(vec![0xff, 0x00]));
    // Input is case-tolerant — the value is the bytes, lowercase
    // just the canonical spelling — and whole bytes only.
    assert_eq!(resolve_query("0xDEad"), Value::from(vec![0xde, 0xad]));
    assert_eq!(resolve_query("0xf"), crate::test_values::text("0xf"));
    assert_eq!(resolve_query("0x"), Value::from(vec![]));
}

#[test]
fn clipboard_spellings_round_trip() {
    let cell = new_cell_id();
    // Atoms are text and round-trip through it; structure rides
    // the private format and round-trips through its bytes.
    let atoms = [
        crate::test_values::text("plain"),
        crate::test_values::text("\"tricky\""),
        Value::from(vec![0xde, 0xad]),
    ];
    for value in atoms {
        let (text, structural) = to_clipboard(&value);
        assert!(!structural);
        assert_eq!(from_clipboard(&text), value);
    }
    let structures = [
        Value::from(cell),
        Value::list([crate::test_values::text("a"), Value::from(cell)]),
        Value::record([(
            crate::test_values::label("x"),
            crate::test_values::text("1"),
        )]),
        Value::record([(cell, Value::from(vec![0x00_u8]))]),
    ];
    for value in structures {
        let (text, structural) = to_clipboard(&value);
        assert!(structural);
        assert_eq!(from_structure(text.as_bytes()), Some(value));
    }
    // Atoms read in other apps; alien text pastes sensibly — and
    // TEXT IS NEVER STRUCTURE: characters that happen to spell
    // Value JSON read as the string they are.
    assert_eq!(to_clipboard(&crate::test_values::text("hi")).0, "\"hi\"");
    assert_eq!(to_clipboard(&Value::from(vec![0xff_u8])).0, "0xff");
    assert_eq!(
        from_clipboard("loose text"),
        crate::test_values::text("loose text")
    );
    let spelled = to_clipboard(&Value::record([])).0;
    assert_eq!(from_clipboard(&spelled), crate::test_values::text(&spelled));
}

#[test]
fn completion_offers_follow_the_stage() {
    let lib = crate::stack::load::<()>().library;
    let (mut doc, cell) = doc_of(vec![
        name::field("roof"),
        (
            crate::test_values::label("kind"),
            crate::test_values::text("building"),
        ),
    ]);
    let sources = src(&doc, &lib);
    let displays = |labels: bool, query: &str| -> Vec<String> {
        completion_entries(&sources, false, labels, query)
            .into_iter()
            .map(|entry| entry.display)
            .collect()
    };

    // The value stage offers the string, references, the value
    // constructors, and the mint.
    let value_stage = displays(false, "");
    assert!(value_stage.iter().any(|d| d == "roof"));
    assert!(
        value_stage.iter().any(|d| d == "new list"),
        "{value_stage:?}"
    );
    assert!(value_stage.iter().any(|d| d == "new record"));
    assert!(value_stage.iter().any(|d| d == "new cell"));

    // The label stage narrows to what can label: no list, record,
    // or blob offers.
    let label_stage = displays(true, "");
    assert!(label_stage.iter().all(|d| d != "new list"));
    assert!(label_stage.iter().all(|d| d != "new record"));
    assert!(label_stage.iter().any(|d| d == "new cell"));
    let label_blob = completion_entries(&sources, false, true, "0xff");
    assert_eq!(label_blob[0].display, "0xff");
    assert_eq!(label_blob[0].detail.as_deref(), Some("new label"));
    assert!(matches!(
        &label_blob[0].action,
        EntryAction::NewLabel(name) if name == "0xff"
    ));

    // A blob query leads with the blob, its string form below.
    let value_blob = displays(false, "0xff");
    assert_eq!(value_blob[0], "0xff");
    assert_eq!(value_blob[1], "\"0xff\"");

    // Reference commits are links.
    let roof = completion_entries(&sources, false, false, "roof");
    assert!(matches!(
        &roof[0].action,
        EntryAction::Value(value) if value.as_cell() == Some(cell)
    ));
    let add = completion_entries(&sources, false, false, "+");
    assert!(matches!(
        &add[0].action,
        EntryAction::Value(value)
            if value.as_cell() == Some(f64::vocabulary::ADD)
    ));

    // A bare id never outranks the typed text: the string the
    // query spells comes before every unnamed reference, however
    // exactly the id matches — ids are for reading; want it
    // reachable, name it.
    let unnamed = new_cell_id();
    doc.cells.set_value(unnamed, crate::test_values::text("x"));
    let sources = src(&doc, &lib);
    let entries = completion_entries(&sources, false, false, &short_id(unnamed));
    let atom = entries
        .iter()
        .position(|e| matches!(&e.action, EntryAction::Value(v) if text::read(v).is_some()))
        .unwrap();
    let reference = entries
        .iter()
        .position(|e| matches!(&e.action, EntryAction::Value(v) if v.as_cell() == Some(unnamed)))
        .unwrap();
    assert!(atom < reference);
}

#[test]
fn cycles_collapse_by_default_and_expand_turn_by_turn() {
    // A: { next: A } — the re-entry at [Follow, next] repeats the
    // root value.
    let a = new_cell_id();
    let mut cells = Cells::new();
    cells.set_value(
        a,
        Value::record([(crate::test_values::label("next"), Value::from(a))]),
    );
    let doc = Document {
        root: Some(Value::from(a)),
        cells,
    };
    let lib = Cells::new();
    let sources = src(&doc, &lib);
    let mut collapse = Annotations::default();
    let reentry = vec![Step::Follow, key("next")];
    // Space's toggle expands the default-collapsed re-entry.
    assert!(toggle_fold(&sources, &mut collapse, &reentry));
    assert!(!crate::annotations::collapsed(&collapse, &reentry, true));
    // The next turn defaults collapsed at its own deeper path and
    // expands the same way — follow the cycle as far as wanted.
    let deeper: Vec<Step> = reentry.iter().chain(reentry.iter()).cloned().collect();
    assert!(crate::annotations::collapsed(&collapse, &deeper, true));
    assert!(toggle_fold(&sources, &mut collapse, &deeper));
    assert!(!crate::annotations::collapsed(&collapse, &deeper, true));
    // Toggling back restores the default (the override is sparse).
    assert!(toggle_fold(&sources, &mut collapse, &deeper));
    assert!(crate::annotations::collapsed(&collapse, &deeper, true));
    assert!(collapse.at(&deeper).is_none());
}

#[test]
fn any_valued_cell_and_any_container_collapse() {
    let lib = Cells::new();
    let (doc, _) = doc_of(vec![(
        crate::test_values::label("kind"),
        crate::test_values::text("building"),
    )]);
    let sources = src(&doc, &lib);
    let mut collapse = Annotations::default();
    // A plain (non-cycle) cell collapses to ( … ) via the same
    // toggle.
    assert!(toggle_fold(&sources, &mut collapse, &[]));
    assert!(crate::annotations::collapsed(&collapse, &[], false));
    // Its record collapses too — layout never enters into it, so
    // inline literals toggle exactly like block forms.
    assert!(toggle_fold(&sources, &mut collapse, &[Step::Follow]));
    assert!(crate::annotations::collapsed(
        &collapse,
        &[Step::Follow],
        false
    ));
    // A valueless location declines.
    let empty = Document {
        root: None,
        cells: Cells::new(),
    };
    assert!(!toggle_fold(
        &src(&empty, &lib),
        &mut collapse,
        &[] as &[Step]
    ));
}

#[test]
fn minting_seeds_bare_and_named_cells() {
    // A mint is fully bare: a link with nothing said at all —
    // naming happens on the head afterward.
    let bare = resolve_entry(&EntryAction::NewCell);
    assert!(bare.unwrap().as_cell().is_some());
    // The value constructors commit pure values — nothing minted.
    assert_eq!(resolve_entry(&EntryAction::NewList), Some(Value::list([])));
    assert_eq!(
        resolve_entry(&EntryAction::NewRecord),
        Some(Value::record([]))
    );

    let (label, created) = resolve_label(&EntryAction::NewLabel("asdf".to_string())).unwrap();
    let (cell, value) = created.unwrap();
    assert_eq!(label, cell);
    assert_eq!(name::read(&value), Some("asdf"));
    assert!(resolve_label(&EntryAction::Value(crate::test_values::text("no"))).is_none());
}

#[test]
fn entry_hover_marks_follow_the_live_query() {
    // The reported bug: hover an entry, keep the mouse still,
    // type — the mark must follow what the entry NOW is, not
    // what it was when the pointer arrived.
    let doc = sample_document();
    let lib = crate::stack::load::<()>().library;
    let sources = src(&doc, &lib);
    let pending = |text: &str| crate::selection::pending_with_query(Vec::new(), text);
    // A quoted query leads with its typed atom, but marks mean
    // IDENTITY: an equal text value is a copy, not the same cell,
    // so string entries mark nothing.
    assert_eq!(
        hover_secondary(&sources, false, Some(&pending("\"a\"")), &Hover::Entry(0)),
        None
    );
    // A closed pending answers nothing.
    assert_eq!(
        hover_secondary(&sources, false, None, &Hover::Entry(0)),
        None
    );
}

#[test]
fn the_sample_document_shows_the_constructs() {
    let doc = sample_document();
    let lib = crate::stack::load::<()>().library;
    let sources = src(&doc, &lib);
    // The root is an inline record of roles.
    assert!(doc.root.as_ref().unwrap().as_record().is_some());
    let roof = sources.resolve(&[key("shape")]).unwrap().as_cell().unwrap();
    assert_eq!(sources.value(roof).and_then(name::read), Some("roof"));
    // The material cell is referenced and fully bare.
    let material = sources
        .resolve(&[key("shape"), Step::Follow, key("material")])
        .unwrap()
        .as_cell()
        .unwrap();
    assert!(sources.value(material).is_none());
    // The stroke cell is a name-only ordinary record, referenced
    // as a label.
    let stroke = sources
        .value(roof)
        .unwrap()
        .as_record()
        .unwrap()
        .keys()
        .copied()
        .find(|cell| sources.value(*cell).and_then(name::read) == Some("stroke"))
        .unwrap();
    assert_eq!(sources.value(stroke).and_then(name::read), Some("stroke"));
    // The style cell is shared by the root and the roof.
    assert_eq!(
        sources.resolve(&[key("style")]),
        sources.resolve(&[key("shape"), Step::Follow, key("style")])
    );
    // Points hold inline records; the swatch is a blob.
    let points = sources
        .resolve(&[key("shape"), Step::Follow, key("points")])
        .unwrap();
    let origin = points
        .as_list()
        .unwrap()
        .values()
        .next()
        .unwrap()
        .as_cell()
        .unwrap();
    assert!(matches!(
        sources
            .value(origin)
            .and_then(|value| value.as_record())
            .and_then(|fields| fields.get(&crate::test_values::label("at"))),
        Some(Value::Record(_))
    ));
    assert!(
        sources
            .resolve(&[key("style"), Step::Follow, key("swatch")])
            .unwrap()
            .as_blob()
            .is_some()
    );
    // Documents round trip with convention data unchanged.
    let json = serde_json::to_string(&doc).unwrap();
    let loaded: Document = serde_json::from_str(&json).unwrap();
    assert_eq!(loaded.root, doc.root);
    assert_eq!(loaded.cells.value(roof).and_then(name::read), Some("roof"));
    assert_eq!(serde_json::to_string(&loaded).unwrap(), json);
}

#[test]
fn selecting_an_empty_value_slot_pends() {
    let mut lib = Cells::new();
    let bare = new_cell_id();
    let mut doc = Document {
        root: Some(Value::from(bare)),
        cells: Cells::new(),
    };
    // A writable valueless cell's Follow slot is already
    // authoring: selecting it (the rendered placeholder) pends.
    assert_eq!(
        make_selection(&doc, &lib, vec![Step::Follow]).stage(),
        crate::selection::Stage::Pending
    );
    // Valued, it selects normally.
    doc.cells.set_value(bare, crate::test_values::text("v"));
    assert_eq!(
        make_selection(&doc, &lib, vec![Step::Follow]).stage(),
        crate::selection::Stage::Edge
    );
    // An EXTERNAL cell has an ordinary value, so its Follow slot
    // selects normally and remains unwritable.
    let lib_cell = new_cell_id();
    lib.set_value(lib_cell, name::record("convention", []));
    doc.root = Some(Value::from(lib_cell));
    let external = make_selection(&doc, &lib, vec![Step::Follow]);
    assert_eq!(external.stage(), crate::selection::Stage::Edge);
    assert!(external.edit().is_none());
    // The empty document's root is the same rule.
    let empty = Document {
        root: None,
        cells: Cells::new(),
    };
    assert_eq!(
        make_selection(&empty, &lib, vec![]).stage(),
        crate::selection::Stage::Pending
    );
}

#[test]
fn a_simple_name_is_an_ordinary_editable_field() {
    let lib = Cells::new();
    let (mut doc, cell) = doc_of(vec![
        name::field("old"),
        (
            crate::test_values::label("x"),
            crate::test_values::text("1"),
        ),
    ]);
    let path = vec![Step::Follow, Step::Key(name::vocabulary::NAME)];

    let mut selection = make_editing_selection(&doc, &lib, path.clone());
    selection.edit_mut().unwrap().set_text("new");
    assert!(write_through(
        &mut doc,
        &lib,
        &crate::stack::load::<()>().foreign,
        &mut selection
    ));
    assert_eq!(doc.cells.value(cell).and_then(name::read), Some("new"));
    assert!(!write_through(
        &mut doc,
        &lib,
        &crate::stack::load::<()>().foreign,
        &mut selection
    ));

    // Empty is an ordinary text value, not a hidden spelling of
    // field absence.
    selection.edit_mut().unwrap().set_text("");
    write_through(
        &mut doc,
        &lib,
        &crate::stack::load::<()>().foreign,
        &mut selection,
    );
    assert_eq!(doc.cells.value(cell).and_then(name::read), Some(""));
    assert_eq!(
        doc.cells
            .value(cell)
            .and_then(Value::as_record)
            .and_then(|fields| { fields.get(&name::vocabulary::NAME) })
            .and_then(text::read),
        Some("")
    );

    // Removing the field uses the same structural deletion as any
    // other record field; the rest of the value remains.
    assert!(delete_edge(&mut doc, &lib, &path));
    assert_eq!(doc.cells.value(cell).and_then(name::read), None);
    assert!(
        doc.cells
            .value(cell)
            .and_then(Value::as_record)
            .is_some_and(|fields| fields.contains_key(&crate::test_values::label("x")))
    );
    assert!(make_selection(&doc, &lib, path).edit().is_none());
}

#[test]
fn editing_an_anonymous_lambdas_placeholder_creates_its_name_field() {
    let lib = Cells::new();
    let (mut doc, cell) = doc_of(vec![
        (grap::vocabulary::PARAMS, Value::list([])),
        (grap::vocabulary::BODY, Value::from(vec![1])),
    ]);
    let path = vec![Step::Follow, Step::Key(name::vocabulary::NAME)];
    let mut selection = make_projected_selection(&doc, &lib, path.clone());

    assert_eq!(selection.edit().map(LineEditState::text), Some(""));
    selection.edit_mut().unwrap().set_text("tree");
    assert!(write_through(
        &mut doc,
        &lib,
        &crate::stack::load::<()>().foreign,
        &mut selection,
    ));
    assert_eq!(doc.cells.value(cell).and_then(name::read), Some("tree"));
    assert_eq!(
        src(&doc, &lib).resolve(&path).and_then(text::read),
        Some("tree")
    );
}

#[test]
fn partials_receive_selection_and_annotations_positionally() {
    // The first library code ever to SEE editor state — as data,
    // positionally: the payload only at the selected path, the
    // annotation record only at its own.
    fn probe(
        input: &progred_display::ProjectionInput<'_, (), Hover>,
    ) -> Option<progred_display::Layout<(), Hover>> {
        input.value.as_blob()?;
        Some(progred_display::dim(
            match (input.selection.is_some(), input.state.is_some()) {
                (true, _) => "selected here",
                (false, true) => "annotated here",
                (false, false) => "cold",
            },
        ))
    }
    let doc = Document {
        root: Some(Value::from(vec![7u8])),
        cells: Cells::new(),
    };
    let lib = Cells::new();
    let projection: Projection<()> =
        Projection::new([probe as progred_display::Partial<(), Hover>]);
    let foreign = grap::ForeignFunctions::default();
    let styles = crate::styles::editor(1.0);
    let mut fonts = parley::FontContext::new();
    let mut layouts = parley::LayoutContext::new();
    let mut cache = puri::text::TextCache::default();
    let mut width = |selection: Option<&Selection>, annotations: &Annotations| {
        let mut tcx = TextCtx {
            fonts: &mut fonts,
            layouts: &mut layouts,
            scale: 1.0,
            cache: &mut cache,
        };
        project::<(), crate::frame::Paint>(
            ProjectDescription {
                sources: Sources {
                    doc: &doc,
                    library: &lib,
                },
                root: doc.root.as_ref(),
                root_path: &[],
                selection,
                source_selection: selection,
                annotations,
                raw: false,
                styles: &styles,
                width: 500.0,
                root_projection: None,
                projection: Some(&projection),
                foreign: &foreign,
            },
            &mut tcx,
            Hooks::<()> {
                select: Rc::new(|_, _| {}),
                select_payload: Rc::new(|_, _, _| {}),
                start_edit: Rc::new(|_, _, _| {}),
                toggle: Rc::new(|_, _| {}),
                edit: Rc::new(|_| None),
                pick: Rc::new(|_, _| false),
                insert: Rc::new(|_, _| {}),
                delete: Rc::new(|_| false),
                apply: Rc::new(|_, _, _, _| false),
                point: Rc::new(|_, _, _, _, _| false),
                commit_offer: Rc::new(|_, _| {}),
            },
        )
        .extent
        .width
    };
    let empty = Annotations::default();
    let cold = width(None, &empty);
    let selected = width(Some(&crate::selection::bare_edge(Vec::new())), &empty);
    let mut marked = Annotations::default();
    marked.set_field(&[], crate::annotations::FOLD, Some(Value::from(vec![1u8])));
    let annotated = width(None, &marked);
    assert_ne!(cold, selected);
    assert_ne!(cold, annotated);
    assert_ne!(selected, annotated);
}

#[test]
fn the_pending_query_writes_through_to_the_payload() {
    let mut doc = Document {
        root: None,
        cells: Cells::new(),
    };
    let lib = Cells::new();
    let mut pending = crate::selection::pending_with_query(Vec::new(), "");
    pending.set_choice(2);
    pending
        .edit_mut()
        .unwrap()
        .handle_ime(&puri::handler::ImeEvent::Commit("ab".to_string()));
    // The payload is stale only WITHIN the dispatch...
    assert_eq!(selection_payload::query(pending.payload()), Some(""));
    assert_eq!(pending.choice(), 2);
    // ...and the per-event write-through syncs it, the same point the
    // document takes its writes.
    write_through(
        &mut doc,
        &lib,
        &crate::stack::load::<()>().foreign,
        &mut pending,
    );
    assert_eq!(selection_payload::query(pending.payload()), Some("ab"));
    assert_eq!(pending.choice(), 0);
}

#[test]
fn a_projection_defined_as_data_realizes() {
    // The display language's data form, decoded with the PROVIDED
    // intents and realized through the ordinary pipeline — the same
    // boundary a Grap-backed library projection can use.
    fn probe(
        input: &progred_display::ProjectionInput<'_, (), Hover>,
    ) -> Option<progred_display::Layout<(), Hover>> {
        use progred_libraries::layout as data;
        input.value.as_blob()?;
        let target = input.targets.current();
        data::decode(
            &data::selectable(data::row(
                4.0,
                [
                    data::text_leaf("from", data::vocabulary::NAME_FACE),
                    data::text_leaf("data", data::vocabulary::DIM_FACE),
                ],
            )),
            &target.select,
            &target.hover,
        )
    }
    let doc = Document {
        root: Some(Value::from(vec![7u8])),
        cells: Cells::new(),
    };
    let lib = Cells::new();
    let projection: Projection<()> =
        Projection::new([probe as progred_display::Partial<(), Hover>]);
    let foreign = grap::ForeignFunctions::default();
    let styles = crate::styles::editor(1.0);
    let mut fonts = parley::FontContext::new();
    let mut layouts = parley::LayoutContext::new();
    let mut cache = puri::text::TextCache::default();
    let mut tcx = TextCtx {
        fonts: &mut fonts,
        layouts: &mut layouts,
        scale: 1.0,
        cache: &mut cache,
    };
    let empty = Annotations::default();
    let measured = project::<(), crate::frame::Paint>(
        ProjectDescription {
            sources: Sources {
                doc: &doc,
                library: &lib,
            },
            root: doc.root.as_ref(),
            root_path: &[],
            selection: None,
            source_selection: None,
            annotations: &empty,
            raw: false,
            styles: &styles,
            width: 500.0,
            root_projection: None,
            projection: Some(&projection),
            foreign: &foreign,
        },
        &mut tcx,
        Hooks::<()> {
            select: Rc::new(|_, _| {}),
            select_payload: Rc::new(|_, _, _| {}),
            start_edit: Rc::new(|_, _, _| {}),
            toggle: Rc::new(|_, _| {}),
            edit: Rc::new(|_| None),
            pick: Rc::new(|_, _| false),
            insert: Rc::new(|_, _| {}),
            delete: Rc::new(|_| false),
            apply: Rc::new(|_, _, _, _| false),
            point: Rc::new(|_, _, _, _, _| false),
            commit_offer: Rc::new(|_, _| {}),
        },
    );
    assert!(measured.extent.width > 0.0);
    let placed = measured::place(
        measured,
        puri::geometry::Placement::root(measured_rect(500.0)),
    );
    // The data's selectable attached the provided semantic action.
    assert!(!placed.activations.is_empty());
}

#[test]
fn a_data_event_realizes_the_apply_hook() {
    fn probe(
        input: &progred_display::ProjectionInput<'_, Vec<(Path, Value, Value)>, Hover>,
    ) -> Option<progred_display::Layout<Vec<(Path, Value, Value)>, Hover>> {
        use progred_libraries::layout as data;
        input.value.as_blob()?;
        let target = input.targets.current();
        data::decode(
            &data::on(
                data::text_leaf("go", data::vocabulary::NAME_FACE),
                Value::from(data::vocabulary::HANDLER),
            ),
            &target.select,
            &target.hover,
        )
    }
    let doc = Document {
        root: Some(Value::from(vec![7u8])),
        cells: Cells::new(),
    };
    let lib = Cells::new();
    let projection: Projection<Vec<(Path, Value, Value)>> =
        Projection::new([probe as progred_display::Partial<Vec<(Path, Value, Value)>, Hover>]);
    let foreign = grap::ForeignFunctions::default();
    let styles = crate::styles::editor(1.0);
    let mut fonts = parley::FontContext::new();
    let mut layouts = parley::LayoutContext::new();
    let mut cache = puri::text::TextCache::default();
    let mut tcx = TextCtx {
        fonts: &mut fonts,
        layouts: &mut layouts,
        scale: 1.0,
        cache: &mut cache,
    };
    let empty = Annotations::default();
    let measured = project::<Vec<(Path, Value, Value)>, crate::frame::Paint>(
        ProjectDescription {
            sources: Sources {
                doc: &doc,
                library: &lib,
            },
            root: doc.root.as_ref(),
            root_path: &[],
            selection: None,
            source_selection: None,
            annotations: &empty,
            raw: false,
            styles: &styles,
            width: 500.0,
            root_projection: None,
            projection: Some(&projection),
            foreign: &foreign,
        },
        &mut tcx,
        Hooks::<Vec<(Path, Value, Value)>> {
            select: Rc::new(|_, _| {}),
            select_payload: Rc::new(|_, _, _| {}),
            start_edit: Rc::new(|_, _, _| {}),
            toggle: Rc::new(|_, _| {}),
            edit: Rc::new(|_| None),
            pick: Rc::new(|_, _| false),
            insert: Rc::new(|_, _| {}),
            delete: Rc::new(|_| false),
            apply: Rc::new(|events, path, handler, event| {
                events.push((path, handler, event));
                true
            }),
            point: Rc::new(|_, _, _, _, _| false),
            commit_offer: Rc::new(|_, _| {}),
        },
    );
    assert!(measured.extent.width > 0.0);
    let placed = measured::place(
        measured,
        puri::geometry::Placement::root(measured_rect(500.0)),
    );
    let handler = placed.handler.expect("event handler");
    let mut state = PointerState::default();
    state.position.x = 1.0;
    state.position.y = 1.0;
    let mut events = Vec::new();
    assert!(handler.dispatch_pointer_down(
        &mut events,
        &PointerButtonEvent {
            button: Some(PointerButton::Primary),
            pointer: PointerInfo {
                pointer_id: Some(PointerId::PRIMARY),
                persistent_device_id: None,
                pointer_type: PointerType::Mouse,
            },
            state: state.clone(),
        },
    ));
    let [(path, function, event)] = &events[..] else {
        panic!("one event");
    };
    assert!(path.is_empty());
    assert_eq!(
        function,
        &Value::from(progred_libraries::layout::vocabulary::HANDLER)
    );
    assert_eq!(
        event
            .as_record()
            .and_then(|fields| fields.get(&progred_libraries::layout::vocabulary::EVENT_KIND))
            .and_then(Value::as_cell),
        Some(progred_libraries::layout::vocabulary::POINTER_DOWN),
    );
    assert!(handler.dispatch_pointer_move(
        &mut events,
        &PointerUpdate {
            pointer: PointerInfo {
                pointer_id: Some(PointerId::PRIMARY),
                persistent_device_id: None,
                pointer_type: PointerType::Mouse,
            },
            current: state.clone(),
            coalesced: Vec::new(),
            predicted: Vec::new(),
        },
    ));
    assert_eq!(
        events[1]
            .2
            .as_record()
            .and_then(|fields| fields.get(&progred_libraries::layout::vocabulary::EVENT_KIND))
            .and_then(Value::as_cell),
        Some(progred_libraries::layout::vocabulary::POINTER_MOVE),
    );

    let touch = PointerInfo {
        pointer_id: PointerId::new(7),
        persistent_device_id: None,
        pointer_type: PointerType::Touch,
    };
    assert!(handler.dispatch_pointer_down(
        &mut events,
        &PointerButtonEvent {
            button: None,
            pointer: touch,
            state: state.clone(),
        },
    ));
    let touch_start = events[2].2.as_record().expect("touch start record");
    assert_eq!(
        touch_start
            .get(&progred_libraries::layout::vocabulary::EVENT_KIND)
            .and_then(Value::as_cell),
        Some(progred_libraries::layout::vocabulary::TOUCH_START),
    );
    assert!(!touch_start.contains_key(&progred_libraries::layout::vocabulary::BUTTON));

    assert!(handler.dispatch_pointer_move(
        &mut events,
        &PointerUpdate {
            pointer: touch,
            current: state.clone(),
            coalesced: Vec::new(),
            predicted: Vec::new(),
        },
    ));
    assert_eq!(
        events[3]
            .2
            .as_record()
            .and_then(|fields| fields.get(&progred_libraries::layout::vocabulary::EVENT_KIND))
            .and_then(Value::as_cell),
        Some(progred_libraries::layout::vocabulary::TOUCH_MOVE),
    );

    assert!(handler.dispatch_pointer_up(
        &mut events,
        &PointerButtonEvent {
            button: None,
            pointer: touch,
            state: state.clone(),
        },
    ));
    assert_eq!(
        events[4]
            .2
            .as_record()
            .and_then(|fields| fields.get(&progred_libraries::layout::vocabulary::EVENT_KIND))
            .and_then(Value::as_cell),
        Some(progred_libraries::layout::vocabulary::TOUCH_END),
    );

    assert!(handler.dispatch_pointer_cancel(&mut events, &touch));
    assert_eq!(
        events[5]
            .2
            .as_record()
            .and_then(|fields| fields.get(&progred_libraries::layout::vocabulary::EVENT_KIND))
            .and_then(Value::as_cell),
        Some(progred_libraries::layout::vocabulary::TOUCH_CANCEL),
    );

    assert!(
        handler
            .dispatch_scroll(
                &mut events,
                &PointerScrollEvent {
                    pointer: PointerInfo {
                        pointer_id: Some(PointerId::PRIMARY),
                        persistent_device_id: None,
                        pointer_type: PointerType::Mouse,
                    },
                    delta: ScrollDelta::LineDelta(0.0, 1.0),
                    state,
                },
            )
            .handled()
    );
    assert_eq!(
        events[6]
            .2
            .as_record()
            .and_then(|fields| fields.get(&progred_libraries::layout::vocabulary::EVENT_KIND))
            .and_then(Value::as_cell),
        Some(progred_libraries::layout::vocabulary::SCROLL),
    );
}

fn measured_rect(width: f64) -> kurbo::Rect {
    kurbo::Rect::new(0.0, 0.0, width, 100.0)
}
