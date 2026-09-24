use super::*;
use crate::hover::{Hover, SourceTrace};
use crate::libraries::{controls, presentation};
use ui_events::keyboard::Modifiers;
use ui_events::pointer::{PointerEvent, PointerInfo, PointerState, PointerUpdate};

const SIZE: kurbo::Size = kurbo::Size::new(720.0, 684.0);

fn paint(runner: &mut crate::EditorRunner) {
    let mut output = DrawList::default();
    puri::frame::render(runner.prepare_paint(1.0, SIZE).renders, &mut output);
    std::hint::black_box(&output);
    runner.frame_presented();
}

fn measure(label: &str, mut frame: impl FnMut(usize)) {
    let mut samples = Vec::new();
    for index in 0..iterations() + 5 {
        let start = Instant::now();
        frame(index);
        if index >= 5 {
            samples.push(start.elapsed());
        }
    }
    distribution(label, samples.into_iter());
}

/// Complete successor frames for changing slider values, not slider hit testing
/// or GPU rendering. Annotation updates match the tutorial's control state.
#[test]
#[ignore = "opt-in headless frame profile"]
fn forest_slider_profile_loop() {
    let (editor, names) = super::super::svg::website_growing_forest_editor();
    let mut runner = crate::EditorRunner::new(editor);
    measure("forest slider-value frame", |index| {
        runner
            .editor
            .model
            .workspace
            .document
            .annotations
            .set_field(
                &[
                    Step::Key(names["second"]),
                    Step::Key(presentation::vocabulary::RESULT),
                ],
                controls::vocabulary::STATE,
                Some(crate::libraries::f64::value((index % 101) as f64 / 100.0)),
            );
        runner.refresh_frame(1.0, SIZE);
        paint(&mut runner);
    });
}

/// Real pointer dispatch against installed geometry, source linking, successor
/// build, and headless painting. No app window or platform renderer is needed.
#[test]
#[ignore = "opt-in headless frame profile"]
fn forest_source_hover_profile_loop() {
    let (editor, names) = super::super::svg::website_growing_forest_editor();
    let mut runner = crate::EditorRunner::new(editor);
    let modifiers = if cfg!(target_os = "macos") {
        Modifiers::META
    } else {
        Modifiers::CONTROL
    };
    runner.editor.modifiers = modifiers;
    runner.refresh_frame(1.0, SIZE);
    paint(&mut runner);
    let point = (20..300).step_by(10).find_map(|y| {
        (20..690).step_by(10).find_map(|x| {
            let point = Point::new(x as f64, y as f64);
            let (_, claim) = runner.frame.dispatch.hover_geometry.probe(Some(point), None, 0.0)?;
            let Claim::Direct(Hovered::Tree(Hover::Calls(calls))) = claim else { return None; };
            calls.sources().any(|source| matches!(source, SourceTrace::InCell { cell, .. } if cell == names["growing_tree"])).then_some(point)
        })
    }).expect("a tree shape has a captured call chain");
    measure("forest source-hover event and frame", |index| {
        let point = if index % 2 == 0 {
            point
        } else {
            Point::new(1.0, 1.0)
        };
        runner.pointer_event(
            &PointerEvent::Move(PointerUpdate {
                pointer: PointerInfo {
                    pointer_id: None,
                    persistent_device_id: None,
                    pointer_type: ui_events::pointer::PointerType::Mouse,
                },
                current: PointerState {
                    position: (point.x, point.y).into(),
                    modifiers,
                    ..Default::default()
                },
                coalesced: Vec::new(),
                predicted: Vec::new(),
            }),
            1.0,
            SIZE,
        );
        paint(&mut runner);
    });
}
