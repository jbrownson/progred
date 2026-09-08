//! Caller-owned continuations for projection gestures. The accepting
//! press installs one; it owns motion until release or cancellation.

use crate::Editor;
use crate::display::widget::gesture::{Gesture, ValueEdit};
use crate::libraries::Libraries;
use crate::model::Model;
use crate::selection::{self, Selection};
use crate::sources::Sources;
use crate::workspace::Root;
use gid::{Path, Step, Value};
use kurbo::Point;

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

pub(crate) fn value_edit(root: Root, path: Path) -> ValueEdit<Editor> {
    let select_root = root.clone();
    let select_path = path.clone();
    let write_path = path.clone();
    let mut recorded = false;
    ValueEdit {
        select: std::rc::Rc::new(move |app| {
            if app.model.selection.as_ref().is_some_and(|selection| {
                matches!(
                    selection.stage(&app.sources()),
                    selection::Stage::Pending | selection::Stage::Label
                )
            }) {
                false
            } else {
                crate::editing::select(app, &select_root, &select_path);
                true
            }
        }),
        write: Box::new(move |app, value| {
            write_value(
                &mut app.model,
                &app.stack.libraries,
                &write_path,
                value,
                &mut recorded,
            )
        }),
        selection: std::rc::Rc::new(move |app, payload| {
            let model = &mut app.model;
            let libraries = &app.stack.libraries;
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
    use crate::libraries::f64;
    use std::rc::Rc;

    use crate::display::widget::gesture as native;
    use crate::display::{PointHandler, ScrubHandler, StateDragHandler};
    use kurbo::Rect;
    use puri::drag::Drag;

    fn model(value: Value) -> Editor {
        crate::test_editor(gid::Document {
            root: Some(value),
            cells: gid::Cells::new(),
        })
    }

    fn scrub(
        origin: Point,
        scale: f64,
        root: Root,
        path: Path,
        handler: ScrubHandler,
    ) -> Active<Editor> {
        let edit = value_edit(root.clone(), path.clone());
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
    ) -> Active<Editor> {
        let state_root = root.clone();
        let state_path = path.clone();
        Active::new(
            root,
            path,
            native::state_drag(
                Drag::new(origin, scale, DRAG_THRESHOLD),
                handler(),
                Rc::new(move |world: &mut Editor, value| {
                    if let Some(view) = world.model.workspace.view_mut(&state_root) {
                        view.annotations.set(&state_path, Some(value));
                        true
                    } else {
                        false
                    }
                }),
            ),
        )
    }

    fn point(root: Root, path: Path, rect: Rect, handler: PointHandler) -> Active<Editor> {
        let edit = value_edit(root.clone(), path.clone());
        Active::new(root, path, native::point(rect, handler, edit))
    }

    #[test]
    fn edit_runs_do_not_write_on_construction_or_retarget_another_selection() {
        let mut world = model(f64::value(10.0));
        let original = world.model.doc.clone();
        let root = world.model.workspace.document_root().clone();
        let other_path = vec![Step::Key(gid::new_cell_id())];
        world.model.selection = Some(selection::bare_edge(&root, other_path.clone()));
        let mut edit = value_edit(root, vec![]);
        assert!(Rc::ptr_eq(&original, &world.model.doc));
        assert!(!(edit.write)(&mut world, f64::value(10.0)));
        assert!(Rc::ptr_eq(&original, &world.model.doc));
        assert!(!world.model.history.can_undo());
        (edit.selection)(&mut world, selection::payload::edge());
        assert_eq!(world.model.selection.as_ref().unwrap().path(), other_path);
        assert!((edit.write)(&mut world, f64::value(11.0)));
        assert!((edit.write)(&mut world, f64::value(12.0)));
        assert_eq!(world.model.selection.as_ref().unwrap().path(), other_path);
        assert!(world.model.step_history(true));
        assert!(Rc::ptr_eq(&original, &world.model.doc));
        assert!(!world.model.history.can_undo());
    }

    #[test]
    fn scrubbing_keeps_threshold_distance_precision_and_one_undo_run() {
        let mut model = model(f64::value(10.0));
        let original = model.model.doc.clone();
        let root = model.model.workspace.document_root().clone();
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
                    crate::display::ScrubUpdate {
                        value: f64::value(value),
                        spelling: Some(format!("{value:.1}")),
                    }
                })
            }),
        );
        assert!(!gesture.advance(&mut model, &[Point::new(102.0, 202.0)]));
        assert!(!model.model.history.can_undo());
        assert!(gesture.advance(&mut model, &[Point::new(108.0, 206.0)]));
        assert_eq!(model.model.doc.root, Some(f64::value(14.0)));
        assert!(gesture.advance(&mut model, &[Point::new(110.0, 206.0)]));
        assert_eq!(model.model.doc.root, Some(f64::value(15.0)));
        assert!(gesture.advance(&mut model, &[Point::new(-20.0, 206.0)]));
        assert_eq!(model.model.doc.root, Some(f64::value(-50.0)));
        assert!(gesture.advance(&mut model, &[Point::new(110.0, 206.0)]));
        assert_eq!(model.model.doc.root, Some(f64::value(15.0)));
        let presentation = gesture.scrub_spelling().unwrap();
        assert_eq!(presentation.root, &root);
        assert!(presentation.path.is_empty());
        assert_eq!(presentation.spelling, "15.0");
        assert!(model.model.step_history(true));
        assert_eq!(model.model.doc.root, original.root);
        assert!(!model.model.history.can_undo());
    }

    #[test]
    fn state_drags_use_total_logical_distance_and_only_update_their_view() {
        let mut model = model(Value::record([]));
        let root = model.model.workspace.document_root().clone();
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
                .model
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
    fn scrubbing_a_batch_preserves_the_precision_path_and_one_undo_step() {
        let mut model = model(f64::value(0.0));
        let root = model.model.workspace.document_root().clone();
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
                    crate::display::ScrubUpdate {
                        value: f64::value(value),
                        spelling: None,
                    }
                })
            }),
        );
        assert!(gesture.advance(&mut model, &samples));
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

    #[test]
    fn state_drag_gets_one_batch_including_threshold_excursions() {
        let mut model = model(Value::record([]));
        let root = model.model.workspace.document_root().clone();
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
        let sample = |x| crate::display::StateDragEvent {
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
        model.model.selection = Some(selection::bare_edge(&root, vec![]));
        let mut gesture = point(
            root.clone(),
            vec![],
            Rect::new(10.0, 20.0, 110.0, 120.0),
            Rc::new(|event| crate::display::PointUpdate {
                value: Value::list([f64::value(event.x), f64::value(event.y)]),
                selection: Some(selection::payload::edge()),
            }),
        );
        assert!(gesture.advance(&mut model, &[Point::new(30.0, 40.0)]));
        assert_eq!(
            model.model.doc.root,
            Some(Value::list([f64::value(0.2), f64::value(0.2)]))
        );
        assert!(gesture.advance(&mut model, &[Point::new(210.0, -20.0)]));
        assert_eq!(
            model.model.doc.root,
            Some(Value::list([f64::value(1.0), f64::value(0.0)]))
        );
        assert!(!gesture.advance(&mut model, &[Point::new(210.0, -20.0)]));
        assert_eq!(model.model.selection.as_ref().unwrap().root(), &root);
        assert!(model.model.step_history(true));
        assert_eq!(model.model.doc.root, original.root);
        assert!(!model.model.history.can_undo());
    }
}
