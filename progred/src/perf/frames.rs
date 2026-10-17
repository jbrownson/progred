//! Opt-in, headless frame canaries. See docs/performance.md for scope and commands.

use crate::annotations::Annotations;
use crate::command::Example;
use crate::frame::Hovered;
use crate::perf::{distribution, iterations};
use crate::projection::Projection;
use crate::projection::test_support::{Bench, BenchContext, FrameTimes, ProfileView};
use crate::selection::Selection;
use crate::sources::Sources;
use gid::{Document, Step, Value};
use peniko::{Brush, ImageData};
use puri::draw::{DrawCmd, DrawList};
use std::rc::Rc;
use std::time::{Duration, Instant};

mod forest;
#[cfg(feature = "layout-profile")]
mod forms;

#[derive(Clone, Copy)]
struct Timing {
    total: Duration,
    phases: FrameTimes,
    disposal: Duration,
}

/// The caller supplies ordinary document/selection/annotation inputs for each
/// frame. Keep app-lifetime resources outside the closure, as the app does.
fn profile(name: &str, mut frame: impl FnMut(usize) -> Bench, check: impl Fn(&Bench)) {
    let count = iterations();
    let warmup = 5;
    let timings: Vec<_> = (0..count + warmup)
        .map(|index| {
            let start = Instant::now();
            let bench = frame(index);
            std::hint::black_box(&bench.list);
            let build = start.elapsed();
            // Validation is outside the timer; disposal is inside it.
            check(&bench);
            let phases = bench.times;
            let cleanup = Instant::now();
            drop(bench);
            let disposal = cleanup.elapsed();
            Timing {
                total: build + disposal,
                phases,
                disposal,
            }
        })
        .collect();
    eprintln!(
        "{name}: first frame {:.2?}; {warmup} warm-up, {count} measured frames",
        timings[0].total,
    );
    let warm = &timings[warmup..];
    distribution("frame + disposal", warm.iter().map(|t| t.total));
    distribution("prepare", warm.iter().map(|t| t.phases.prepare));
    distribution(
        "choices + settled geometry",
        warm.iter().map(|t| t.phases.choices),
    );
    distribution("placement + hover", warm.iter().map(|t| t.phases.placement));
    distribution("after-hover binding", warm.iter().map(|t| t.phases.hover));
    distribution(
        "paint + handler disposal",
        warm.iter().map(|t| t.phases.paint),
    );
    distribution("output disposal", warm.iter().map(|t| t.disposal));
    crate::perf::record("", warm.iter().map(|t| t.total));
}

fn fixture(source: &str) -> Document {
    crate::gid_text::parse(source)
        .expect("profile document parses")
        .0
}

fn first_viewport(doc: &Document) -> gid::Path {
    crate::workspace::declarations(doc.root.as_ref())
        .into_iter()
        .next()
        .expect("example declares a viewport")
        .path
}

fn iop_tree() -> Document {
    fixture(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../examples/iop-tree.gid"
    )))
}

#[test]
#[ignore]
fn iop_tree_profile_loop() {
    let doc = iop_tree();
    let view = ProfileView {
        size: kurbo::Size::new(500.0, 500.0),
        scale: 1.0,
        root: Some(first_viewport(&doc)),
    };
    let (view, mut context) = view.prepare(&doc);
    profile(
        "IoP picture, 500x500 @1",
        |_| context.frame(view.frame(&doc, &Annotations::default())).0,
        |bench| assert!(!bench.list.0.is_empty()),
    );
}

#[test]
#[ignore]
fn iop_tree_source_profile_loop() {
    let doc = iop_tree();
    let view = ProfileView {
        size: kurbo::Size::new(1400.0, 900.0),
        scale: 1.0,
        root: None,
    };
    let (view, mut context) = view.prepare(&doc);
    profile(
        "IoP source, 1400x900 @1",
        |_| context.frame(view.frame(&doc, &Annotations::default())).0,
        |bench| assert!(!bench.list.0.is_empty()),
    );
}

fn color_picker_fixture() -> (Document, Value) {
    use crate::libraries::{color, f64};
    (
        Document {
            root: Some(color::value(puri::Color::from_rgba8(
                0xb4, 0xe0, 0xfe, 0x99,
            ))),
            cells: gid::Cells::new(),
        },
        Value::record([(
            color::vocabulary::PICKER,
            Value::record([(color::vocabulary::HUE, f64::value(0.57))]),
        )]),
    )
}

fn color_picker_frame() -> impl FnMut() -> Bench {
    let (doc, payload) = color_picker_fixture();
    let (view, mut context) = ProfileView {
        size: kurbo::Size::new(600.0, 400.0),
        scale: 2.0,
        root: None,
    }
    .prepare(&doc);
    let selection = Selection::from_payload(
        &crate::test_root(),
        &Sources {
            doc: &doc,
            libraries: &context.stack.libraries,
        },
        Vec::new(),
        payload,
    );
    move || {
        let annotations = Annotations::default();
        let mut frame = view.frame(&doc, &annotations);
        frame.selection = Some(&selection);
        context.frame(frame).0
    }
}

#[test]
#[ignore]
fn color_picker_profile_loop() {
    let mut frame = color_picker_frame();
    profile(
        "RGBA picker, 600x400 @2",
        |_| frame(),
        |bench| {
            assert!(
                bench.list.0.iter().any(|command| matches!(
                    command,
                    DrawCmd::Fill {
                        brush: Brush::Gradient(_),
                        ..
                    }
                )),
                "the open picker must paint its gradients"
            );
        },
    );
}

fn fidget_example() -> Document {
    fixture(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../examples/fidget.gid"
    )))
}

fn cam_profile_source() -> String {
    match std::env::var_os("CAM_PROFILE_SOURCE") {
        Some(path) => std::fs::read_to_string(path).expect("read CAM profile fixture"),
        None => Example::Toolpaths.source().to_owned(),
    }
}

#[test]
#[ignore = "CAM source pane, excluding the 3D viewport"]
fn cam_source_profile_loop() {
    cam_source_profile(false);
}

#[test]
#[ignore = "CAM source scrolling, excluding the 3D viewport and GPU presentation"]
fn cam_source_scroll_profile_loop() {
    cam_source_profile(true);
}

fn cam_source_profile(scrolling: bool) {
    let editor = crate::test_editor(fixture(&cam_profile_source()));
    let doc = &editor.model.doc;
    let (view, mut context) = ProfileView {
        size: kurbo::Size::new(600.0, 900.0),
        scale: 2.0,
        root: None,
    }
    .prepare(doc);
    profile(
        if scrolling {
            "CAM source scrolling, 600x900 @2"
        } else {
            "CAM source, 600x900 @2"
        },
        |index| {
            let mut frame = view.frame(doc, &editor.model.workspace.document.annotations);
            if scrolling {
                // Sweep a fixed 1200-point span and back, with a fixed viewport.
                let step = index % 240;
                let y = if step <= 120 { step } else { 240 - step };
                frame.origin.y -= y as f64 * 10.0 * view.scale;
            }
            context.frame(frame).0
        },
        |bench| assert!(!bench.list.0.is_empty()),
    );
}

#[test]
#[ignore = "Full native CAM frame while scrolling source; records GPU commands without presenting"]
fn cam_editor_scroll_profile_loop() {
    let mut runner = crate::EditorRunner::new(crate::test_editor(fixture(&cam_profile_source())));
    let size = kurbo::Size::new(1500.0, 900.0);
    let scale = 2.0;
    let mut samples = Vec::new();
    for index in 0..iterations() + 5 {
        let start = Instant::now();
        let step = index % 240;
        let y = if step <= 120 { step } else { 240 - step };
        runner.editor.model.workspace.document.scroll.y = y as f64 * 10.0;
        runner.refresh_frame(scale, size);
        let mut output = DrawList::default();
        puri::frame::render(runner.prepare_paint(scale, size).renders, &mut output);
        std::hint::black_box(&output);
        drop(output);
        runner.frame_presented();
        let elapsed = start.elapsed();
        if index == 0 {
            eprintln!("CAM full editor, 1500x900 @2, first frame {elapsed:.2?}");
        }
        if index >= 5 {
            samples.push(elapsed);
        }
    }
    distribution("CAM full editor source scrolling", samples.iter().copied());
    crate::perf::record("", samples);
}

fn orbit(path: &[Step], frame: usize) -> Annotations {
    // The camera belongs to the viewport's computed result, not its declaration.
    let path: Vec<_> = path
        .iter()
        .cloned()
        .chain([Step::Key(
            crate::libraries::presentation::vocabulary::RESULT,
        )])
        .collect();
    camera_at(&path, frame)
}

fn camera_at(path: &[Step], frame: usize) -> Annotations {
    use crate::libraries::{f32, fidget::vocabulary as f};
    let mut annotations = Annotations::default();
    annotations.set_field(
        path,
        f::CAMERA,
        Some(Value::record([
            (f::YAW, f32::value(30.0 + (frame % 180) as f32 * 2.0)),
            (f::PITCH, f32::value(60.0)),
            (f::ZOOM, f32::value(1.0)),
        ])),
    );
    annotations
}

fn fidget_orbit_profile(name: &str, source: &str) {
    // This canary records deferred mesh draws; GPU composition is measured by
    // shell_tests::compositor::mesh::cam_mesh_roundtrip_profile. CPU
    // validation is untimed.
    let doc = fixture(source);
    let path = first_viewport(&doc);
    let view = ProfileView {
        size: kurbo::Size::new(400.0, 600.0),
        scale: 2.0,
        root: Some(path.clone()),
    };
    let camera_path = Rc::new(std::cell::RefCell::new(None));
    let observed = camera_path.clone();
    let mut context = BenchContext::new();
    context.stack.projection = Projection::new([
        crate::display::partial(move |input| {
            use crate::libraries::{fidget::vocabulary as f, toolpath::vocabulary as t};
            let fields = input.value?.as_record()?;
            if observed.borrow().is_none()
                && [f::PREVIEW_3D, f::PREVIEW_MESH, t::PREVIEW_MESH]
                    .iter()
                    .any(|key| fields.contains_key(key))
            {
                let Hovered::Tree(crate::hover::Hover::Value(path)) = input.targets.current().hover
                else {
                    panic!("preview must have an occurrence");
                };
                *observed.borrow_mut() = Some(path);
            }
            None
        }),
        context.stack.projection.partial().clone(),
    ]);
    let (view, mut context) = view.prepare_with(&doc, context);
    let first_image = std::cell::RefCell::new(None::<ImageData>);
    let changed = std::cell::Cell::new(false);
    profile(
        &format!(
            "{name} orbit frame construction, 400x600 @2 viewport including controls; mesh rasterization excluded"
        ),
        |index| {
            let annotations = camera_path
                .borrow()
                .as_ref()
                .map(|path| camera_at(path, index))
                .unwrap_or_default();
            context.frame(view.frame(&doc, &annotations)).0
        },
        |bench| {
            let image = bench.image();
            assert_eq!(image.width, 800);
            assert!(image.height > 0 && image.height <= (view.size.height * view.scale) as u32);
            let mut first = first_image.borrow_mut();
            if let Some(first) = first.as_ref() {
                changed.set(changed.get() || first.data.as_ref() != image.data.as_ref());
            } else {
                *first = Some(image.clone());
            }
        },
    );
    assert!(camera_path.borrow().is_some());
    assert!(changed.get(), "orbit must change the rendered image");
}

#[test]
#[ignore]
fn fidget_orbit_profile_loop() {
    fidget_orbit_profile("spheres", crate::test_examples::SPHERES);
}

#[test]
#[ignore]
fn fidget_torus_profile_loop() {
    fidget_orbit_profile("torus", crate::test_examples::TORUS);
}

#[test]
#[ignore]
fn fidget_tanglecube_profile_loop() {
    fidget_orbit_profile("tanglecube", crate::test_examples::TANGLECUBE);
}

#[test]
#[ignore]
fn fidget_gyroid_profile_loop() {
    fidget_orbit_profile("gyroid", crate::test_examples::GYROID);
}

#[test]
#[ignore]
fn fidget_cube_profile_loop() {
    fidget_orbit_profile("cube", crate::test_examples::CUBE);
}

#[test]
#[ignore]
fn fidget_toolpaths_profile_loop() {
    use crate::libraries::toolpath::vocabulary::{PREVIEW_MESH, PREVIEW_REFINED};
    fidget_orbit_profile(
        "toolpaths",
        &cam_profile_source().replace(&PREVIEW_REFINED.to_string(), &PREVIEW_MESH.to_string()),
    );
}

#[test]
#[ignore = "CAM declaration and control pipeline, excluding geometry and rasterization"]
fn cam_controls_profile_loop() {
    let doc = fixture(&cam_profile_source());
    let view = ProfileView {
        size: kurbo::Size::new(400.0, 600.0),
        scale: 2.0,
        root: Some(first_viewport(&doc)),
    };
    // Keep viewport, memo, controls, and view-call evaluation intact. Only the
    // final 3D projection is replaced, avoiding the headless CPU raster fallback.
    let previews = Rc::new(std::cell::Cell::new(0));
    let rendered = previews.clone();
    let mut context = BenchContext::new();
    context.stack.projection = Projection::new([
        crate::display::runtime_partial(move |input| {
            input
                .value?
                .field(crate::libraries::toolpath::vocabulary::PREVIEW_REFINED)?;
            rendered.set(rendered.get() + 1);
            Some(crate::display::dim("preview omitted"))
        }),
        context.stack.projection.partial().clone(),
    ]);
    let (view, mut context) = view.prepare_with(&doc, context);
    profile(
        "CAM viewport/control pipeline, 400x600 @2, no 3D work",
        |_| context.frame(view.frame(&doc, &Annotations::default())).0,
        |bench| assert!(!bench.list.0.is_empty()),
    );
    assert_eq!(previews.get(), iterations() + 5);
}

#[test]
fn profile_viewport_uses_size_scale_and_camera_state() {
    let mut doc = fidget_example();
    let path = first_viewport(&doc);
    let cell = gid::new_cell_id();
    let declaration = crate::spine::get(doc.root.as_ref().unwrap(), &path)
        .unwrap()
        .clone();
    doc.cells.set_value(cell, declaration);
    doc.root = crate::spine::set(doc.root.as_ref(), &path, cell.into());
    let view = ProfileView {
        size: kurbo::Size::new(24.0, 32.0),
        scale: 2.0,
        root: Some(path.clone()),
    };
    let (view, mut context) = view.prepare(&doc);
    let resolved = view.root.as_deref().unwrap();
    assert_eq!(
        resolved.last(),
        Some(&Step::Follow(gid::Resolution::Document))
    );
    let (first, extent) = context.frame(view.frame(&doc, &orbit(resolved, 0)));
    let (second, _) = context.frame(view.frame(&doc, &orbit(resolved, 40)));
    let first = first.image();
    let second = second.image();
    assert_eq!((first.width, first.height), (48, 64));
    assert_eq!(extent.width, 48.0);
    assert_eq!(extent.ascent + extent.descent, 64.0);
    assert_ne!(
        first.data.as_ref(),
        second.data.as_ref(),
        "orbit must reach the renderer"
    );
}
