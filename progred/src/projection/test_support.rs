//! The frame harness tests share: editors framed headlessly, projected
//! selections, line writes, and a bench that times a frame's phases.

use super::*;
use crate::annotations::Annotations;
use crate::display::test_support::{Landmarks, ResolveForDispatch};
use crate::hover::hover_secondary;
use crate::libraries::text;
use crate::library::Libraries;
use crate::test_support::{arrow, make_selection};
use gid::Document;
use kurbo::{Rect, Stroke};
use measured::Extent;
use peniko::{Brush, ImageData};
use puri::draw::{DrawList, GlyphRun, Shape};
use puri::geometry::Placement;
use puri::hover::Claim;
use ui_events::keyboard::NamedKey;

type World = crate::Editor;

pub(crate) type EditingWorld = crate::Editor;

pub(crate) fn editing_world(doc: &Document, libraries: &Libraries) -> EditingWorld {
    let mut world = crate::test_editor(doc.clone());
    world.stack.libraries = libraries.clone();
    world
}

pub(crate) fn editing_frame(
    world: &mut EditingWorld,
    raw: bool,
) -> crate::placed::HoverOutput<EditingWorld> {
    editing_frame_with_projection(world, raw, None)
}

pub(crate) fn editing_frame_with_projection(
    world: &mut EditingWorld,
    raw: bool,
    projection: Option<&Projection<EditingWorld>>,
) -> crate::placed::HoverOutput<EditingWorld> {
    editing_frame_at(world, raw, projection, None)
}

pub(crate) fn editing_frame_at(
    world: &mut EditingWorld,
    raw: bool,
    projection: Option<&Projection<EditingWorld>>,
    pointer: Option<Point>,
) -> crate::placed::HoverOutput<EditingWorld> {
    editing_frame_with_annotations(world, raw, projection, pointer, &Annotations::default())
}

pub(crate) fn editing_frame_with_annotations(
    world: &mut EditingWorld,
    raw: bool,
    projection: Option<&Projection<EditingWorld>>,
    pointer: Option<Point>,
    annotations: &Annotations,
) -> crate::placed::HoverOutput<EditingWorld> {
    let stack = &world.stack;
    let styles = crate::styles::editor(world.palette, 1.0);
    let mut tcx = TextCtx {
        fonts: &mut world.font_cx,
        layouts: &mut world.layout_cx,
        scale: 1.0,
        cache: &mut world.text_cache,
    };
    let measured = project(
        ProjectDescription {
            focused: world.focused,
            computations: None,
            view: &crate::test_root(),
            completions: Some(&stack.completions),
            sources: Sources {
                doc: &world.model.doc,
                libraries: &world.stack.libraries,
            },
            root: world.model.doc.root.as_ref(),
            root_path: &[],
            selection: world.model.selection.as_ref(),
            source_selection: world.model.selection.as_ref(),
            annotations,
            raw,
            styles: &styles,
            width: 500.0,

            projection: (!raw).then_some(projection.unwrap_or(&stack.projection)),
        },
        &mut tcx,
    );
    let height = measured.extent.height().max(1.0);
    crate::display::widget::frame::place(
        measured,
        // A short document still has a viewport below it for completion popups.
        Placement::new(
            Rect::new(0.0, 0.0, 500.0, height),
            Rect::new(0.0, 0.0, 500.0, height.max(500.0)),
        ),
        &placed::HoverInput {
            command_modifier: world.command_modifier,
            pointer,
            ..Default::default()
        },
    )
}

/// Select through the current projection, without an editing interaction.
pub(crate) fn make_projected_selection(
    doc: &Document,
    libraries: &Libraries,
    path: Path,
) -> Selection {
    let mut world = editing_world(doc, libraries);
    let placed = editing_frame(&mut world, false);
    if let Some(target) = placed.descends.landmark(&path) {
        (target.select)(&mut world, None);
    }
    world
        .model
        .selection
        .unwrap_or_else(|| make_selection(path))
}

pub(crate) fn make_projected_editing_selection(
    doc: &Document,
    libraries: &Libraries,
    path: Path,
) -> Selection {
    let mut world = editing_world(doc, libraries);
    world.model.selection = Some(make_projected_selection(doc, libraries, path));
    editing_frame(&mut world, false)
        .resolve_for_dispatch()
        .dispatch_key(&mut world, &arrow(NamedKey::End));
    world.model.selection.unwrap()
}

pub(crate) fn write_text(
    doc: &mut Rc<Document>,
    libraries: &Libraries,
    selected: &mut Selection,
) -> bool {
    write_with(doc, libraries, selected, text::edit)
}

pub(crate) fn write_with(
    doc: &mut Rc<Document>,
    libraries: &Libraries,
    selected: &mut Selection,
    update: impl Fn(&str, Option<&Value>) -> Option<Value> + 'static,
) -> bool {
    line_control::commit(
        doc,
        libraries,
        selected,
        &crate::libraries::line_edit::native(update),
    )
}

#[derive(Clone, Copy, Default)]
pub(crate) struct FrameTimes {
    pub(crate) prepare: std::time::Duration,
    pub(crate) choices: std::time::Duration,
    pub(crate) placement: std::time::Duration,
    pub(crate) hover: std::time::Duration,
    pub(crate) paint: std::time::Duration,
}

pub(crate) struct Bench {
    pub(crate) handler: puri::handler::Handler<World, crate::placed::DispatchContext<World>>,
    pub(crate) list: DrawList,
    pub(crate) descends: Vec<Descend<World>>,
    /// What the probe answered for the pass's pointer input.
    pub(crate) hit: Option<Claim<Hovered>>,
    pub(crate) times: FrameTimes,
    pub(crate) frame_elapsed: std::time::Duration,
}

/// Paint and unpack the output of the completed hover pass.
pub(crate) fn settle(mut placed: crate::placed::HoverOutput<World>) -> Bench {
    let binding = std::time::Instant::now();
    #[cfg(feature = "layout-profile")]
    let profile = crate::display::profile::enter(crate::display::profile::Kind::Hover);
    let hit = placed.claim.take();
    let hit = hit.map(|(_, claim)| claim);
    let hovered = match &hit {
        Some(Claim::Direct(hover)) => Some(hover.clone()),
        _ => None,
    };
    let landmark = match &hovered {
        Some(Hovered::Tree(Hover::Value(path))) => {
            placed.descends.iter().find(|descend| descend.path == *path)
        }
        _ => None,
    };
    let hovered_secondary = match &hovered {
        Some(Hovered::Tree(hover)) => hover_secondary(landmark, placed.completion.as_ref(), hover),
        _ => None,
    };
    let selected_secondary =
        crate::display::widget::navigation::selected_secondary(&placed.descends);
    let frame = placed.bind(crate::placed::Attribution {
        hovered,
        hovered_secondary,
        selected_secondary,
        hovered_trace: None,
    });
    let hover = binding.elapsed();
    #[cfg(feature = "layout-profile")]
    drop(profile);
    let mut bench = Bench {
        handler: frame.handler.unwrap_or_default(),
        list: DrawList::new(),
        descends: frame.descends,
        hit,
        times: FrameTimes {
            hover,
            ..Default::default()
        },
        frame_elapsed: std::time::Duration::ZERO,
    };
    puri::frame::render(frame.renders, &mut bench);
    bench
}

impl puri::draw::CanvasSink for Bench {
    fn draw_mesh(&mut self, scene: puri::mesh::Scene, transform: Affine) {
        self.list.mesh(scene, transform);
    }
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

pub(crate) fn place_with_pointer(
    doc: &Document,
    selection: Option<&Selection>,
    width: f64,
    pointer: Option<Point>,
) -> (Bench, Extent) {
    place_with_inputs(doc, selection, width, pointer, None)
}

pub(crate) fn place_with_inputs(
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

pub(crate) fn place_with_annotations(
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

pub(crate) struct BenchContext {
    pub(crate) computations: crate::computations::Computations,
    pub(crate) stack: crate::stack::Stack<World>,
    pub(crate) styles: crate::styles::Styles,
    pub(crate) fonts: parley::FontContext,
    pub(crate) layouts: parley::LayoutContext<Brush>,
    pub(crate) cache: puri::text::TextCache,
}

pub(crate) struct BenchFrame<'a> {
    pub(crate) doc: &'a Document,
    pub(crate) selection: Option<&'a Selection>,
    pub(crate) annotations: &'a Annotations,
    pub(crate) width: f64,
    pub(crate) origin: Point,
    pub(crate) pointer: Option<Point>,
    pub(crate) viewport: Option<Rect>,
    pub(crate) root: &'a [Step],
}

impl BenchContext {
    pub(crate) fn new() -> Self {
        Self {
            computations: crate::computations::Computations::default(),
            stack: crate::stack::load(),
            styles: crate::styles::editor(crate::styles::Theme::Light.palette(), 1.0),
            fonts: crate::fonts::bundled_font_context(),
            layouts: parley::LayoutContext::new(),
            cache: puri::text::TextCache::default(),
        }
    }

    pub(crate) fn place(
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

    pub(crate) fn frame(&mut self, input: BenchFrame<'_>) -> (Bench, Extent) {
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
            computations,
            stack,
            styles,
            fonts,
            layouts,
            cache,
        } = self;
        computations.begin(Rc::new(doc.clone()), stack.libraries.clone());
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
                focused: true,
                computations: Some(computations),
                view: &crate::test_root(),
                completions: Some(&stack.completions),
                sources,
                root,
                root_path,
                selection,
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
        let placed = crate::display::widget::frame::place(
            node,
            match viewport {
                Some(clip_rect) => Placement::new(rect, clip_rect),
                None => Placement::root(rect),
            },
            &crate::display::widget::HoverInput {
                pointer,
                reach_px: crate::frame::HOVER_REACH_POINTS * styles.scale,
                ..Default::default()
            },
        );
        #[cfg(feature = "layout-profile")]
        drop(profile);
        let placement = phase.elapsed();
        let phase = std::time::Instant::now();
        #[cfg(feature = "layout-profile")]
        let profile = crate::display::profile::enter(crate::display::profile::Kind::Paint);
        let mut settled = settle(placed);
        #[cfg(feature = "layout-profile")]
        drop(profile);
        settled.times = FrameTimes {
            prepare,
            choices,
            placement,
            hover: settled.times.hover,
            paint: phase.elapsed().saturating_sub(settled.times.hover),
        };
        settled.frame_elapsed = start.elapsed();
        (settled, extent)
    }
}

pub(crate) fn place(doc: &Document, selection: Option<&Selection>, width: f64) -> (Bench, Extent) {
    place_with_pointer(doc, selection, width, None)
}
