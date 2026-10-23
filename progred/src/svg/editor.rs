//! Whole editor frames written as SVG, without a window: examples and the CAM
//! preview while its computations run.

use super::*;
use crate::display::widget::view::Area;
use crate::test_examples::{cam_editor, cam_position, result_path};
use gid::CellId;
#[test]
#[ignore = "writes full-editor captures, including Fidget rasterization"]
fn editor_svg_captures() {
    use crate::command::Example;
    for (source, file) in [
        (Example::Fidget.source(), "editor_fidget.svg"),
        (crate::test_examples::CUBE, "editor_fidget_cube.svg"),
        (Example::Toolpaths.source(), "editor_toolpaths.svg"),
    ] {
        let (doc, _) = crate::gid_text::parse(source).unwrap();
        render_editor(
            crate::test_editor(doc),
            kurbo::Size::new(1200.0, 900.0),
            file,
        );
    }
}

#[test]
#[ignore = "writes a full-editor capture of the mesh viewport"]
fn editor_mesh_svg_capture() {
    let (doc, _) = crate::gid_text::parse(crate::test_examples::CUBE).unwrap();
    render_editor(
        crate::test_editor(doc),
        kurbo::Size::new(1200.0, 900.0),
        "editor_fidget_mesh.svg",
    );
}

#[test]
#[ignore = "writes a full-editor capture of the meshed model and streamed toolpaths"]
fn editor_toolpath_mesh_svg_capture() {
    render_editor(
        cam_editor(crate::libraries::toolpath::vocabulary::PREVIEW_MESH),
        kurbo::Size::new(1500.0, 1050.0),
        "editor_toolpath_mesh.svg",
    );
}

#[test]
#[ignore = "captures mesh CAM's first pending frame and retained stock during an async update"]
fn editor_toolpath_async_svg_captures() {
    capture_cam_async(crate::libraries::toolpath::vocabulary::PREVIEW_MESH);
}

#[test]
#[ignore = "captures async implicit CAM using the software renderer"]
fn editor_toolpath_implicit_async_svg_captures() {
    capture_cam_async(crate::libraries::toolpath::vocabulary::PREVIEW_3D);
}

#[test]
#[ignore = "captures successive implicit refinements through the full editor, without a window"]
fn editor_toolpath_progressive_svg_captures() {
    use incremental::background::Executor;
    use std::{
        sync::mpsc,
        time::{Duration, Instant},
    };

    fn image_sizes(commands: &[DrawCmd], sizes: &mut Vec<(u32, u32)>) {
        sizes.extend(
            crate::display::test_support::painted(commands).filter_map(|command| match command {
                DrawCmd::Image { image, .. } => Some((image.width, image.height)),
                DrawCmd::Mesh { scene, .. } => Some((scene.view.width, scene.view.height)),
                _ => None,
            }),
        );
    }

    let mut editor = cam_editor(crate::libraries::toolpath::vocabulary::PREVIEW_3D);
    let (send, receive) = mpsc::channel();
    editor.computations = crate::computations::Computations::new(
        Executor::new({
            let send = send.clone();
            move |job| {
                let send = send.clone();
                std::thread::spawn(move || {
                    job();
                    send.send(true).unwrap();
                });
            }
        }),
        move || {
            send.send(false).unwrap();
        },
    );
    let mut runner = crate::EditorRunner::new(editor);
    let size = kurbo::Size::new(1000.0, 750.0);
    let start = Instant::now();
    runner.refresh_frame(1.0, size);
    let mut seen = Vec::new();
    loop {
        let finished = receive.recv_timeout(Duration::from_secs(60)).unwrap();
        if runner.editor.computations.poll() {
            runner.refresh_frame(1.0, size);
            let paint = runner.prepare_paint(1.0, size);
            let mut list = DrawList::new();
            puri::frame::render(paint.renders, &mut list);
            let mut pixels = Vec::new();
            image_sizes(&list.0, &mut pixels);
            assert_eq!(pixels.len(), 1);
            seen.push(pixels[0]);
            eprintln!(
                "implicit update {} {:?}: {:.2} ms",
                seen.len(),
                pixels[0],
                start.elapsed().as_secs_f64() * 1000.0
            );
            write_svg(
                &list,
                size.width,
                size.height,
                &format!("cam_progressive_{}.svg", seen.len()),
            );
        }
        if finished {
            break;
        }
    }
    assert!(
        seen.len() >= 2,
        "the editor must display intermediate results"
    );
    for pair in seen.windows(2) {
        assert!(pair[0].0 <= pair[1].0 && pair[0].1 <= pair[1].1);
    }
    assert_eq!(
        seen.last().unwrap().1,
        size.height as u32,
        "the final raster fills the pane, including behind its controls"
    );
}

fn capture_cam_async(mode: CellId) {
    use crate::libraries::controls::vocabulary::STATE;
    use incremental::background::{Executor, Job};
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
    };

    let mut editor = cam_editor(mode);
    let queue = Arc::new(Mutex::new(VecDeque::<Job>::new()));
    editor.computations = crate::computations::Computations::new(
        Executor::new({
            let queue = queue.clone();
            move |job| queue.lock().unwrap().push_back(job)
        }),
        || {},
    );
    let mut runner = crate::EditorRunner::new(editor);
    let size = kurbo::Size::new(1000.0, 750.0);
    let capture = |runner: &mut crate::EditorRunner, file| {
        let start = std::time::Instant::now();
        runner.refresh_frame(1.0, size);
        let paint = runner.prepare_paint(1.0, size);
        let mut list = DrawList::new();
        puri::frame::render(paint.renders, &mut list);
        eprintln!(
            "{file}: frame {:.2} ms",
            start.elapsed().as_secs_f64() * 1000.0
        );
        write_svg(&list, size.width, size.height, file);
    };
    let complete = |runner: &mut crate::EditorRunner| {
        let job = queue.lock().unwrap().pop_front().unwrap();
        let start = std::time::Instant::now();
        std::thread::spawn(job).join().unwrap();
        eprintln!(
            "preview worker {:.2} ms",
            start.elapsed().as_secs_f64() * 1000.0
        );
        assert!(runner.editor.computations.poll());
    };
    capture(&mut runner, "cam_async_first.svg");
    complete(&mut runner);
    capture(&mut runner, "cam_async_ready.svg");
    let path = crate::workspace::panes(runner.editor.model.doc.root.as_ref()).remove(0);
    let annotations = &mut runner
        .editor
        .model
        .workspace
        .pane_mut(&Area::part(crate::test_document(), path.clone()))
        .unwrap()
        .annotations;
    annotations.set_field(&result_path(&path), STATE, Some(cam_position(0.7)));
    capture(&mut runner, "cam_async_updating.svg");
    assert_eq!(queue.lock().unwrap().len(), 1);
    complete(&mut runner);
    capture(&mut runner, "cam_async_updated.svg");
    assert!(queue.lock().unwrap().is_empty());
}
