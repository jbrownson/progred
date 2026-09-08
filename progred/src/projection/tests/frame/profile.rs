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
        context.styles = crate::styles::editor(self.scale);
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

fn orbit(path: &[Step], frame: usize) -> Annotations {
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

fn image(bench: &Bench) -> &ImageData {
    let images: Vec<_> = bench
        .list
        .0
        .iter()
        .filter_map(|cmd| match cmd {
            DrawCmd::Image { image, .. } => Some(image),
            _ => None,
        })
        .collect();
    assert_eq!(
        images.len(),
        1,
        "the preview must render, not fall back to source text"
    );
    images[0]
}

fn fidget_orbit_profile(example: Example) {
    let doc = fixture(example.source());
    let path = first_viewport(&doc);
    let view = ProfileView {
        size: kurbo::Size::new(400.0, 600.0),
        scale: 2.0,
        root: Some(path.clone()),
    };
    let (view, mut context) = view.prepare(&doc);
    profile(
        &format!("{example:?} orbit, 400x600 @2 (800x1200 raster), automatic backend"),
        |index| {
            context
                .frame(view.frame(&doc, &orbit(view.root.as_deref().unwrap(), index)))
                .0
        },
        |bench| {
            let image = image(bench);
            assert_eq!((image.width, image.height), (800, 1200));
        },
    );
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
fn complex_fidget_examples_render_visible_surfaces() {
    let side = 64;
    for example in [
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
