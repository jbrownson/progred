//! Whole editor frames written as SVG, without a window: examples, the CAM
//! preview while its computations run, and the website's images.

use super::*;
use crate::test_examples::{cam_controls_path, cam_editor, cam_position};
use gid::{CellId, Value};
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
        if runner.editor.computations.tasks.poll() {
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
        assert!(runner.editor.computations.tasks.poll());
    };
    capture(&mut runner, "cam_async_first.svg");
    complete(&mut runner);
    capture(&mut runner, "cam_async_ready.svg");
    let path = crate::workspace::declarations(runner.editor.model.doc.root.as_ref())[0]
        .path
        .clone();
    let annotations = &mut runner.editor.model.workspace.left.panes[0].view.annotations;
    annotations.set_field(&cam_controls_path(&path), STATE, Some(cam_position(0.7)));
    capture(&mut runner, "cam_async_updating.svg");
    assert_eq!(queue.lock().unwrap().len(), 1);
    complete(&mut runner);
    capture(&mut runner, "cam_async_updated.svg");
    assert!(queue.lock().unwrap().is_empty());
}

/// The home page's picture of the libraries example: its fractions and dials,
/// drawn by the projections the document declares, ahead of those
/// declarations.
#[test]
#[ignore = "regenerates the website's document-libraries image"]
fn website_libraries_image() {
    use crate::libraries::workspace::vocabulary::LIBRARIES;
    let (mut doc, _) = crate::gid_text::parse(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../examples/libraries.gid"
    )))
    .unwrap();
    let root = doc.root.take().unwrap();
    let fields = root.as_record().unwrap();
    let declarations = fields.get(&LIBRARIES).unwrap().clone();
    doc.root = Some(Value::record(
        fields
            .iter()
            .filter(|(key, _)| *key != LIBRARIES)
            .map(|(key, value)| (*key, value.clone()))
            .chain([(LIBRARIES, declarations)]),
    ));
    let mut editor = crate::test_editor(doc);
    editor.palette = crate::styles::Theme::Light.palette();
    editor.drawn_menu = false;
    render_editor(
        editor,
        kurbo::Size::new(704.0, 234.0),
        "website_libraries.svg",
    );
}

/// The home page's link preview: the forest drawn with the nine libraries it
/// uses.
#[test]
#[ignore = "regenerates the website's link preview image"]
fn website_preview_image() {
    use crate::libraries::{
        absent, color, control, controls, f64, grap, layout, presentation, sequence,
    };
    let (doc, fields) = crate::gid_text::parse(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../website/public/lessons/growing-forest.gid"
    )))
    .unwrap();
    let forest = [
        absent::ID,
        color::ID,
        control::ID,
        f64::ID,
        grap::ID,
        layout::ID,
        presentation::ID,
        sequence::ID,
        controls::ID,
    ]
    .map(|id| id.to_string())
    .join(",");
    let mut editor = crate::test_editor_with_stack(
        doc,
        crate::web_embed::peeled(
            Some(&forest),
            &forest,
            Some(&format!("{},{}", fields["second"], fields["first"])),
            true,
        )
        .unwrap(),
    );
    editor.palette = crate::styles::Theme::Light.palette();
    editor.drawn_menu = false;
    render_editor(
        editor,
        kurbo::Size::new(704.0, 600.0),
        "website_preview.svg",
    );
}
