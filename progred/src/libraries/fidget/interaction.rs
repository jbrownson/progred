//! Per-preview scheduling policy, shared by camera handlers and async rendering.

use crate::{computations::Computations, workspace::Root};
use gid::Step;
use incremental::{Input, Memo};
use puri::timer::{Duration, Instant, Timers};
use puri_widgets::debounce::Debounce;
use std::{cell::RefCell, rc::Rc};

const ZOOM_QUIET: Duration = Duration::from_millis(150);

pub(crate) struct Interaction {
    zoom: RefCell<Debounce>,
    quiet: Input<bool>,
    pub permitted: Memo<bool>,
}

impl Interaction {
    pub fn new(computations: &Computations) -> Self {
        let quiet = computations.runtime.input(true);
        let permitted = computations.runtime.memo({
            let quiet = quiet.clone();
            let pressed = computations.pointer_pressed.clone();
            move |read| Ok(!*pressed.read(read) && *quiet.read(read))
        });
        Self {
            zoom: RefCell::default(),
            quiet,
            permitted,
        }
    }

    pub fn at(computations: &Computations, view: &Root, path: &[Step]) -> Rc<Self> {
        let state = computations.at(view, path, || Self::new(computations));
        state.refresh(computations.frame_time.get());
        state
    }

    pub fn zoomed(&self, timers: &mut impl Timers, now: Instant) {
        self.zoom.borrow_mut().trigger(timers, now, ZOOM_QUIET);
        self.refresh(now);
    }

    pub fn refresh(&self, now: Instant) {
        // Time is sampled outside the memo; a missed delivery recovers on any frame.
        self.quiet.set(self.zoom.borrow().ready(now));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        EditorRunner,
        display::{Layout, partial, widget},
        projection::Projection,
    };
    use gid::{Cells, Document, Value};
    use kurbo::Size;
    use puri::handler::{PointerInfo, PointerState, PointerType, ScrollDelta};
    use ui_events::pointer::{
        PointerEvent, PointerGesture, PointerGestureEvent, PointerScrollEvent,
    };

    const VIEWPORT: Size = Size::new(500.0, 400.0);

    fn runner() -> EditorRunner {
        let mut editor = crate::test_editor(Document {
            root: Some(Value::record([])),
            cells: Cells::new(),
        });
        editor.stack.projection = Projection::new([partial(|input| {
            Some(super::super::interactive_volume(
                Layout::widget(Rc::new(|_| {
                    widget::leaf(
                        widget::Extent {
                            width: 100.0,
                            ascent: 100.0,
                            descent: 0.0,
                        },
                        |_, _| {},
                    )
                })),
                input,
            ))
        })]);
        let mut runner = EditorRunner::new(editor);
        runner.refresh_frame(1.0, VIEWPORT);
        runner
    }

    fn activity(runner: &EditorRunner) -> Rc<Interaction> {
        Interaction::at(
            &runner.editor.computations,
            runner.editor.model.workspace.document_root(),
            &[],
        )
    }

    fn permitted(runner: &EditorRunner, activity: &Interaction) -> bool {
        *runner
            .editor
            .computations
            .runtime
            .read(&activity.permitted)
            .unwrap()
    }

    fn pointer() -> PointerInfo {
        PointerInfo {
            pointer_id: None,
            persistent_device_id: None,
            pointer_type: PointerType::Mouse,
        }
    }

    fn scroll(runner: &mut EditorRunner, position: (f64, f64), delta: (f64, f64)) {
        runner.pointer_event(
            &PointerEvent::Scroll(PointerScrollEvent {
                pointer: pointer(),
                state: PointerState {
                    position: position.into(),
                    ..Default::default()
                },
                delta: ScrollDelta::PixelDelta(delta.into()),
            }),
            1.0,
            VIEWPORT,
        );
        runner.flush_pending_continuous();
    }

    #[test]
    fn only_changed_camera_input_starts_a_local_debounce() {
        let mut runner = runner();
        let activity = activity(&runner);
        let other = Interaction::at(
            &runner.editor.computations,
            runner.editor.model.workspace.document_root(),
            &[Step::Key(gid::new_cell_id())],
        );
        for (position, delta) in [((250.0, 250.0), (0.0, 10.0)), ((25.0, 25.0), (10.0, 0.0))] {
            scroll(&mut runner, position, delta);
            assert!(runner.editor.timers.deadline().is_none());
            assert!(permitted(&runner, &activity));
        }
        scroll(&mut runner, (25.0, 25.0), (0.0, 10.0));
        let deadline = runner
            .editor
            .timers
            .deadline()
            .expect("handled zoom schedules wakeup");
        assert!(!permitted(&runner, &activity));
        assert!(permitted(&runner, &other));
        assert!(runner.editor.timers.fire_due(deadline));
        runner.refresh_frame(1.0, VIEWPORT);
        assert!(permitted(&runner, &activity));
    }

    #[test]
    fn pinch_and_missing_timer_delivery_use_the_same_readiness() {
        let mut runner = runner();
        let activity = activity(&runner);
        runner.pointer_event(
            &PointerEvent::Gesture(PointerGestureEvent {
                pointer: pointer(),
                state: PointerState {
                    position: (25.0, 25.0).into(),
                    ..Default::default()
                },
                gesture: PointerGesture::Pinch(0.25),
            }),
            1.0,
            VIEWPORT,
        );
        runner.flush_pending_continuous();
        assert!(!permitted(&runner, &activity));

        // An elapsed constraint with no timer delivery must recover during a normal build.
        activity.zoomed(
            &mut runner.editor.timers,
            Instant::now() - Duration::from_secs(1),
        );
        runner.editor.timers = crate::timers::Timers::default();
        assert!(!permitted(&runner, &activity));
        runner.refresh_frame(1.0, VIEWPORT);
        assert!(permitted(&runner, &activity));
    }

    #[test]
    fn replacing_a_document_clears_its_wakeups_and_creates_fresh_interaction_state() {
        let mut runner = runner();
        let old = activity(&runner);
        scroll(&mut runner, (25.0, 25.0), (0.0, 10.0));
        let deadline = runner.editor.timers.deadline().unwrap();
        runner.adopt_model(
            Document {
                root: Some(Value::record([])),
                cells: Cells::new(),
            },
            None,
            Default::default(),
        );
        assert!(runner.editor.timers.deadline().is_none());
        assert!(!runner.editor.timers.fire_due(deadline));
        runner.refresh_frame(1.0, VIEWPORT);
        let new = activity(&runner);
        assert!(!Rc::ptr_eq(&old, &new));
        assert!(permitted(&runner, &new));
    }

    #[test]
    fn closing_the_editor_does_not_retain_its_interaction_state() {
        let mut runner = runner();
        scroll(&mut runner, (25.0, 25.0), (0.0, 10.0));
        let activity = Rc::downgrade(&activity(&runner));
        assert!(activity.upgrade().is_some());
        drop(runner);
        assert!(activity.upgrade().is_none());
    }
}
