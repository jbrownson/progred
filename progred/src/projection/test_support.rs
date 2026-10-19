//! The frame harness tests share: editors framed headlessly, projected
//! selections, line writes, and a bench that times a frame's phases in a
//! window of a given size.

use super::*;
use crate::annotations::Annotations;
use crate::display::test_support::Landmarks;
use crate::display::widget::navigation::Landmark;
use crate::driver::Driver;
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

/// How a test frame shows an editor's document: as the editor's own view
/// would, unless a field says otherwise.
#[derive(Clone, Copy, Default)]
pub(crate) struct Framing<'a> {
    /// Raw's view rather than the libraries' projections.
    pub(crate) raw: bool,
    /// Drawn with this projection rather than the editor's.
    pub(crate) projection: Option<&'a Projection>,
    /// Where the pointer rests.
    pub(crate) pointer: Option<Point>,
    /// Selected this way rather than as the editor is.
    pub(crate) selection: Option<&'a Selection>,
}

/// `world`'s document view projected 500 points wide and placed, unbound.
pub(crate) fn editing_frame(world: &mut crate::Editor) -> HoverOutput<crate::Editor> {
    framed(world, Framing::default())
}

/// As [`editing_frame`], framed as `framing` says.
pub(crate) fn framed(
    world: &mut crate::Editor,
    framing: Framing<'_>,
) -> HoverOutput<crate::Editor> {
    let Framing {
        raw,
        projection,
        pointer,
        selection,
    } = framing;
    let selection = selection.or(world.model.selection.as_ref());
    let stack = &world.stack;
    let styles = crate::display::widget::style::editor(world.palette, 1.0);
    let raw_projection = Projection::default();
    world
        .computations
        .begin(world.model.doc.clone(), stack.libraries.clone());
    let mut tcx = TextCtx {
        fonts: &mut world.font_cx,
        layouts: &mut world.layout_cx,
        scale: 1.0,
        cache: &mut world.text_cache,
    };
    let view = &world.model.workspace.document;
    let measured = project(
        ProjectDescription {
            focused: world.focused,
            computations: &world.computations,
            view: &view.root,
            completions: Some(&stack.completions),
            sources: Sources {
                doc: &world.model.doc,
                libraries: &stack.libraries,
            },
            root: world.model.doc.root.as_ref(),
            root_path: &[],
            selection,
            source_selection: selection,
            annotations: &view.annotations,
            raw,
            styles: &styles,
            width: 500.0,
            projection: if raw {
                &raw_projection
            } else {
                projection.unwrap_or(&stack.projection)
            },
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
        &crate::display::widget::HoverInput {
            command_modifier: world.command_modifier,
            pointer,
            ..Default::default()
        },
    )
}

/// What `run` makes of a widget context at the root of the view `cx`,
/// drawing `value` when given.
pub(crate) fn at_root<W: 'static, H: 'static, R>(
    cx: &Cx<'_>,
    text: &mut TextCtx<'_>,
    project: &dyn crate::display::widget::project::Project<W, H>,
    value: Option<&grap::RuntimeValue>,
    run: impl FnOnce(&mut crate::display::widget::Context<'_, '_, W, H>) -> R,
) -> R {
    run(&mut crate::display::widget::Context {
        text,
        project,
        toolkit: &Occurrence { cx, path: &[] },
        value,
    })
}

/// The entries the picker open at `selection` offers in `world`.
pub(crate) fn offered(
    world: &mut crate::Editor,
    selection: &Selection,
) -> Vec<crate::display::widget::offers::Entry<crate::Editor>> {
    framed(
        world,
        Framing {
            selection: Some(selection),
            ..Framing::default()
        },
    )
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
    let mut world = editor_over(doc, libraries);
    let placed = editing_frame(&mut world);
    let target = placed
        .landmarks
        .landmark(&path)
        .unwrap_or_else(|| panic!("nothing drawn at {path:?}"));
    assert!((target.select)(&mut world, None), "{path:?} refused");
    world.model.selection.expect("a selection")
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

/// Bind `placed` as the frame does, attributing what the pointer rests on
/// through `sources`, then paint and unpack it.
pub(crate) fn settle(mut placed: HoverOutput<crate::Editor>, sources: &Sources<'_>) -> Bench {
    let binding = std::time::Instant::now();
    #[cfg(feature = "layout-profile")]
    let profile = crate::display::profile::enter(crate::display::profile::Kind::Hover);
    let claim = placed.claim.take();
    let hit = claim.as_ref().map(|(_, claim)| claim.clone());
    let (hovered, root) = crate::frame::hover_target(claim);
    let attribution = crate::frame::attribute(
        sources,
        &placed.landmarks,
        root.as_ref(),
        placed.completion.as_ref(),
        hovered,
        false,
    );
    let frame = placed.bind(attribution);
    let hover = binding.elapsed();
    #[cfg(feature = "layout-profile")]
    drop(profile);
    let mut bench = Bench {
        handler: frame.handler.unwrap_or_default(),
        list: DrawList::new(),
        landmarks: frame.landmarks,
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
        let root = sources.resolve_path(root_path);
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
                raw: false,
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

/// A window for a bench frame: its size, its zoom, and optionally the
/// viewport declaration it shows alone, as a pane does.
pub(crate) struct ProfileView {
    /// Logical points; the projection and clip receive physical pixels.
    pub(crate) size: kurbo::Size,
    pub(crate) scale: f64,
    pub(crate) root: Option<gid::Path>,
}

impl ProfileView {
    /// A bench for this view, with a pane's projection when it shows a
    /// viewport, and the viewport's path resolved through its cells.
    pub(crate) fn prepare(self, doc: &Document) -> (Self, BenchContext) {
        self.prepare_with(doc, BenchContext::new())
    }

    /// As [`Self::prepare`], on a bench whose projection the caller chose.
    pub(crate) fn prepare_with(
        mut self,
        doc: &Document,
        mut context: BenchContext,
    ) -> (Self, BenchContext) {
        context.styles = crate::display::widget::style::editor(
            crate::display::widget::style::Theme::Light.palette(),
            self.scale,
        );
        let view = crate::workspace::View {
            root: self.root.clone().map_or_else(
                crate::display::widget::view::Root::document,
                crate::display::widget::view::Root::pane,
            ),
            projection: crate::workspace::Projection::Standard,
            hidden: Vec::new(),
            annotations: Annotations::default(),
            scroll: kurbo::Vec2::ZERO,
        };
        let sources = Sources {
            doc,
            libraries: &context.stack.libraries,
        };
        let shown = crate::frame::view_content(
            &context.stack,
            sources,
            &view,
            self.size * self.scale,
            self.scale,
        );
        assert!(
            self.root.is_none() || shown.viewport,
            "profile target must be a viewport declaration"
        );
        let (root, projection) = (shown.root_path.into_owned(), shown.projection.into_owned());
        self.root = self.root.map(|_| root);
        context.stack.projection = projection;
        (self, context)
    }

    pub(crate) fn frame<'a>(
        &'a self,
        doc: &'a Document,
        annotations: &'a Annotations,
    ) -> BenchFrame<'a> {
        let size = self.size * self.scale;
        let margin = crate::frame::content_margin(self.root.is_some(), self.scale);
        BenchFrame {
            doc,
            selection: None,
            annotations: Some(annotations),
            width: size.width - 2.0 * margin,
            origin: Point::new(margin, margin),
            pointer: None,
            viewport: Some(size.to_rect()),
            root: self.root.as_deref().unwrap_or(&[]),
        }
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
