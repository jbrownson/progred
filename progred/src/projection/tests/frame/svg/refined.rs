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
#[ignore = "captures the CAM outline without launching the app"]
fn editor_cam_outline_svg_captures() {
    let (doc, names) = crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    // Source-only views: the same projection, with sections folded through
    // ordinary view state. No special screenshot or outline state in production.
    for (expanded, file) in [
        ("settings_section", "cam_outline_overview.svg"),
        ("tools_section", "cam_outline_tools.svg"),
    ] {
        let mut editor = crate::test_editor(doc.clone());
        for key in [
            "settings_section",
            "geometry_section",
            "tools_section",
            "operations_section",
            "strategies_section",
            "helpers_section",
        ] {
            if key != expanded {
                editor.collapse(&crate::test_root(), &[Step::Key(names[key])], Some(true));
            }
        }
        // Project only the document here; its pane data remains present.
        let (bench, extent) = place_with_annotations(
            &editor.model.doc,
            None,
            &editor.model.workspace.document.annotations,
            780.0,
            None,
            None,
            None,
        );
        write_svg(&bench.list, 780.0, extent.height() + 48.0, "#F6F6F8", file);
    }
    render_editor(
        cam_editor(t::PREVIEW_MESH),
        kurbo::Size::new(1200.0, 1000.0),
        "cam_outline_editor.svg",
    );
}

#[test]
#[ignore = "regenerates README screenshots, including the completed progressive CAM render"]
fn readme_svg_captures() {
    let size = kurbo::Size::new(1200.0, 900.0);
    let (doc, _) = crate::gid_text::parse(crate::command::Example::IopTree.source()).unwrap();
    let mut editor = crate::test_editor(doc);
    editor.model.workspace.left_width = 0.5;
    render_editor(editor, size, "readme_iop.svg");

    let mut editor = cam_editor(t::PREVIEW_REFINED);
    editor.model.workspace.left_width = 0.5;
    let path = crate::workspace::declarations(editor.model.doc.root.as_ref())[0]
        .path
        .clone();
    let view = &mut editor.model.workspace.left.panes[0].view;
    view.annotations.set_field(
        &result_path(&path),
        STATE,
        Some(Value::record([(t::PROGRESS, f64::value(0.3))])),
    );
    view.annotations.set_field(
        &result_path(&result_path(&path)),
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
        assert!(runner.editor.computations.tasks.poll());
        runner.refresh_frame(1.0, size);
    }
    let paint = runner.prepare_paint(1.0, size);
    let mut list = DrawList::new();
    puri::frame::render(paint.renders, &mut list);
    assert!(queue.lock().unwrap().is_empty());
    assert!(!runner.editor.computations.tasks.poll());
    write_svg(&list, size.width, size.height, "#F6F6F8", "readme_cam.svg");
}

#[test]
#[ignore = "captures editable tool profiles without launching the app"]
fn editor_tool_profiles_svg_captures() {
    use crate::libraries::toolpath::cutter::Tool;
    let (mut doc, names) =
        crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let libraries = crate::stack::load().libraries;
    let sources = crate::sources::Sources {
        doc: &doc,
        libraries: &libraries,
    };
    let square = ::grap::evaluate(&Value::from(names["square_tool"]), &sources, 1000).result;
    assert!(Tool::read(&square).is_some());
    for (name, tool) in [
        ("square", square),
        ("ball", Tool::ball(0.125, 0.22).unwrap().value()),
        ("bull", Tool::bull(0.125, 0.02, 0.22).unwrap().value()),
    ] {
        doc.root = Some(tool);
        render(&doc, None, 800.0, &format!("tool_profile_{name}.svg"));
    }
    doc.root = Some(Value::record([(
        ::grap::vocabulary::EVALUATE,
        Value::Cell(names["ball_tool"]),
    )]));
    for width in [600.0, 1000.0, 2000.0] {
        render(
            &doc,
            None,
            width,
            &format!("tool_profile_evaluated_{width}.svg"),
        );
    }
    let selected = Selection::edge(&crate::test_root(), Vec::new());
    render(
        &doc,
        Some(&selected),
        600.0,
        "tool_profile_evaluated_selected.svg",
    );
}

#[test]
#[ignore = "captures both tilted CAM operations without launching the app"]
fn editor_toolpath_operations_svg_captures() {
    fn has_image(commands: &[DrawCmd]) -> bool {
        commands.iter().any(|command| match command {
            DrawCmd::Image { .. } => true,
            DrawCmd::Clip { children, .. } => has_image(children),
            _ => false,
        })
    }
    for (progress, pitch, file) in [
        (0.3, 45.0, "cam_operations_op1.svg"),
        (0.93, 135.0, "cam_operations_op2.svg"),
        (1.0, 135.0, "cam_operations_complete.svg"),
    ] {
        let mut editor = cam_editor(t::PREVIEW_MESH);
        let path = crate::workspace::declarations(editor.model.doc.root.as_ref())[0]
            .path
            .clone();
        editor.model.workspace.left.panes[0]
            .view
            .annotations
            .set_field(
                &result_path(&path),
                STATE,
                Some(Value::record([(t::PROGRESS, f64::value(progress))])),
            );
        editor.model.workspace.left.panes[0]
            .view
            .annotations
            .set_field(
                &result_path(&result_path(&path)),
                f::CAMERA,
                Some(Value::record([
                    (f::YAW, crate::libraries::f32::value(30.0)),
                    (f::PITCH, crate::libraries::f32::value(pitch)),
                ])),
            );
        let start = Instant::now();
        let size = kurbo::Size::new(1200.0, 900.0);
        let mut runner = crate::EditorRunner::new(editor);
        let paint = runner.prepare_paint(1.0, size);
        let mut list = DrawList::new();
        puri::frame::render(paint.renders, &mut list);
        assert!(
            has_image(&list.0),
            "the combined operations must render, not show a fuel/geometry absent"
        );
        write_svg(&list, size.width, size.height, "#F6F6F8", file);
        eprintln!("{file}: {:.2}s", start.elapsed().as_secs_f64());
    }
}

#[test]
#[ignore = "captures controls over zoomed-in CAM geometry in both renderers"]
fn editor_toolpath_controls_overlay_svg_captures() {
    for (mode, file) in [
        (t::PREVIEW_MESH, "cam_controls_overlay_mesh.svg"),
        (t::PREVIEW_3D, "cam_controls_overlay_implicit.svg"),
    ] {
        let mut editor = cam_editor(mode);
        let path = crate::workspace::declarations(editor.model.doc.root.as_ref())[0]
            .path
            .clone();
        editor.model.workspace.left.panes[0]
            .view
            .annotations
            .set_field(
                &result_path(&result_path(&path)),
                f::CAMERA,
                Some(Value::record([(
                    f::ZOOM,
                    crate::libraries::f32::value(4.0),
                )])),
            );
        render_editor(editor, kurbo::Size::new(1000.0, 750.0), file);
    }
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
    assert_eq!(queue.lock().unwrap().len(), 1);
    finish_mesh(&mut runner, 0);
    capture(&mut runner, "cam_refined_mesh");
    finish_image(&mut runner, "cam_refined_image");
    assert!(queue.lock().unwrap().is_empty());

    runner.editor.model.workspace.left.panes[0]
        .view
        .annotations
        .set_field(
            &result_path(&result_path(&path)),
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
            &result_path(&path),
            STATE,
            Some(Value::record([(t::PROGRESS, f64::value(0.7))])),
        );
    capture(&mut runner, "cam_refined_playback_pending");
    assert_eq!(queue.lock().unwrap().len(), 1);
    finish_mesh(&mut runner, 0);
    capture(&mut runner, "cam_refined_playback_mesh");
    finish_image(&mut runner, "cam_refined_playback_image");
    assert!(queue.lock().unwrap().is_empty());
}
