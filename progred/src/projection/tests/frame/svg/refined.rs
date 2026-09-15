use super::*;
use crate::libraries::{
    controls::vocabulary::STATE, fidget::vocabulary as f, toolpath::vocabulary as t,
};
use incremental::background::{Executor, Job};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

#[test]
#[ignore = "captures automatic mesh fallback and implicit refinements through the full editor"]
fn editor_toolpath_refined_svg_captures() {
    let queue = Arc::new(Mutex::new(VecDeque::<Job>::new()));
    let (send, receive) = mpsc::channel();
    let mut editor = cam_editor(t::PREVIEW_REFINED);
    editor.computations = crate::computations::Computations::new(
        Executor::new({
            let queue = queue.clone();
            move |job| queue.lock().unwrap().push_back(job)
        }),
        {
            let send = send.clone();
            move || {
                let _ = send.send(false);
            }
        },
    );
    let path = crate::workspace::declarations(editor.model.doc.root.as_ref())[0]
        .path
        .clone();
    let mut runner = crate::EditorRunner::new(editor);
    let size = kurbo::Size::new(1000.0, 750.0);
    let capture = |runner: &mut crate::EditorRunner, name: &str| {
        let start = Instant::now();
        runner.refresh_frame(1.0, size);
        let paint = runner.prepare_paint(1.0, size);
        let mut list = DrawList::new();
        puri::frame::render(paint.renders, &mut list);
        eprintln!(
            "{name}: frame {:.2} ms",
            start.elapsed().as_secs_f64() * 1000.0
        );
        write_svg(
            &list,
            size.width,
            size.height,
            "#F6F6F8",
            &format!("{name}.svg"),
        );
    };
    let finish_mesh = |runner: &mut crate::EditorRunner, index| {
        let job = queue.lock().unwrap().remove(index).unwrap();
        let start = Instant::now();
        std::thread::spawn(job).join().unwrap();
        eprintln!(
            "mesh worker {:.2} ms",
            start.elapsed().as_secs_f64() * 1000.0
        );
        assert!(runner.editor.computations.tasks.poll());
    };
    let finish_image = |runner: &mut crate::EditorRunner, prefix: &str| {
        receive.try_iter().for_each(drop);
        let job = queue.lock().unwrap().pop_front().unwrap();
        let send = send.clone();
        let worker = std::thread::spawn(move || {
            job();
            send.send(true).unwrap();
        });
        let start = Instant::now();
        let mut updates = 0;
        loop {
            let finished = receive.recv_timeout(Duration::from_secs(60)).unwrap();
            if runner.editor.computations.tasks.poll() {
                updates += 1;
                eprintln!(
                    "{prefix} update {updates}: {:.2} ms",
                    start.elapsed().as_secs_f64() * 1000.0
                );
                capture(runner, &format!("{prefix}_{updates}"));
            }
            if finished {
                break;
            }
        }
        worker.join().unwrap();
        assert!(updates >= 1);
    };

    capture(&mut runner, "cam_refined_initial");
    assert_eq!(queue.lock().unwrap().len(), 2);
    finish_mesh(&mut runner, 0);
    capture(&mut runner, "cam_refined_mesh");
    finish_image(&mut runner, "cam_refined_image");
    assert!(queue.lock().unwrap().is_empty());

    runner.editor.model.workspace.left.panes[0]
        .view
        .annotations
        .set_field(
            &path,
            f::CAMERA,
            Some(Value::record([
                (f::YAW, crate::libraries::f32::value(55.0)),
                (f::PITCH, crate::libraries::f32::value(45.0)),
            ])),
        );
    capture(&mut runner, "cam_refined_orbit");
    assert_eq!(queue.lock().unwrap().len(), 1, "orbit requests no meshing");
    finish_image(&mut runner, "cam_refined_orbit_image");

    runner.editor.model.workspace.left.panes[0]
        .view
        .annotations
        .set_field(
            &path,
            STATE,
            Some(Value::record([(t::PROGRESS, f64::value(0.7))])),
        );
    capture(&mut runner, "cam_refined_playback_pending");
    assert_eq!(queue.lock().unwrap().len(), 2);
    finish_mesh(&mut runner, 0);
    capture(&mut runner, "cam_refined_playback_mesh");
    finish_image(&mut runner, "cam_refined_playback_image");
    assert!(queue.lock().unwrap().is_empty());
}
