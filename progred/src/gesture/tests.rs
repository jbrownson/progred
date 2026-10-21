use super::*;
use crate::Editor;
use crate::libraries::f64;
use crate::occurrence::{Path, Step};
use std::rc::Rc;

use crate::libraries::number::scrub::{ScrubHandler, ScrubUpdate};
use kurbo::Point;
use puri::drag::Drag;

fn model(value: Value) -> Editor {
    crate::editor_of(value)
}

fn scrub(origin: Point, scale: f64, path: Path, handler: ScrubHandler) -> Active {
    let edit = Site::new(path.into()).edit_run();
    crate::libraries::number::scrub::scrub(
        Drag::new(origin, scale, DRAG_THRESHOLD),
        handler(),
        edit,
    )
}

#[test]
fn edit_runs_do_not_write_on_construction_or_retarget_another_selection() {
    let mut world = model(f64::value(10.0));
    let original = world.model.doc.clone();
    let other_path = vec![Step::Key(gid::new_cell_id())];
    world.model.selection = Some(Selection::edge(other_path.clone()));
    let mut edit = Site::new(crate::occurrence::Trail::from([])).edit_run();
    assert!(Rc::ptr_eq(&original, &world.model.doc));
    assert!(!edit.write(&mut world.session(), f64::value(10.0)));
    assert!(Rc::ptr_eq(&original, &world.model.doc));
    assert!(!world.model.history.can_undo());
    edit.selection(&mut world.session(), selection::payload::edge());
    assert_eq!(world.model.selection.as_ref().unwrap().path(), other_path);
    assert!(edit.write(&mut world.session(), f64::value(11.0)));
    assert!(edit.write(&mut world.session(), f64::value(12.0)));
    assert_eq!(world.model.selection.as_ref().unwrap().path(), other_path);
    assert!(world.model.step_history(true));
    assert!(Rc::ptr_eq(&original, &world.model.doc));
    assert!(!world.model.history.can_undo());
}

#[test]
fn finishing_a_scrub_preserves_other_payload_and_never_retargets_selection() {
    let mut world = model(f64::value(1.0));
    let marker = gid::new_cell_id();
    let payload = Value::record([(marker, Value::from(vec![7]))]);
    world.model.selection = Some(Selection::from_payload(
        &world.sources(),
        vec![],
        payload.clone(),
    ));
    let gesture = || {
        scrub(
            Point::ZERO,
            1.0,
            vec![],
            Rc::new(|| {
                Box::new(|_| ScrubUpdate {
                    value: f64::value(2.0),
                    spelling: "2.0".into(),
                })
            }),
        )
    };
    let mut active = gesture();
    active.advance(&mut world.session(), &[Point::new(10.0, 0.0)]);
    assert_eq!(
        world
            .model
            .selection
            .as_ref()
            .unwrap()
            .edit()
            .unwrap()
            .text(),
        "2.0"
    );
    active.finish(&mut world.session());
    assert_eq!(world.model.selection.as_ref().unwrap().payload(), payload);
    let mut active = gesture();
    active.advance(&mut world.session(), &[Point::new(10.0, 0.0)]);
    let other = vec![Step::Key(gid::new_cell_id())];
    world.model.selection = Some(Selection::edge(other.clone()));
    active.finish(&mut world.session());
    assert_eq!(world.model.selection.as_ref().unwrap().path(), other);
}

#[test]
fn gesture_finalization_runs_before_replacement_and_history_restore() {
    struct Finish(Rc<std::cell::RefCell<Vec<Option<Value>>>>);
    impl Gesture for Finish {
        fn advance(&mut self, _: &mut crate::editing::Session<'_>, _: &[Point]) -> bool {
            false
        }
        fn finish(&mut self, session: &mut crate::editing::Session<'_>) {
            self.0.borrow_mut().push(session.model.doc.root.clone());
        }
    }
    let mut world = model(f64::value(1.0));
    let log = Rc::new(std::cell::RefCell::new(Vec::new()));
    world.gesture = Some(Box::new(Finish(log.clone())));
    world.replace_document(
        gid::Document {
            root: Some(f64::value(2.0)),
            cells: gid::Cells::new(),
        },
        None,
        Default::default(),
    );
    assert!(world.gesture.is_none());
    world.gesture = Some(Box::new(Finish(log.clone())));
    world.step_history(true, Default::default());
    assert!(world.gesture.is_none());
    assert_eq!(
        *log.borrow(),
        [Some(f64::value(1.0)), Some(f64::value(2.0))]
    );
}

#[test]
fn scrubbing_keeps_threshold_distance_precision_and_one_undo_run() {
    let mut model = model(f64::value(10.0));
    let original = model.model.doc.clone();
    model.model.selection = Some(Selection::edge(vec![]));
    let mut gesture = scrub(
        Point::new(100.0, 200.0),
        2.0,
        vec![],
        Rc::new(|| {
            let mut value = 10.0;
            Box::new(move |event| {
                assert_eq!(event.distance_y, 3.0);
                value += event.movement_x;
                ScrubUpdate {
                    value: f64::value(value),
                    spelling: format!("{value:.1}"),
                }
            })
        }),
    );
    assert!(!gesture.advance(&mut model.session(), &[Point::new(102.0, 202.0)]));
    assert!(!model.model.history.can_undo());
    assert!(gesture.advance(&mut model.session(), &[Point::new(108.0, 206.0)]));
    assert_eq!(model.model.doc.root, Some(f64::value(14.0)));
    assert!(gesture.advance(&mut model.session(), &[Point::new(110.0, 206.0)]));
    assert_eq!(model.model.doc.root, Some(f64::value(15.0)));
    assert!(gesture.advance(&mut model.session(), &[Point::new(-20.0, 206.0)]));
    assert_eq!(model.model.doc.root, Some(f64::value(-50.0)));
    assert!(gesture.advance(&mut model.session(), &[Point::new(110.0, 206.0)]));
    assert_eq!(model.model.doc.root, Some(f64::value(15.0)));
    assert_eq!(
        model
            .model
            .selection
            .as_ref()
            .unwrap()
            .edit()
            .unwrap()
            .text(),
        "15.0"
    );
    gesture.finish(&mut model.session());
    assert!(model.model.selection.as_ref().unwrap().edit().is_none());
    assert!(model.model.step_history(true));
    assert_eq!(model.model.doc.root, original.root);
    assert!(!model.model.history.can_undo());
}

#[test]
fn scrubbing_a_batch_preserves_the_precision_path_and_one_undo_step() {
    let mut model = model(f64::value(0.0));
    let samples = [
        Point::new(10.0, 0.0),
        Point::new(10.0, 10.0),
        Point::new(20.0, 10.0),
    ];
    let received = Rc::new(std::cell::RefCell::new(Vec::new()));
    let log = received.clone();
    let mut gesture = scrub(
        Point::ZERO,
        1.0,
        vec![],
        Rc::new(move || {
            let log = log.clone();
            let mut value = 0.0;
            Box::new(move |event| {
                log.borrow_mut().push((event.movement_x, event.distance_y));
                value += event.movement_x * if event.distance_y == 0.0 { 1.0 } else { 0.1 };
                ScrubUpdate {
                    value: f64::value(value),
                    spelling: value.to_string(),
                }
            })
        }),
    );
    assert!(gesture.advance(&mut model.session(), &samples));
    assert_eq!(*received.borrow(), [(10.0, 0.0), (0.0, 10.0), (10.0, 10.0)]);
    assert_eq!(
        model.model.doc.root,
        Some(f64::value(11.0)),
        "latest-only would produce 2"
    );
    assert!(model.model.step_history(true));
    assert_eq!(model.model.doc.root, Some(f64::value(0.0)));
    assert!(!model.model.history.can_undo());
}
