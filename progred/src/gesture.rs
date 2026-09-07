//! Caller-owned continuations for projection gestures. The accepting
//! press installs one; it owns motion until release or cancellation.

use crate::model::Model;
use crate::selection::{self, Selection};
use crate::sources::Sources;
use crate::workspace::Root;
use gid::{Path, Step, Value};
use kurbo::Point;
use progred_display::widget::gesture::{Gesture, ValueEdit};
use progred_libraries::Libraries;

// Logical pixels; replace when the input adapter exposes the platform threshold.
pub(crate) const DRAG_THRESHOLD: f64 = 3.0;

pub(crate) struct Active<World> {
    root: Root,
    path: Path,
    continuation: Box<dyn Gesture<World>>,
}

pub(crate) struct ScrubSpelling<'a> {
    pub root: &'a Root,
    pub path: &'a [Step],
    pub spelling: &'a str,
}

impl<World> Active<World> {
    pub(crate) fn new(root: Root, path: Path, continuation: Box<dyn Gesture<World>>) -> Self {
        Self {
            root,
            path,
            continuation,
        }
    }

    pub(crate) fn advance(&mut self, world: &mut World, samples: &[Point]) -> bool {
        self.continuation.advance(world, samples)
    }

    pub(crate) fn scrub_spelling(&self) -> Option<ScrubSpelling<'_>> {
        self.continuation.spelling().map(|spelling| ScrubSpelling {
            root: &self.root,
            path: &self.path,
            spelling,
        })
    }
}

pub(crate) fn value_edit<World: 'static>(
    root: Root,
    path: Path,
    select: progred_display::ActionHandler<World>,
    access: fn(&mut World) -> (&mut Model, &Libraries),
) -> ValueEdit<World> {
    let write_path = path.clone();
    let mut recorded = false;
    ValueEdit {
        select,
        write: Box::new(move |world, value| {
            let (model, libraries) = access(world);
            write_value(model, libraries, &write_path, value, &mut recorded)
        }),
        selection: std::rc::Rc::new(move |world, payload| {
            let (model, libraries) = access(world);
            if let Some(recorded) = model
                .selection
                .as_ref()
                .filter(|selection| selection.root() == &root && selection.path() == path)
                .map(Selection::recorded)
            {
                let mut next = Selection::from_payload(
                    &root,
                    &Sources {
                        doc: &model.doc,
                        libraries,
                    },
                    path.clone(),
                    payload,
                );
                next.preserve_recorded(recorded);
                model.selection = Some(next);
            }
        }),
    }
}

fn write_value(
    model: &mut Model,
    libraries: &Libraries,
    path: &[Step],
    replacement: Value,
    recorded: &mut bool,
) -> bool {
    if (Sources {
        doc: &model.doc,
        libraries,
    })
    .resolve_path(path)
        == Some(&replacement)
    {
        false
    } else {
        let before = model.snapshot();
        let wrote = selection::set_value(&mut model.doc, libraries, path, replacement);
        if wrote && !*recorded {
            model.history.record(before);
            *recorded = true;
        }
        wrote
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use progred_libraries::f64;
    use std::rc::Rc;

    use kurbo::Rect;
    use progred_display::widget::gesture as native;
    use progred_display::{PointHandler, ScrubHandler, StateDragHandler};
    use puri::drag::Drag;

    struct World {
        model: Model,
        libraries: Libraries,
    }
    impl std::ops::Deref for World {
        type Target = Model;
        fn deref(&self) -> &Model {
            &self.model
        }
    }
    impl std::ops::DerefMut for World {
        fn deref_mut(&mut self) -> &mut Model {
            &mut self.model
        }
    }

    fn model(value: Value) -> World {
        World {
            model: Model::new(gid::Document {
                root: Some(value),
                cells: gid::Cells::new(),
            }),
            libraries: Libraries::default(),
        }
    }

    fn edit(root: Root, path: Path) -> ValueEdit<World> {
        value_edit(root, path, Rc::new(|_| true), |world| {
            (&mut world.model, &world.libraries)
        })
    }

    fn scrub(
        origin: Point,
        scale: f64,
        root: Root,
        path: Path,
        handler: ScrubHandler,
    ) -> Active<World> {
        let edit = edit(root.clone(), path.clone());
        Active::new(
            root,
            path,
            native::scrub(
                Drag::new(origin, scale, DRAG_THRESHOLD),
                handler(),
                edit.write,
            ),
        )
    }

    fn state_drag(
        origin: Point,
        scale: f64,
        root: Root,
        path: Path,
        handler: StateDragHandler,
    ) -> Active<World> {
        let state_root = root.clone();
        let state_path = path.clone();
        Active::new(
            root,
            path,
            native::state_drag(
                Drag::new(origin, scale, DRAG_THRESHOLD),
                handler(),
                Rc::new(move |world: &mut World, value| {
                    if let Some(view) = world.workspace.view_mut(&state_root) {
                        view.annotations.set(&state_path, Some(value));
                        true
                    } else {
                        false
                    }
                }),
            ),
        )
    }

    fn point(root: Root, path: Path, rect: Rect, handler: PointHandler) -> Active<World> {
        let edit = edit(root.clone(), path.clone());
        Active::new(root, path, native::point(rect, handler, edit))
    }

    #[test]
    fn edit_runs_do_not_write_on_construction_or_retarget_another_selection() {
        let mut world = model(f64::value(10.0));
        let original = world.doc.clone();
        let root = world.workspace.document_root().clone();
        let other_path = vec![Step::Key(gid::new_cell_id())];
        world.selection = Some(selection::bare_edge(&root, other_path.clone()));
        let mut edit = value_edit(
            root,
            vec![],
            Rc::new(|_: &mut World| panic!("creating or writing an edit does not select")),
            |world| (&mut world.model, &world.libraries),
        );
        assert!(Rc::ptr_eq(&original, &world.doc));
        assert!(!(edit.write)(&mut world, f64::value(10.0)));
        assert!(Rc::ptr_eq(&original, &world.doc));
        assert!(!world.history.can_undo());
        (edit.selection)(&mut world, selection::payload::edge());
        assert_eq!(world.selection.as_ref().unwrap().path(), other_path);
        assert!((edit.write)(&mut world, f64::value(11.0)));
        assert!((edit.write)(&mut world, f64::value(12.0)));
        assert!(world.step_history(true));
        assert!(Rc::ptr_eq(&original, &world.doc));
        assert!(!world.history.can_undo());
    }

    #[test]
    fn scrubbing_keeps_threshold_distance_precision_and_one_undo_run() {
        let mut model = model(f64::value(10.0));
        let original = model.doc.clone();
        let root = model.workspace.document_root().clone();
        let mut gesture = scrub(
            Point::new(100.0, 200.0),
            2.0,
            root.clone(),
            vec![],
            Rc::new(|| {
                let mut value = 10.0;
                Box::new(move |event| {
                    assert_eq!(event.distance_y, 3.0);
                    value += event.movement_x;
                    progred_display::ScrubUpdate {
                        value: f64::value(value),
                        spelling: Some(format!("{value:.1}")),
                    }
                })
            }),
        );
        assert!(!gesture.advance(&mut model, &[Point::new(102.0, 202.0)]));
        assert!(!model.history.can_undo());
        assert!(gesture.advance(&mut model, &[Point::new(108.0, 206.0)]));
        assert_eq!(model.doc.root, Some(f64::value(14.0)));
        assert!(gesture.advance(&mut model, &[Point::new(110.0, 206.0)]));
        assert_eq!(model.doc.root, Some(f64::value(15.0)));
        assert!(gesture.advance(&mut model, &[Point::new(-20.0, 206.0)]));
        assert_eq!(model.doc.root, Some(f64::value(-50.0)));
        assert!(gesture.advance(&mut model, &[Point::new(110.0, 206.0)]));
        assert_eq!(model.doc.root, Some(f64::value(15.0)));
        let presentation = gesture.scrub_spelling().unwrap();
        assert_eq!(presentation.root, &root);
        assert!(presentation.path.is_empty());
        assert_eq!(presentation.spelling, "15.0");
        assert!(model.step_history(true));
        assert_eq!(model.doc.root, original.root);
        assert!(!model.history.can_undo());
    }

    #[test]
    fn state_drags_use_total_logical_distance_and_only_update_their_view() {
        let mut model = model(Value::record([]));
        let root = model.workspace.document_root().clone();
        let path = vec![Step::Key(gid::new_cell_id())];
        let mut gesture = state_drag(
            Point::ZERO,
            2.0,
            root.clone(),
            path.clone(),
            Rc::new(|| {
                Box::new(|event, _| {
                    Value::list([f64::value(event.delta_x), f64::value(event.delta_y)])
                })
            }),
        );
        gesture.advance(&mut model, &[Point::new(5.0, 0.0)]);
        assert!(
            model
                .workspace
                .view(&root)
                .unwrap()
                .annotations
                .at(&path)
                .is_none()
        );
        gesture.advance(&mut model, &[Point::new(8.0, 12.0)]);
        gesture.advance(&mut model, &[Point::new(10.0, 12.0)]);
        assert_eq!(
            model.workspace.view(&root).unwrap().annotations.at(&path),
            Some(&Value::list([f64::value(5.0), f64::value(6.0)]))
        );
        assert_eq!(model.doc.root, Some(Value::record([])));
        assert!(!model.history.can_undo());
    }

    #[test]
    fn scrubbing_a_batch_preserves_the_precision_path_and_one_undo_step() {
        let mut model = model(f64::value(0.0));
        let root = model.workspace.document_root().clone();
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
            root,
            vec![],
            Rc::new(move || {
                let log = log.clone();
                let mut value = 0.0;
                Box::new(move |event| {
                    log.borrow_mut().push((event.movement_x, event.distance_y));
                    value += event.movement_x * if event.distance_y == 0.0 { 1.0 } else { 0.1 };
                    progred_display::ScrubUpdate {
                        value: f64::value(value),
                        spelling: None,
                    }
                })
            }),
        );
        assert!(gesture.advance(&mut model, &samples));
        assert_eq!(*received.borrow(), [(10.0, 0.0), (0.0, 10.0), (10.0, 10.0)]);
        assert_eq!(
            model.doc.root,
            Some(f64::value(11.0)),
            "latest-only would produce 2"
        );
        assert!(model.step_history(true));
        assert_eq!(model.doc.root, Some(f64::value(0.0)));
        assert!(!model.history.can_undo());
    }

    #[test]
    fn state_drag_gets_one_batch_including_threshold_excursions() {
        let mut model = model(Value::record([]));
        let root = model.workspace.document_root().clone();
        let calls = Rc::new(std::cell::RefCell::new(Vec::new()));
        let log = calls.clone();
        let mut gesture = state_drag(
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
        gesture.advance(&mut model, &[Point::new(8.0, 0.0), Point::new(1.0, 0.0)]);
        let sample = |x| progred_display::StateDragEvent {
            delta_x: x,
            delta_y: 0.0,
        };
        assert_eq!(*calls.borrow(), [(sample(1.0), vec![sample(8.0)])]);
        assert_eq!(
            model.workspace.view(&root).unwrap().annotations.at(&[]),
            Some(&f64::value(1.0))
        );
        assert!(!model.history.can_undo());
    }

    #[test]
    fn point_controls_clamp_unbounded_motion_and_coalesce_their_writes() {
        let mut model = model(f64::value(0.0));
        let original = model.doc.clone();
        let root = model.workspace.document_root().clone();
        model.selection = Some(selection::bare_edge(&root, vec![]));
        let mut gesture = point(
            root.clone(),
            vec![],
            Rect::new(10.0, 20.0, 110.0, 120.0),
            Rc::new(|event| progred_display::PointUpdate {
                value: Value::list([f64::value(event.x), f64::value(event.y)]),
                selection: Some(selection::payload::edge()),
            }),
        );
        assert!(gesture.advance(&mut model, &[Point::new(30.0, 40.0)]));
        assert_eq!(
            model.doc.root,
            Some(Value::list([f64::value(0.2), f64::value(0.2)]))
        );
        assert!(gesture.advance(&mut model, &[Point::new(210.0, -20.0)]));
        assert_eq!(
            model.doc.root,
            Some(Value::list([f64::value(1.0), f64::value(0.0)]))
        );
        assert!(!gesture.advance(&mut model, &[Point::new(210.0, -20.0)]));
        assert_eq!(model.selection.as_ref().unwrap().root(), &root);
        assert!(model.step_history(true));
        assert_eq!(model.doc.root, original.root);
        assert!(!model.history.can_undo());
    }
}
