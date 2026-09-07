//! Real projection and dispatch fixtures, without a window.

use super::*;
use peniko::ImageData;
use puri::draw::{DrawCmd, DrawList, GlyphRun, Shape};
use puri::hover::Claim;

type World = ();

struct Bench {
    list: DrawList,
    descends: Vec<Descend<World>>,
    /// What the probe answered for the pass's pointer input.
    hit: Option<Claim<Hovered>>,
    project_elapsed: std::time::Duration,
    frame_elapsed: std::time::Duration,
}

/// Probe with the pointer, then render and unpack the placed frame.
fn settle(placed: Placed<World, Bench>, pointer: Option<Point>) -> Bench {
    settle_with_sources(placed, pointer, None)
}

fn settle_with_sources(
    placed: Placed<World, Bench>,
    pointer: Option<Point>,
    sources: Option<&Sources>,
) -> Bench {
    let hit = pointer.and_then(|point| placed.probe(point, None, crate::frame::HOVER_REACH));
    let hovered = match &hit {
        Some(Claim::Direct(hover)) => Some(hover.clone()),
        _ => None,
    };
    let hovered_secondary = sources.and_then(|sources| match &hovered {
        Some(Hovered::Tree(hover)) => hover_secondary(sources, placed.completion.as_ref(), hover),
        _ => None,
    });
    let Placed {
        descends, renders, ..
    } = placed;
    let mut bench = Bench {
        list: DrawList::new(),
        descends,
        hit,
        project_elapsed: std::time::Duration::ZERO,
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

impl Canvas for Bench {
    fn image(&mut self, image: ImageData, transform: Affine) {
        self.list.image(image, transform);
    }

    fn fill(&mut self, shape: impl Into<Shape>, brush: impl Into<Brush>, transform: Affine) {
        self.list.fill(shape, brush, transform);
    }
    fn stroke(
        &mut self,
        shape: impl Into<Shape>,
        style: Stroke,
        brush: impl Into<Brush>,
        transform: Affine,
    ) {
        self.list.stroke(shape, style, brush, transform);
    }
    fn glyph_run(&mut self, run: GlyphRun) {
        self.list.glyph_run(run);
    }
    fn clip(
        &mut self,
        shape: impl Into<Shape>,
        transform: Affine,
        content: impl FnOnce(&mut Self),
    ) {
        let _ = (shape.into(), transform);
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
            bench.frame_elapsed, bench.project_elapsed,
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
        let hooks = Hooks::<World> {
            completions: Some(stack.completions.clone()),
            select: Rc::new(|_, _| {}),
            select_payload: Rc::new(|_, _, _| {}),
            edit_line: Rc::new(|_, _, _| None),
            toggle: Rc::new(|_, _| {}),
            update_state: Rc::new(|_, _, _| false),
            edit: Rc::new(|_| None),
            pick: Rc::new(|_, _| false),
            insert: Rc::new(|_, _| {}),
            delete: Rc::new(|_, _| false),
            apply: Rc::new(|_, _, _, _| false),
            point: Rc::new(|_, _, _, _, _| false),
            state_drag: Rc::new(|_, _, _, _, _| {}),
            scrub: Rc::new(|_, _, _, _, _| false),
            select_source: Rc::new(|_, _, _| {}),
            commit_value: Rc::new(|_, _, _| {}),
            commit_label: Rc::new(|_, _, _, _| {}),
            set_completion_view: Rc::new(|_, _, _, _| {}),
        };
        // Timed as the frame perf canary: projection is reported
        // separately, while the total also includes placement, hover,
        // and render-continuation settlement. Fallback-heavy narrow
        // widths are where accidental exponentials have surfaced twice.
        // Numbers only, no assert (user call).
        let start = std::time::Instant::now();
        let root_path = root;
        let root = sources.resolve_path(root_path);
        let node = project::<World, Bench>(
            ProjectDescription {
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
            hooks,
        );
        let project_elapsed = start.elapsed();
        let extent = node.extent;
        let rect = node.extent.rect_at(origin);
        let placed = measured::place(
            node,
            match viewport {
                Some(clip_rect) => Placement::new(rect, clip_rect),
                None => Placement::root(rect),
            },
        );
        let mut settled = settle_with_sources(placed, pointer, Some(&sources));
        settled.project_elapsed = project_elapsed;
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
mod drawing;
mod fidget_cube;
mod fidget_source;
mod interaction;
mod iop_tree_native;
mod layout;
mod profile;
mod svg;
