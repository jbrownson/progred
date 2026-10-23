use super::*;
use crate::display::widget::view::{Area, ViewId};
use crate::libraries::{
    controls::vocabulary::STATE, fidget::vocabulary as f, toolpath::vocabulary as t,
};
use crate::test_examples::{cam_editor, cam_position, result_path};
use gid::Value;
use incremental::background::{Executor, Job};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, mpsc},
    time::{Duration, Instant},
};

#[test]
#[ignore = "regenerates README screenshots, including the completed progressive CAM render"]
fn readme_svg_captures() {
    let size = kurbo::Size::new(1200.0, 900.0);
    let (doc, _) = crate::gid_text::parse(crate::command::Example::IopTree.source()).unwrap();
    let editor = crate::test_editor(doc);
    render_editor(editor, size, "readme_iop.svg");

    let mut editor = cam_editor(t::PREVIEW_REFINED);
    let path = crate::workspace::panes(editor.model.doc.root.as_ref()).remove(0);
    let view = editor
        .model
        .workspace
        .view_mut(&Area::part(ViewId::Document, path.clone()))
        .unwrap();
    view.annotations
        .set_field(&result_path(&path), STATE, Some(cam_position(0.3)));
    view.annotations.set_field(
        &[result_path(&path), result_path(&[])].concat(),
        f::CAMERA,
        Some(Value::record([
            (f::YAW, crate::libraries::f32::value(30.0)),
            (f::PITCH, crate::libraries::f32::value(45.0)),
            (f::ZOOM, crate::libraries::f32::value(1.7)),
        ])),
    );
    // Run mesh preparation, then every implicit pass before the screenshot. This
    // runs every implicit refinement (including final depth refinement), not
    // just the first published image or the mesh fallback.
    let queue = Arc::new(Mutex::new(VecDeque::<Job>::new()));
    editor.computations = crate::computations::Computations::new(
        Executor::new({
            let queue = queue.clone();
            move |job| queue.lock().unwrap().push_back(job)
        }),
        || eprintln!("README render: background result published"),
    );
    let mut runner = crate::EditorRunner::new(editor);
    runner.refresh_frame(1.0, size);
    assert_eq!(queue.lock().unwrap().len(), 1);
    loop {
        let job = queue.lock().unwrap().pop_front();
        let Some(job) = job else { break };
        let start = Instant::now();
        std::thread::spawn(job).join().unwrap();
        eprintln!(
            "README render: worker finished in {:.2}s",
            start.elapsed().as_secs_f64()
        );
        assert!(runner.editor.computations.poll());
        runner.refresh_frame(1.0, size);
    }
    let paint = runner.prepare_paint(1.0, size);
    let mut list = DrawList::new();
    puri::frame::render(paint.renders, &mut list);
    assert!(queue.lock().unwrap().is_empty());
    assert!(!runner.editor.computations.poll());
    write_svg(&list, size.width, size.height, "readme_cam.svg");
}

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
    let path = crate::workspace::panes(editor.model.doc.root.as_ref()).remove(0);
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
        write_svg(&list, size.width, size.height, &format!("{name}.svg"));
    };
    let finish_mesh = |runner: &mut crate::EditorRunner, index| {
        let job = queue.lock().unwrap().remove(index).unwrap();
        let start = Instant::now();
        std::thread::spawn(job).join().unwrap();
        eprintln!(
            "mesh worker {:.2} ms",
            start.elapsed().as_secs_f64() * 1000.0
        );
        assert!(runner.editor.computations.poll());
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
            if runner.editor.computations.poll() {
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
    assert_eq!(queue.lock().unwrap().len(), 1);
    finish_mesh(&mut runner, 0);
    capture(&mut runner, "cam_refined_mesh");
    finish_image(&mut runner, "cam_refined_image");
    assert!(queue.lock().unwrap().is_empty());

    runner
        .editor
        .model
        .workspace
        .view_mut(&Area::part(ViewId::Document, path.clone()))
        .unwrap()
        .annotations
        .set_field(
            &[result_path(&path), result_path(&[])].concat(),
            f::CAMERA,
            Some(Value::record([
                (f::YAW, crate::libraries::f32::value(55.0)),
                (f::PITCH, crate::libraries::f32::value(45.0)),
            ])),
        );
    capture(&mut runner, "cam_refined_orbit");
    assert_eq!(queue.lock().unwrap().len(), 1, "orbit requests no meshing");
    finish_image(&mut runner, "cam_refined_orbit_image");

    runner
        .editor
        .model
        .workspace
        .view_mut(&Area::part(ViewId::Document, path.clone()))
        .unwrap()
        .annotations
        .set_field(&result_path(&path), STATE, Some(cam_position(0.7)));
    capture(&mut runner, "cam_refined_playback_pending");
    assert_eq!(queue.lock().unwrap().len(), 1);
    finish_mesh(&mut runner, 0);
    capture(&mut runner, "cam_refined_playback_mesh");
    finish_image(&mut runner, "cam_refined_playback_image");
    assert!(queue.lock().unwrap().is_empty());
}
