use super::*;
use crate::{
    EditorRunner,
    display::{Layout, partial, widget},
    projection::Projection,
};
use gid::{Cells, Document, Step, Value};
use kurbo::Size;
use ui_events::ScrollDelta;
use ui_events::pointer::{
    PointerEvent, PointerGesture, PointerGestureEvent, PointerInfo, PointerScrollEvent,
    PointerState,
};

const VIEWPORT: Size = Size::new(500.0, 400.0);

fn runner() -> EditorRunner {
    let mut editor = crate::editor_of(Value::record([]));
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

/// The interaction of the occurrence at `path` in the document's view.
fn at(runner: &EditorRunner, path: Vec<Step>) -> Rc<Interaction> {
    let site = crate::editing::Site::new(crate::occurrence::of(&path).into());
    Interaction::at(&runner.editor.computations, &site)
}

fn activity(runner: &EditorRunner) -> Rc<Interaction> {
    at(runner, Vec::new())
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
    crate::display::test_support::mouse()
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
    let other = at(&runner, vec![Step::Key(gid::new_cell_id())]);
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
