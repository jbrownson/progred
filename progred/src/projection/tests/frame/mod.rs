//! Real projection and dispatch fixtures, without a window.

use super::*;
use peniko::ImageData;
use puri::draw::{DrawCmd, DrawList, GlyphRun, Shape};
use puri::hover::Claim;

type World = crate::Editor;

#[test]
fn native_decorators_preserve_front_to_back_input_and_back_to_front_paint() {
    let log = Rc::new(std::cell::RefCell::new(vec![]));
    let contribution = |name: &'static str| {
        let log = log.clone();
        Box::new(
            move |output: &mut crate::display::widget::HoverContext<'_, crate::Editor, Hovered>,
                  _: Placement| {
                let during_paint = log.clone();
                output.render(move |_, _| during_paint.borrow_mut().push(name));
                output.handler().on_key(move |_, _| {
                    log.borrow_mut().push(name);
                    false
                });
            },
        ) as crate::display::widget::HoverCallback<crate::Editor, Hovered>
    };
    let extent = Extent {
        width: 40.0,
        ascent: 10.0,
        descent: 5.0,
    };
    let child = crate::display::widget::leaf(extent, contribution("child"));
    let before = contribution("before");
    let after = contribution("after");
    let decorated = crate::display::widget::after_hover(
        crate::display::widget::before_hover(child, move |placement, output| {
            before(output, placement)
        }),
        move |placement, output| after(output, placement),
    );
    assert_eq!(decorated.extent, extent);
    let output = measured::place_top_left(decorated, Point::ZERO).run(&Default::default());
    assert!(log.borrow().is_empty());
    assert!(!output.handler.as_ref().unwrap().dispatch_key(
        &mut crate::test_editor(Document {
            root: None,
            cells: Cells::new()
        }),
        &KeyboardEvent::default()
    ));
    assert_eq!(&*log.borrow(), &["after", "child", "before"]);
    log.borrow_mut().clear();
    settle(output);
    assert_eq!(&*log.borrow(), &["before", "child", "after"]);
}

#[derive(Clone, Copy, Default)]
struct FrameTimes {
    prepare: std::time::Duration,
    choices: std::time::Duration,
    placement: std::time::Duration,
    hover: std::time::Duration,
    paint: std::time::Duration,
}

struct Bench {
    list: DrawList,
    descends: Vec<Descend<World>>,
    /// What the probe answered for the pass's pointer input.
    hit: Option<Claim<Hovered>>,
    times: FrameTimes,
    frame_elapsed: std::time::Duration,
}

/// Paint and unpack the output of the completed hover pass.
fn settle(placed: crate::placed::Ready<World>) -> Bench {
    settle_with_sources(placed, None)
}

fn settle_with_sources(placed: crate::placed::Ready<World>, sources: Option<&Sources>) -> Bench {
    let hit = placed.claim;
    let hit = hit.map(|(_, claim)| claim);
    let hovered = match &hit {
        Some(Claim::Direct(hover)) => Some(hover.clone()),
        _ => None,
    };
    let hovered_secondary = sources.and_then(|sources| match &hovered {
        Some(Hovered::Tree(hover)) => hover_secondary(sources, placed.completion.as_ref(), hover),
        _ => None,
    });
    let crate::placed::Ready {
        descends, renders, ..
    } = placed;
    let mut bench = Bench {
        list: DrawList::new(),
        descends,
        hit,
        times: FrameTimes::default(),
        frame_elapsed: std::time::Duration::ZERO,
    };
    let ink = crate::placed::Ink {
        hovered: hovered.as_ref(),
        hovered_secondary: hovered_secondary.as_ref(),
        hovered_trace: None,
        debug_geometry: false,
    };
    for render in renders {
        render(&mut bench, ink);
    }
    bench
}

impl puri::draw::CanvasSink for Bench {
    fn draw_image(&mut self, image: ImageData, transform: Affine) {
        self.list.image(image, transform);
    }

    fn fill_shape(&mut self, shape: Shape, brush: Brush, transform: Affine) {
        self.list.fill(shape, brush, transform);
    }
    fn stroke_shape(&mut self, shape: Shape, style: Stroke, brush: Brush, transform: Affine) {
        self.list.stroke(shape, style, brush, transform);
    }
    fn draw_glyphs(&mut self, run: GlyphRun) {
        self.list.glyph_run(run);
    }
    fn with_clip(
        &mut self,
        shape: Shape,
        transform: Affine,
        content: Box<dyn FnOnce(&mut dyn puri::draw::CanvasSink) + '_>,
    ) {
        let _ = (shape, transform);
        content(self);
    }
}

fn place_with_pointer(
    doc: &Document,
    selection: Option<&Selection>,
    width: f64,
    pointer: Option<Point>,
) -> (Bench, Extent) {
    place_with_inputs(doc, selection, width, pointer, None)
}

fn place_with_inputs(
    doc: &Document,
    selection: Option<&Selection>,
    width: f64,
    pointer: Option<Point>,
    viewport: Option<Rect>,
) -> (Bench, Extent) {
    place_with_annotations(
        doc,
        selection,
        &Annotations::default(),
        width,
        pointer,
        viewport,
        None,
    )
}

fn place_with_annotations(
    doc: &Document,
    selection: Option<&Selection>,
    annotations: &Annotations,
    width: f64,
    pointer: Option<Point>,
    viewport: Option<Rect>,
    root: Option<&[Step]>,
) -> (Bench, Extent) {
    BenchContext::new().place(doc, selection, annotations, width, pointer, viewport, root)
}

struct BenchContext {
    stack: crate::stack::Stack<World>,
    styles: crate::styles::Styles,
    fonts: parley::FontContext,
    layouts: parley::LayoutContext<Brush>,
    cache: puri::text::TextCache,
}

struct BenchFrame<'a> {
    doc: &'a Document,
    selection: Option<&'a Selection>,
    annotations: &'a Annotations,
    width: f64,
    origin: Point,
    pointer: Option<Point>,
    viewport: Option<Rect>,
    root: &'a [Step],
}

impl BenchContext {
    fn new() -> Self {
        Self {
            stack: crate::stack::load(),
            styles: crate::styles::editor(1.0),
            fonts: parley::FontContext::new(),
            layouts: parley::LayoutContext::new(),
            cache: puri::text::TextCache::default(),
        }
    }

    fn place(
        &mut self,
        doc: &Document,
        selection: Option<&Selection>,
        annotations: &Annotations,
        width: f64,
        pointer: Option<Point>,
        viewport: Option<Rect>,
        root: Option<&[Step]>,
    ) -> (Bench, Extent) {
        let (bench, extent) = self.frame(BenchFrame {
            doc,
            selection,
            annotations,
            width: width - 48.0,
            origin: Point::new(24.0, 24.0),
            pointer,
            viewport,
            root: root.unwrap_or(&[]),
        });
        eprintln!(
            "frame at {width:.0}px: {:.1?} (project {:.1?})",
            bench.frame_elapsed,
            bench.times.prepare + bench.times.choices,
        );
        (bench, extent)
    }

    fn frame(&mut self, input: BenchFrame<'_>) -> (Bench, Extent) {
        let BenchFrame {
            doc,
            selection,
            annotations,
            width,
            origin,
            pointer,
            viewport,
            root,
        } = input;
        let Self {
            stack,
            styles,
            fonts,
            layouts,
            cache,
        } = self;
        let sources = Sources {
            doc,
            libraries: &stack.libraries,
        };
        let mut tcx = TextCtx {
            fonts,
            layouts,
            scale: styles.scale as f32,
            cache,
        };

        // Test-only phase timings; normal frames contain no timers.
        let start = std::time::Instant::now();
        let root_path = root;
        let root = sources.resolve_path(root_path);
        #[cfg(feature = "layout-profile")]
        let profile = crate::display::profile::enter(crate::display::profile::Kind::Projection);
        let graph = prepare_project(
            ProjectDescription {
                view: &crate::test_root(),
                completions: Some(&stack.completions),
                sources,
                root,
                root_path,
                selection,
                scrub_spelling: None,
                source_selection: selection,
                annotations,
                raw: false,
                styles,
                width,
                projection: Some(&stack.projection),
            },
            &mut tcx,
        );
        #[cfg(feature = "layout-profile")]
        drop(profile);
        let prepare = start.elapsed();
        let phase = std::time::Instant::now();
        #[cfg(feature = "layout-profile")]
        let profile = crate::display::profile::enter(crate::display::profile::Kind::Choices);
        let node = resolve_choices(
            graph,
            width,
            std::env::var_os("PROGRED_LAYOUT_TRACE").is_some(),
        );
        #[cfg(feature = "layout-profile")]
        drop(profile);
        let choices = phase.elapsed();
        let phase = std::time::Instant::now();
        let extent = node.extent;
        let rect = node.extent.rect_at(origin);
        #[cfg(feature = "layout-profile")]
        let profile = crate::display::profile::enter(crate::display::profile::Kind::Placement);
        let placed = measured::place(
            node,
            match viewport {
                Some(clip_rect) => Placement::new(rect, clip_rect),
                None => Placement::root(rect),
            },
        );
        #[cfg(feature = "layout-profile")]
        drop(profile);
        let placement = phase.elapsed();
        let phase = std::time::Instant::now();
        #[cfg(feature = "layout-profile")]
        let profile = crate::display::profile::enter(crate::display::profile::Kind::Hover);
        let placed = placed.run(&crate::display::widget::HoverInput {
            pointer,
            reach: crate::frame::HOVER_REACH,
            ..Default::default()
        });
        #[cfg(feature = "layout-profile")]
        drop(profile);
        let hover = phase.elapsed();
        let phase = std::time::Instant::now();
        #[cfg(feature = "layout-profile")]
        let profile = crate::display::profile::enter(crate::display::profile::Kind::Paint);
        let mut settled = settle_with_sources(placed, Some(&sources));
        #[cfg(feature = "layout-profile")]
        drop(profile);
        settled.times = FrameTimes {
            prepare,
            choices,
            placement,
            hover,
            paint: phase.elapsed(),
        };
        settled.frame_elapsed = start.elapsed();
        (settled, extent)
    }
}

fn place(doc: &Document, selection: Option<&Selection>, width: f64) -> (Bench, Extent) {
    place_with_pointer(doc, selection, width, None)
}

fn key(s: &str) -> Step {
    Step::Key(crate::test_values::label(s))
}

mod completion;
mod declarations;
mod drawing;
mod fidget_cube;
mod fidget_source;
mod interaction;
mod iop_tree_native;
mod layout;
mod profile;
mod svg;
