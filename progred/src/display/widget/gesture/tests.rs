use super::*;
use crate::display::test_support::ResolveForDispatch;
use crate::display::test_support::press_at;
use crate::display::widget::view::Root;
use crate::libraries::f64;
use crate::libraries::test_widgets::{drag_frame, gesture_place, hover};
use gid::{Document, Path, Step};
use kurbo::Point;
use puri::handler::test_support::DispatchShorthands;
use ui_events::keyboard::Modifiers;
use ui_events::pointer::{PointerButton, PointerType};

fn model(value: Value) -> crate::Editor {
    crate::editor_of(value)
}

/// A state drag from `origin` that annotates `path` in `root`'s view.
fn annotating_drag(
    origin: Point,
    scale: f64,
    root: Root,
    path: Path,
    handler: StateDragHandler,
) -> Box<dyn Gesture> {
    state_drag(
        Drag::new(origin, scale, crate::gesture::DRAG_THRESHOLD),
        handler(),
        Rc::new(move |session: &mut crate::editing::Session<'_>, value| {
            if let Some(view) = session.model.workspace.view_mut(&root) {
                view.annotations.set(&path, Some(value));
                true
            } else {
                false
            }
        }),
    )
}

#[test]
fn state_drag_press_composes_selection_and_start_in_pointer_order() {
    for (accepts, covered) in [(true, false), (false, false), (true, true)] {
        let log = Rc::new(std::cell::RefCell::new(Vec::new()));
        let select_log = log.clone();
        let start_log = log.clone();
        let frame = drag_frame(
            gesture_place(
                crate::display::on_state_drag(
                    crate::display::row(0.0, []),
                    hover(vec![]),
                    Rc::new(move |session| {
                        select_log.borrow_mut().push("select");
                        if accepts {
                            crate::editing::Scope::default()
                                .open(session.reborrow())
                                .select(&crate::test_root(), &[]);
                        }
                        accepts
                    }),
                    Rc::new(move || {
                        start_log.borrow_mut().push("start");
                        Box::new(|_, _| Value::record([]))
                    }),
                ),
                false,
            ),
            covered,
        );
        let mut world = crate::test_editor(Document::default());
        let handled = frame.resolve_for_dispatch().dispatch_pointer_down_with(
            &mut world,
            &press_at(Point::new(5.0, 5.0), Modifiers::empty()),
            &mut crate::frame::DispatchContext::new(Some(crate::test_root()), Some(hover(vec![]))),
        );
        assert_eq!(handled, covered || accepts);
        assert_eq!(world.gesture.is_some(), accepts && !covered);
        assert_eq!(world.model.selection.is_some(), accepts && !covered);
        assert_eq!(
            *log.borrow(),
            if covered {
                vec![]
            } else if accepts {
                vec!["select", "start"]
            } else {
                vec!["select"]
            }
        );
    }
}

#[test]
fn state_drag_starts_only_at_a_visible_primary_contact_in_its_own_view() {
    for (x, button, pointer_type, owns_view, expected) in [
        (
            5.0,
            Some(PointerButton::Primary),
            PointerType::Mouse,
            true,
            true,
        ),
        (5.0, None, PointerType::Touch, true, true),
        (
            15.0,
            Some(PointerButton::Primary),
            PointerType::Mouse,
            true,
            false,
        ),
        (
            25.0,
            Some(PointerButton::Primary),
            PointerType::Mouse,
            true,
            false,
        ),
        (
            5.0,
            Some(PointerButton::Secondary),
            PointerType::Mouse,
            true,
            false,
        ),
        (
            5.0,
            Some(PointerButton::Primary),
            PointerType::Mouse,
            false,
            false,
        ),
    ] {
        let frame = drag_frame(
            gesture_place(
                crate::display::on_state_drag(
                    crate::display::row(0.0, []),
                    hover(vec![]),
                    Rc::new(|_| true),
                    Rc::new(|| Box::new(|_, _| Value::record([]))),
                ),
                false,
            ),
            false,
        );
        let mut world = crate::test_editor(Document::default());
        let mut event = press_at(Point::new(x, 5.0), Modifiers::empty());
        event.button = button;
        event.pointer.pointer_type = pointer_type;
        let handled = frame.resolve_for_dispatch().dispatch_pointer_down_with(
            &mut world,
            &event,
            &mut crate::frame::DispatchContext::new(
                owns_view.then(crate::test_root),
                Some(hover(vec![])),
            ),
        );
        assert_eq!(handled, expected);
        assert_eq!(world.gesture.is_some(), expected);
    }
}

#[test]
fn readonly_gesture_controls_do_not_start_or_construct_edit_runs() {
    for layout in [
        crate::libraries::number::scrub::on_scrub(
            crate::display::row(0.0, []),
            hover(vec![]),
            Rc::new(|| panic!("read-only scrub")),
        ),
        crate::display::on_point(
            crate::display::row(0.0, []),
            Rc::new(|_| panic!("read-only point control")),
        ),
    ] {
        let frame = drag_frame(gesture_place(layout, true), false);
        let mut world = crate::test_editor(Document::default());
        assert!(
            !frame
                .handler
                .is_some_and(|handler| handler.dispatch_pointer_down_with(
                    &mut world,
                    &press_at(Point::new(5.0, 5.0), crate::modifiers::command()),
                    &mut crate::frame::DispatchContext::new(
                        Some(crate::test_root()),
                        Some(hover(vec![]))
                    )
                ))
        );
        assert!(world.gesture.is_none());
    }
}

#[test]
fn state_drags_use_total_logical_distance_and_only_update_their_view() {
    let mut model = model(Value::record([]));
    let root = model.model.workspace.document_root().clone();
    let path = vec![Step::Key(gid::new_cell_id())];
    let mut gesture = annotating_drag(
        Point::ZERO,
        2.0,
        root.clone(),
        path.clone(),
        Rc::new(|| {
            Box::new(|event, _| Value::list([f64::value(event.delta_x), f64::value(event.delta_y)]))
        }),
    );
    gesture.advance(&mut model.session(), &[Point::new(5.0, 0.0)]);
    assert!(
        model
            .model
            .workspace
            .view(&root)
            .unwrap()
            .annotations
            .at(&path)
            .is_none()
    );
    gesture.advance(&mut model.session(), &[Point::new(8.0, 12.0)]);
    gesture.advance(&mut model.session(), &[Point::new(10.0, 12.0)]);
    assert_eq!(
        model
            .model
            .workspace
            .view(&root)
            .unwrap()
            .annotations
            .at(&path),
        Some(&Value::list([f64::value(5.0), f64::value(6.0)]))
    );
    assert_eq!(model.model.doc.root, Some(Value::record([])));
    assert!(!model.model.history.can_undo());
}

#[test]
fn state_drag_gets_one_batch_including_threshold_excursions() {
    let mut model = model(Value::record([]));
    let root = model.model.workspace.document_root().clone();
    let calls = Rc::new(std::cell::RefCell::new(Vec::new()));
    let log = calls.clone();
    let mut gesture = annotating_drag(
        Point::ZERO,
        1.0,
        root.clone(),
        vec![],
        Rc::new(move || {
            let log = log.clone();
            Box::new(move |current, coalesced| {
                log.borrow_mut().push((current, coalesced.to_vec()));
                f64::value(current.delta_x)
            })
        }),
    );
    gesture.advance(
        &mut model.session(),
        &[Point::new(8.0, 0.0), Point::new(1.0, 0.0)],
    );
    let sample = |x| StateDragEvent {
        delta_x: x,
        delta_y: 0.0,
    };
    assert_eq!(*calls.borrow(), [(sample(1.0), vec![sample(8.0)])]);
    assert_eq!(
        model
            .model
            .workspace
            .view(&root)
            .unwrap()
            .annotations
            .at(&[]),
        Some(&f64::value(1.0))
    );
    assert!(!model.model.history.can_undo());
}

#[test]
fn point_controls_clamp_unbounded_motion_and_coalesce_their_writes() {
    let mut model = model(f64::value(0.0));
    let original = model.model.doc.clone();
    let root = model.model.workspace.document_root().clone();
    model.model.selection = Some(crate::selection::Selection::edge(&root, vec![]));
    let mut gesture = point(
        Rect::new(10.0, 20.0, 110.0, 120.0),
        Rc::new(|event| PointUpdate {
            value: Value::list([f64::value(event.x), f64::value(event.y)]),
            selection: Some(crate::selection::payload::edge()),
        }),
        crate::editing::Site::new(root.clone(), Rc::from([]), Default::default()).edit_run(),
    );
    assert!(gesture.advance(&mut model.session(), &[Point::new(30.0, 40.0)]));
    assert_eq!(
        model.model.doc.root,
        Some(Value::list([f64::value(0.2), f64::value(0.2)]))
    );
    assert!(gesture.advance(&mut model.session(), &[Point::new(210.0, -20.0)]));
    assert_eq!(
        model.model.doc.root,
        Some(Value::list([f64::value(1.0), f64::value(0.0)]))
    );
    assert!(!gesture.advance(&mut model.session(), &[Point::new(210.0, -20.0)]));
    assert_eq!(model.model.selection.as_ref().unwrap().root(), &root);
    assert!(model.model.step_history(true));
    assert_eq!(model.model.doc.root, original.root);
    assert!(!model.model.history.can_undo());
}
