//! The frame harness tests share, beside the editor frames of
//! `frame::test_support`: a picker's offers, projected selections, line
//! writes, and a bench that times a frame's phases.

use super::*;
use crate::annotations::Annotations;
use crate::display::widget::navigation::Landmark;
use crate::driver::Driver;
use crate::frame::test_support::{editing_frame, settle};
use crate::libraries::text;
use crate::library::Libraries;
use gid::Document;
use kurbo::{Point, Rect, Stroke};
use measured::Extent;
use peniko::{Brush, ImageData};
use puri::draw::{DrawCmd, DrawList, GlyphRun, Shape};
use puri::hover::Claim;
use ui_events::keyboard::NamedKey;
use uig::Placement;

/// The toolkit's frame output and projection input over the editor.
pub(crate) type HoverOutput<C> = crate::display::widget::frame::HoverOutput<C, Hovered>;
pub(crate) type ProjectionInput<'a, V = Value> =
    crate::display::host::ProjectionInput<'a, crate::Editor, V>;

/// What `run` makes of a widget context at the root of the view `cx`,
/// drawing `value` when given.
pub(crate) fn at_root<W: 'static, H: 'static, R>(
    cx: &Cx<'_>,
    text: &mut TextCtx<'_>,
    project: &dyn crate::display::widget::project::Project<W, H>,
    value: Option<&grap::RuntimeValue>,
    run: impl FnOnce(&mut crate::display::widget::Context<'_, '_, W, H>) -> R,
) -> R {
    at_path(cx, text, project, &[], value, run)
}

/// [`at_root`] at `path`, where a computed one, `[Step::Computed]`, is
/// read-only.
pub(crate) fn at_path<W: 'static, H: 'static, R>(
    cx: &Cx<'_>,
    text: &mut TextCtx<'_>,
    project: &dyn crate::display::widget::project::Project<W, H>,
    path: &[crate::occurrence::Step],
    value: Option<&grap::RuntimeValue>,
    run: impl FnOnce(&mut crate::display::widget::Context<'_, '_, W, H>) -> R,
) -> R {
    run(&mut crate::display::widget::Context {
        text,
        project,
        toolkit: &Occurrence {
            cx,
            path,
            ancestors: &super::Ancestry::default(),
        },
        value,
    })
}

/// The entries the picker open at `selection` offers in `world`, once
/// `world` selects it again from its payload.
pub(crate) fn offered(
    world: &mut crate::Editor,
    selection: &Selection,
) -> Vec<crate::display::widget::offers::Entry<crate::Editor>> {
    world.model.selection = Some(Selection::from_payload(
        selection.root(),
        &world.sources(),
        selection.path().to_vec(),
        selection.payload(),
    ));
    editing_frame(world)
        .completion
        .expect("the selected pending emits its offers")
        .entries
        .to_vec()
}

/// An editor of `doc` reading through `libraries`.
fn editor_over(doc: &Document, libraries: &Libraries) -> crate::Editor {
    let mut world = crate::test_editor(doc.clone());
    world.stack.libraries = libraries.clone();
    world
}

/// Select what the current projection draws at `path`, without an editing
/// interaction.
pub(crate) fn make_projected_selection(
    doc: &Document,
    libraries: &Libraries,
    path: Path,
) -> Selection {
    let mut driver = Driver::new(editor_over(doc, libraries), Driver::VIEWPORT);
    driver.select(&path);
    driver
        .runner
        .editor
        .model
        .selection
        .take()
        .expect("a selection")
}

/// What selecting the line drawn at `path` and pressing End selects: its
/// editor, the caret at the end.
pub(crate) fn make_projected_editing_selection(
    doc: &Document,
    libraries: &Libraries,
    path: Path,
) -> Selection {
    let mut driver = Driver::new(editor_over(doc, libraries), Driver::VIEWPORT);
    driver.select(&path);
    driver.press(NamedKey::End);
    driver
        .runner
        .editor
        .model
        .selection
        .take()
        .expect("a selection")
}

/// Write `selected`'s line through as text; whether that opened an undo
/// step.
pub(crate) fn write_text(
    doc: &mut Rc<Document>,
    libraries: &Libraries,
    selected: &mut Selection,
) -> bool {
    crate::editing::commit_line(
        doc,
        libraries,
        selected,
        &(Rc::new(text::edit) as crate::display::LineUpdate),
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
    pub(crate) handler:
        puri::handler::Handler<crate::Editor, crate::frame::DispatchContext<crate::Editor>>,
    pub(crate) list: DrawList,
    pub(crate) landmarks: Vec<Landmark<crate::Editor>>,
    /// What the probe answered for the pass's pointer input.
    pub(crate) hit: Option<Claim<Hovered>>,
    pub(crate) times: FrameTimes,
    pub(crate) frame_elapsed: std::time::Duration,
}

impl puri::draw::CanvasSink for Bench {
    fn draw_mesh(&mut self, scene: puri::mesh::Scene, transform: Affine) {
        self.list.draw_mesh(scene, transform);
    }
    fn draw_image(&mut self, image: ImageData, transform: Affine) {
        self.list.draw_image(image, transform);
    }

    fn fill_shape(&mut self, shape: Shape, brush: Brush, transform: Affine) {
        self.list.fill(shape, brush, transform);
    }
    fn stroke_shape(&mut self, shape: Shape, style: Stroke, brush: Brush, transform: Affine) {
        self.list.stroke(shape, style, brush, transform);
    }
    fn draw_glyphs(&mut self, run: GlyphRun) {
        self.list.draw_glyphs(run);
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

pub(crate) struct BenchContext {
    pub(crate) computations: crate::computations::Computations,
    pub(crate) stack: crate::stack::Stack,
    pub(crate) styles: crate::display::widget::style::Styles,
    pub(crate) fonts: crate::display::test_support::Fonts,
}

/// What a bench frame draws and where: `root` within `doc`, `width` wide at
/// `origin`, clipped to `viewport` when given.
#[derive(Clone, Copy)]
pub(crate) struct BenchFrame<'a> {
    pub(crate) doc: &'a Document,
    pub(crate) selection: Option<&'a Selection>,
    pub(crate) annotations: Option<&'a Annotations>,
    pub(crate) width: f64,
    pub(crate) origin: Point,
    pub(crate) pointer: Option<Point>,
    pub(crate) viewport: Option<Rect>,
    pub(crate) root: &'a [Step],
}

impl<'a> BenchFrame<'a> {
    /// `doc`'s root in a window `width` wide, 24 points in from its edges.
    pub(crate) fn of(doc: &'a Document, width: f64) -> Self {
        Self {
            doc,
            selection: None,
            annotations: None,
            width: width - 48.0,
            origin: Point::new(24.0, 24.0),
            pointer: None,
            viewport: None,
            root: &[],
        }
    }
}

impl BenchContext {
    pub(crate) fn new() -> Self {
        Self {
            computations: crate::computations::Computations::default(),
            stack: crate::stack::load(),
            styles: crate::display::widget::style::editor(
                crate::display::widget::style::Theme::Light.palette(),
                1.0,
            ),
            fonts: crate::display::test_support::Fonts::new(),
        }
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
        } = self;
        computations.begin(Rc::new(doc.clone()), stack.libraries.clone());
        let unannotated = Annotations::default();
        let annotations = annotations.unwrap_or(&unannotated);
        let sources = Sources {
            doc,
            libraries: &stack.libraries,
        };
        let mut tcx = fonts.text(styles.scale);

        // Test-only phase timings; normal frames contain no timers.
        let start = std::time::Instant::now();
        let root_path = root;
        let root = crate::occurrence::read(&sources, root_path);
        #[cfg(feature = "layout-profile")]
        let profile = crate::display::profile::enter(crate::display::profile::Kind::Projection);
        let graph = prepare_project(
            ProjectDescription {
                focused: true,
                computations,
                view: &crate::test_root(),
                completions: Some(&stack.completions),
                sources,
                root,
                root_path,
                selection,
                source_selection: selection,
                annotations,
                names: true,
                styles,
                width,
                projection: &stack.projection,
            },
            &mut tcx,
        );
        #[cfg(feature = "layout-profile")]
        drop(profile);
        let prepare = start.elapsed();
        let phase = std::time::Instant::now();
        #[cfg(feature = "layout-profile")]
        let profile = crate::display::profile::enter(crate::display::profile::Kind::Choices);
        let node = resolve_choices(graph, width);
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
        let mut settled = settle(placed, &sources);
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

impl Bench {
    /// The one image the frame draws, rasterizing a deferred mesh.
    pub(crate) fn image(&self) -> ImageData {
        let images: Vec<_> = self
            .list
            .0
            .iter()
            .filter_map(|command| match command {
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
}

/// `doc` placed in a window `width` wide, `selection` selected.
pub(crate) fn place(doc: &Document, selection: Option<&Selection>, width: f64) -> (Bench, Extent) {
    place_with(BenchFrame {
        selection,
        ..BenchFrame::of(doc, width)
    })
}

/// What `frame` draws, on a bench of its own.
pub(crate) fn place_with(frame: BenchFrame<'_>) -> (Bench, Extent) {
    BenchContext::new().frame(frame)
}
