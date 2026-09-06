//! Caller-owned continuations for projection gestures. The accepting
//! press installs one; it owns motion until release or cancellation.

use crate::model::Model;
use crate::selection::{self, Selection};
use crate::sources::Sources;
use crate::workspace::Root;
use gid::{Path, Step, Value};
use kurbo::{Point, Rect, Vec2};
use progred_display::{PointHandler, ScrubHandler, StateDragHandler};
use progred_libraries::Libraries;

pub(crate) trait Gesture {
    /// Returns whether the document changed, for the window's save indicator.
    fn advance(&mut self, model: &mut Model, libraries: &Libraries, point: Point) -> bool;

    fn scrub_spelling(&self) -> Option<ScrubSpelling<'_>> {
        None
    }
}

pub(crate) struct ScrubSpelling<'a> {
    pub root: &'a Root,
    pub path: &'a [Step],
    pub spelling: &'a str,
}

struct Drag {
    origin: Point,
    previous: Point,
    scale: f64,
    dragging: bool,
}

struct Motion {
    movement: Vec2,
    distance: Vec2,
}

impl Drag {
    fn new(origin: Point, scale: f64) -> Self {
        Self {
            origin,
            previous: origin,
            scale,
            dragging: false,
        }
    }

    fn advance(&mut self, point: Point) -> Option<Motion> {
        let distance = (point - self.origin) / self.scale;
        let movement = if self.dragging {
            (point - self.previous) / self.scale
        } else {
            distance
        };
        // Logical pixels; replace when the input adapter exposes the platform threshold.
        self.dragging |= distance.hypot() >= 3.0;
        self.previous = point;
        self.dragging.then_some(Motion { movement, distance })
    }
}

struct Scrub {
    drag: Drag,
    root: Root,
    path: Path,
    gesture: progred_display::ScrubGesture,
    recorded: bool,
    spelling: Option<String>,
}

pub(crate) fn scrub(
    origin: Point,
    scale: f64,
    root: Root,
    path: Path,
    handler: ScrubHandler,
) -> Box<dyn Gesture> {
    Box::new(Scrub {
        drag: Drag::new(origin, scale),
        root,
        path,
        gesture: handler(),
        recorded: false,
        spelling: None,
    })
}

impl Gesture for Scrub {
    fn advance(&mut self, model: &mut Model, libraries: &Libraries, point: Point) -> bool {
        self.drag.advance(point).is_some_and(|motion| {
            let update = (self.gesture)(progred_display::ScrubEvent {
                movement_x: motion.movement.x,
                distance_y: motion.distance.y,
            });
            self.spelling = update.spelling;
            write_value(
                model,
                libraries,
                &self.path,
                update.value,
                &mut self.recorded,
            )
        })
    }

    fn scrub_spelling(&self) -> Option<ScrubSpelling<'_>> {
        self.spelling.as_deref().map(|spelling| ScrubSpelling {
            root: &self.root,
            path: &self.path,
            spelling,
        })
    }
}

struct StateDrag {
    drag: Drag,
    root: Root,
    path: Path,
    gesture: progred_display::StateDragGesture,
}

pub(crate) fn state_drag(
    origin: Point,
    scale: f64,
    root: Root,
    path: Path,
    handler: StateDragHandler,
) -> Box<dyn Gesture> {
    Box::new(StateDrag {
        drag: Drag::new(origin, scale),
        root,
        path,
        gesture: handler(),
    })
}

impl Gesture for StateDrag {
    fn advance(&mut self, model: &mut Model, _: &Libraries, point: Point) -> bool {
        if let Some(motion) = self.drag.advance(point) {
            let state = (self.gesture)(progred_display::StateDragEvent {
                delta_x: motion.distance.x,
                delta_y: motion.distance.y,
            });
            if let Some(view) = model.workspace.view_mut(&self.root)
                && view.annotations.at(&self.path) != Some(&state)
            {
                view.annotations.set(&self.path, Some(state));
            }
        }
        false
    }
}

struct PointControl {
    root: Root,
    path: Path,
    rect: Rect,
    handler: PointHandler,
    recorded: bool,
}

pub(crate) fn point(root: Root, path: Path, rect: Rect, handler: PointHandler) -> Box<dyn Gesture> {
    Box::new(PointControl {
        root,
        path,
        rect,
        handler,
        recorded: false,
    })
}

impl Gesture for PointControl {
    fn advance(&mut self, model: &mut Model, libraries: &Libraries, point: Point) -> bool {
        let update = (self.handler)(progred_display::PointEvent {
            x: ((point.x - self.rect.x0) / self.rect.width()).clamp(0.0, 1.0),
            y: ((point.y - self.rect.y0) / self.rect.height()).clamp(0.0, 1.0),
        });
        let wrote = write_value(
            model,
            libraries,
            &self.path,
            update.value,
            &mut self.recorded,
        );
        if let Some(payload) = update.selection
            && let Some(recorded) = model
                .selection
                .as_ref()
                .filter(|selection| selection.root() == &self.root && selection.path() == self.path)
                .map(Selection::recorded)
        {
            let mut next = Selection::from_payload(
                &self.root,
                &Sources {
                    doc: &model.doc,
                    libraries,
                },
                self.path.clone(),
                payload,
            );
            next.preserve_recorded(recorded);
            model.selection = Some(next);
        }
        wrote
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
        let before = model.doc.clone();
        let wrote = selection::set_value(&mut model.doc, libraries, path, replacement);
        if wrote && !*recorded {
            model.history.record(before, Some(path.to_vec()));
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

    fn model(value: Value) -> Model {
        Model {
            doc: gid::Document {
                root: Some(value),
                cells: gid::Cells::new(),
            },
            selection: None,
            history: Default::default(),
            view: Default::default(),
            workspace: Default::default(),
        }
    }

    #[test]
    fn scrubbing_keeps_threshold_distance_precision_and_one_undo_run() {
        let mut model = model(f64::value(10.0));
        let original = model.doc.clone();
        let root = model.workspace.document_root().clone();
        let libraries = Libraries::default();
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
        assert!(!gesture.advance(&mut model, &libraries, Point::new(102.0, 202.0)));
        assert!(!model.history.can_undo());
        assert!(gesture.advance(&mut model, &libraries, Point::new(108.0, 206.0)));
        assert_eq!(model.doc.root, Some(f64::value(14.0)));
        assert!(gesture.advance(&mut model, &libraries, Point::new(110.0, 206.0)));
        assert_eq!(model.doc.root, Some(f64::value(15.0)));
        assert!(gesture.advance(&mut model, &libraries, Point::new(-20.0, 206.0)));
        assert_eq!(model.doc.root, Some(f64::value(-50.0)));
        assert!(gesture.advance(&mut model, &libraries, Point::new(110.0, 206.0)));
        assert_eq!(model.doc.root, Some(f64::value(15.0)));
        let presentation = gesture.scrub_spelling().unwrap();
        assert_eq!(presentation.root, &root);
        assert!(presentation.path.is_empty());
        assert_eq!(presentation.spelling, "15.0");
        assert_eq!(
            model.history.undo(model.doc.clone(), None).unwrap().0.root,
            original.root
        );
        assert!(!model.history.can_undo());
    }

    #[test]
    fn state_drags_use_total_logical_distance_and_only_update_their_view() {
        let mut model = model(Value::record([]));
        let root = model.workspace.document_root().clone();
        let path = vec![Step::Key(gid::new_cell_id())];
        let libraries = Libraries::default();
        let mut gesture = state_drag(
            Point::ZERO,
            2.0,
            root.clone(),
            path.clone(),
            Rc::new(|| {
                Box::new(|event| {
                    Value::list([f64::value(event.delta_x), f64::value(event.delta_y)])
                })
            }),
        );
        gesture.advance(&mut model, &libraries, Point::new(5.0, 0.0));
        assert!(
            model
                .workspace
                .view(&root)
                .unwrap()
                .annotations
                .at(&path)
                .is_none()
        );
        gesture.advance(&mut model, &libraries, Point::new(8.0, 12.0));
        gesture.advance(&mut model, &libraries, Point::new(10.0, 12.0));
        assert_eq!(
            model.workspace.view(&root).unwrap().annotations.at(&path),
            Some(&Value::list([f64::value(5.0), f64::value(6.0)]))
        );
        assert_eq!(model.doc.root, Some(Value::record([])));
        assert!(!model.history.can_undo());
    }

    #[test]
    fn point_controls_clamp_unbounded_motion_and_coalesce_their_writes() {
        let mut model = model(f64::value(0.0));
        let original = model.doc.clone();
        let root = model.workspace.document_root().clone();
        model.selection = Some(selection::bare_edge(&root, vec![]));
        let libraries = Libraries::default();
        let mut gesture = point(
            root.clone(),
            vec![],
            Rect::new(10.0, 20.0, 110.0, 120.0),
            Rc::new(|event| progred_display::PointUpdate {
                value: Value::list([f64::value(event.x), f64::value(event.y)]),
                selection: Some(selection::payload::edge()),
            }),
        );
        assert!(gesture.advance(&mut model, &libraries, Point::new(30.0, 40.0)));
        assert_eq!(
            model.doc.root,
            Some(Value::list([f64::value(0.2), f64::value(0.2)]))
        );
        assert!(gesture.advance(&mut model, &libraries, Point::new(210.0, -20.0)));
        assert_eq!(
            model.doc.root,
            Some(Value::list([f64::value(1.0), f64::value(0.0)]))
        );
        assert!(!gesture.advance(&mut model, &libraries, Point::new(210.0, -20.0)));
        assert_eq!(model.selection.as_ref().unwrap().root(), &root);
        assert_eq!(
            model.history.undo(model.doc.clone(), None).unwrap().0.root,
            original.root
        );
        assert!(!model.history.can_undo());
    }
}
