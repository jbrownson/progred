//! Opt-in, headless frame canaries. See docs/performance.md for scope and commands.

use super::*;
use crate::command::Example;
use std::time::{Duration, Instant};

#[cfg(feature = "layout-profile")]
mod forms;
mod layout_ffi;

struct ProfileView {
    /// Logical points; the projection and clip receive physical pixels.
    size: kurbo::Size,
    scale: f64,
    root: Option<gid::Path>,
}

impl ProfileView {
    fn prepare(mut self, doc: &Document) -> (Self, BenchContext) {
        let mut context = BenchContext::new();
        context.styles = crate::styles::editor(crate::styles::Theme::Light.palette(), self.scale);
        if let Some(path) = &self.root {
            let sources = Sources {
                doc,
                libraries: &context.stack.libraries,
            };
            self.root = Some(
                crate::projection::viewport::entry(sources, path)
                    .expect("profile target must be a viewport declaration")
                    .path,
            );
            context.stack.projection =
                crate::projection::viewport::projection(&context.stack.projection, self.size);
        }
        (self, context)
    }

    fn frame<'a>(&'a self, doc: &'a Document, annotations: &'a Annotations) -> BenchFrame<'a> {
        let size = self.size * self.scale;
        let margin = if self.root.is_some() {
            0.0
        } else {
            12.0 * self.scale
        };
        BenchFrame {
            doc,
            selection: None,
            annotations,
            width: size.width - 2.0 * margin,
            origin: Point::new(margin, margin),
            pointer: None,
            viewport: Some(size.to_rect()),
            root: self.root.as_deref().unwrap_or(&[]),
        }
    }
}

#[derive(Clone, Copy)]
struct Timing {
    total: Duration,
    phases: FrameTimes,
    disposal: Duration,
}

fn iterations() -> usize {
    match std::env::var("FRAME_PROFILE_ITERATIONS") {
        Ok(value) => value
            .parse::<usize>()
            .ok()
            .filter(|n| *n > 0)
            .expect("FRAME_PROFILE_ITERATIONS must be a positive integer"),
        Err(_) => 60,
    }
}

fn distribution(label: &str, samples: impl Iterator<Item = Duration>) {
    let mut samples: Vec<_> = samples.collect();
    samples.sort_unstable();
    let percentile = |percent: usize| samples[(samples.len() * percent).div_ceil(100) - 1];
    eprintln!(
        "  {label}: median {:.2?}, p95 {:.2?}, max {:.2?}",
        percentile(50),
        percentile(95),
        samples.last().unwrap(),
    );
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
    let editor = crate::test_editor(fixture(&cam_profile_source()));
    let doc = &editor.model.doc;
    let (view, mut context) = ProfileView {
        size: kurbo::Size::new(600.0, 900.0),
        scale: 2.0,
        root: None,
    }
    .prepare(doc);
    profile(
        "CAM source, 600x900 @2",
        |_| {
            context
                .frame(view.frame(doc, &editor.model.workspace.document.annotations))
                .0
        },
        |bench| assert!(!bench.list.0.is_empty()),
    );
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

fn image(bench: &Bench) -> ImageData {
    let images: Vec<_> = bench
        .list
        .0
        .iter()
        .filter_map(|cmd| match cmd {
            DrawCmd::Image { image, .. } => Some(image.clone()),
            DrawCmd::Mesh { scene, .. } => scene.rasterize(),
            _ => None,
        })
        .collect();
    assert_eq!(
        images.len(),
        1,
        "the preview must render, not fall back to source text"
    );
    images.into_iter().next().unwrap()
}

fn fidget_orbit_profile(example: Example) {
    // This canary records deferred mesh draws; GPU composition is measured by
    // compositor::mesh::cam_mesh_roundtrip_profile. CPU validation is untimed.
    let source = if example == Example::Toolpaths {
        use crate::libraries::toolpath::vocabulary::{PREVIEW_MESH, PREVIEW_REFINED};
        cam_profile_source().replace(
            &PREVIEW_REFINED.simple().to_string(),
            &PREVIEW_MESH.simple().to_string(),
        )
    } else {
        example.source().to_owned()
    };
    let doc = fixture(&source);
    let path = first_viewport(&doc);
    let view = ProfileView {
        size: kurbo::Size::new(400.0, 600.0),
        scale: 2.0,
        root: Some(path.clone()),
    };
    let (view, mut context) = view.prepare(&doc);
    let camera_path = Rc::new(std::cell::RefCell::new(None));
    let observed = camera_path.clone();
    context.stack.projection.partial = crate::display::compose_partials([
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
        context.stack.projection.partial.clone(),
    ]);
    let first_image = std::cell::RefCell::new(None::<ImageData>);
    let changed = std::cell::Cell::new(false);
    profile(
        &format!(
            "{example:?} orbit frame construction, 400x600 @2 viewport including controls; mesh rasterization excluded"
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
            let image = image(bench);
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
    fidget_orbit_profile(Example::Fidget);
}

#[test]
#[ignore]
fn fidget_torus_profile_loop() {
    fidget_orbit_profile(Example::Torus);
}

#[test]
#[ignore]
fn fidget_tanglecube_profile_loop() {
    fidget_orbit_profile(Example::Tanglecube);
}

#[test]
#[ignore]
fn fidget_gyroid_profile_loop() {
    fidget_orbit_profile(Example::Gyroid);
}

#[test]
#[ignore]
fn fidget_cube_profile_loop() {
    fidget_orbit_profile(Example::Cube);
}

#[test]
#[ignore]
fn fidget_toolpaths_profile_loop() {
    fidget_orbit_profile(Example::Toolpaths);
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
    let (view, mut context) = view.prepare(&doc);
    // Keep viewport, memo, controls, and view-call evaluation intact. Only the
    // final 3D projection is replaced, avoiding the headless CPU raster fallback.
    let previews = Rc::new(std::cell::Cell::new(0));
    let rendered = previews.clone();
    context.stack.projection.partial = crate::display::compose_partials([
        crate::display::runtime_partial(move |input| {
            input
                .value?
                .field(crate::libraries::toolpath::vocabulary::PREVIEW_REFINED)?;
            rendered.set(rendered.get() + 1);
            Some(crate::display::dim("preview omitted"))
        }),
        context.stack.projection.partial.clone(),
    ]);
    profile(
        "CAM viewport/control pipeline, 400x600 @2, no 3D work",
        |_| context.frame(view.frame(&doc, &Annotations::default())).0,
        |bench| assert!(!bench.list.0.is_empty()),
    );
    assert_eq!(previews.get(), iterations() + 5);
}

#[test]
fn complex_fidget_examples_render_visible_surfaces() {
    let side = 64;
    for example in [
        Example::Fidget,
        Example::Torus,
        Example::Tanglecube,
        Example::Gyroid,
        Example::Cube,
    ] {
        let doc = fixture(example.source());
        let view = ProfileView {
            size: kurbo::Size::new(side as f64, side as f64),
            scale: 1.0,
            root: Some(first_viewport(&doc)),
        };
        let (view, mut context) = view.prepare(&doc);
        let (bench, _) = context.frame(view.frame(&doc, &Annotations::default()));
        let image = image(&bench);
        assert_eq!((image.width, image.height), (side, side));
        let covered = image
            .data
            .as_ref()
            .chunks_exact(4)
            .filter(|pixel| pixel[3] > 0)
            .count();
        assert!(
            covered > side as usize && covered < (side * side / 2) as usize,
            "{example:?}: {covered} covered pixels"
        );
        let pixels = || image.data.as_ref().chunks_exact(4).filter(|p| p[3] > 0);
        if example == Example::Fidget {
            assert!(pixels().any(|p| p[2] > p[0]), "blue shell");
            assert!(pixels().any(|p| p[0] > p[2]), "gold sphere");
        } else if example == Example::Cube {
            assert!(pixels().all(|p| p[2] > p[0]), "cyan cube");
        }
    }
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
    let first = image(&first);
    let second = image(&second);
    assert_eq!((first.width, first.height), (48, 64));
    assert_eq!(extent.width, 48.0);
    assert_eq!(extent.ascent + extent.descent, 64.0);
    assert_ne!(
        first.data.as_ref(),
        second.data.as_ref(),
        "orbit must reach the renderer"
    );
}
