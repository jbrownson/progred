//! The frame's own rules, for test harnesses: the editor's document view
//! framed headlessly, a placed frame bound with its hover attributed, what
//! the pointer lands on, and what a bench window's view shows.

use super::*;
use crate::annotations::Annotations;
use crate::projection::test_support::{Bench, BenchContext, BenchFrame, FrameTimes, HoverOutput};
use crate::sources::Sources;
use gid::Document;
use puri::draw::DrawList;

/// `world`'s document view projected 500 points wide and placed, unbound.
pub(crate) fn editing_frame(world: &mut Editor) -> HoverOutput<Editor> {
    framed(world, None)
}

/// As [`editing_frame`], the pointer resting at `pointer`. The view draws
/// its document as the editor's would: with Plain's or Raw's projection, or
/// the libraries' less those it hides.
pub(crate) fn framed(world: &mut Editor, pointer: Option<Point>) -> HoverOutput<Editor> {
    let styles = style::editor(world.palette, 1.0);
    world
        .computations
        .begin(world.model.doc.clone(), world.stack.libraries.clone());
    let mut tcx = TextCtx {
        fonts: &mut world.font_cx,
        layouts: &mut world.layout_cx,
        scale: 1.0,
        cache: &mut world.text_cache,
    };
    let view = &world.model.workspace.document;
    let sources = Sources {
        doc: &world.model.doc,
        libraries: &world.stack.libraries,
    };
    let shown = view_content(&world.stack, sources, view, Size::new(500.0, 500.0), 1.0);
    let completions = crate::frame::completions(&world.stack, view);
    let selection = world.model.selection.as_ref();
    let measured = projection::project(
        projection::ProjectDescription {
            focused: world.focused,
            computations: &world.computations,
            view: &view.root,
            completions: Some(&completions),
            sources,
            root: shown.root,
            root_path: &shown.root_path,
            selection,
            source_selection: selection,
            annotations: &view.annotations,
            names: crate::frame::names(&world.stack, view),
            styles: &styles,
            width: 500.0,
            projection: &shown.projection,
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

/// What the pointer would land on at `point` in `dispatch`'s frame.
pub(crate) fn hover_at(dispatch: &Dispatch, point: Point) -> Option<Hovered> {
    hover_target(dispatch.hover_geometry.probe(Some(point), None, 0.0)).0
}

/// Bind `placed` as the frame does, attributing what the pointer rests on
/// through `sources`, then paint and unpack it.
pub(crate) fn settle(mut placed: HoverOutput<crate::Editor>, sources: &Sources<'_>) -> Bench {
    let binding = std::time::Instant::now();
    #[cfg(feature = "layout-profile")]
    let profile = crate::display::profile::enter(crate::display::profile::Kind::Hover);
    let claim = placed.claim.take();
    let hit = claim.as_ref().map(|(_, claim)| claim.clone());
    let (hovered, root) = hover_target(claim);
    let attribution = attribute(
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
        let shown = view_content(
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
        let margin = content_margin(self.root.is_some(), self.scale);
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
