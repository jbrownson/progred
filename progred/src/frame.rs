//! One staged pass: place the document, probe hover, mint dispatch.
//! Ink stays latent in the returned frame; rendering it is the
//! caller's choice, so a silent mint never draws.

use crate::hover;
use crate::menu;
use crate::model::{Model, ViewFlags};
use crate::navigate;
use crate::placed::{self, Placed};
use crate::projection;
use crate::selection;
use crate::sources;
use crate::stack;
use crate::workspace::{self, Root};
use crate::{Editor, PendingPaint, content_viewport};
use kurbo::{Affine, Insets, Point, Rect, Size, Stroke, Vec2};
use parley::{FontContext, LayoutContext};
use peniko::{Brush, Color, ImageData};
use puri::draw::{Canvas, GlyphRun, Shape};
use puri::edit::EditCtx;
use puri::geometry::Placement;
use puri::handler::{Handler, HasHandler, ScrollOutcome};
use puri::hover::Claim;
use puri::interact::is_primary_contact;
use puri::text::TextCtx;
#[cfg(not(target_arch = "wasm32"))]
use puri_vello::VelloCanvas;
#[cfg(target_arch = "wasm32")]
use puri_web::WebCanvas;
use std::rc::Rc;
use ui_events::ScrollDelta;
#[cfg(not(target_arch = "wasm32"))]
use vello::Scene;
use winit::dpi::PhysicalPosition;

pub(crate) const HOVER_REACH: f64 = 8.0;

pub(crate) struct Dispatch {
    pub(crate) handler: Handler<Editor, placed::DispatchContext<Editor>>,
    pub(crate) pointer_root: Option<crate::workspace::Root>,
    pub(crate) descends: Rc<[navigate::Descend<Editor>]>,
    pub(crate) view_regions: Vec<placed::ViewRegion>,
    /// One nominal line height at the frame's scale — the quantum
    /// keyboard navigation reads rows with.
    pub(crate) line: f64,
}

/// One minted frame: the dispatch the shell retains, and the ink the
/// pass deferred — run it into a [`Paint`] or drop it silently.
pub(crate) struct Frame {
    pub(crate) dispatch: Dispatch,
    pub(crate) renders: Vec<placed::Render<Paint>>,
    /// The cell-relative location the resolved hover refers to, for
    /// the render pass's secondary marks.
    pub(crate) hovered_secondary: Option<hover::Secondary>,
    pub(crate) hovered_trace: Option<hover::SourceTrace>,
}

/// What the resting pointer claims in the document or application
/// chrome.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Hovered {
    Tree(hover::Hover),
    Menu(menu::Hover),
    Divider(workspace::Divider),
    /// Pointer-occupied chrome with no editor action. Keeping this
    /// distinct from air prevents the shell's empty-space fallback
    /// without inventing a clickable identity for the chrome.
    Blocked,
}

/// The concrete native canvas frame ink renders into: the Vello scene,
/// owned so deferred ink closures need no lifetime.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct Paint {
    pub(crate) scene: Scene,
}

#[cfg(not(target_arch = "wasm32"))]
impl Canvas for Paint {
    fn image(&mut self, image: ImageData, transform: Affine) {
        VelloCanvas(&mut self.scene).image(image, transform);
    }

    fn fill(&mut self, shape: impl Into<Shape>, brush: impl Into<Brush>, transform: Affine) {
        VelloCanvas(&mut self.scene).fill(shape, brush, transform);
    }

    fn stroke(
        &mut self,
        shape: impl Into<Shape>,
        style: Stroke,
        brush: impl Into<Brush>,
        transform: Affine,
    ) {
        VelloCanvas(&mut self.scene).stroke(shape, style, brush, transform);
    }

    fn glyph_run(&mut self, run: GlyphRun) {
        VelloCanvas(&mut self.scene).glyph_run(run);
    }

    fn clip(
        &mut self,
        shape: impl Into<Shape>,
        transform: Affine,
        content: impl FnOnce(&mut Self),
    ) {
        let shape = shape.into();
        VelloCanvas(&mut self.scene).push_clip(&shape, transform);
        content(self);
        VelloCanvas(&mut self.scene).pop_clip();
    }
}

/// The same deferred Puri ink, interpreted immediately by Canvas2D.
#[cfg(target_arch = "wasm32")]
pub(crate) struct Paint {
    pub(crate) canvas: WebCanvas,
}

#[cfg(target_arch = "wasm32")]
impl Canvas for Paint {
    fn image(&mut self, image: ImageData, transform: Affine) {
        self.canvas.image(image, transform);
    }

    fn fill(&mut self, shape: impl Into<Shape>, brush: impl Into<Brush>, transform: Affine) {
        self.canvas.fill(shape, brush, transform);
    }

    fn stroke(
        &mut self,
        shape: impl Into<Shape>,
        style: Stroke,
        brush: impl Into<Brush>,
        transform: Affine,
    ) {
        self.canvas.stroke(shape, style, brush, transform);
    }

    fn glyph_run(&mut self, run: GlyphRun) {
        self.canvas.glyph_run(run);
    }

    fn clip(
        &mut self,
        shape: impl Into<Shape>,
        transform: Affine,
        content: impl FnOnce(&mut Self),
    ) {
        let shape = shape.into();
        self.canvas.push_clip(&shape, transform);
        content(self);
        self.canvas.pop_clip();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FrameDisposition {
    Retain,
    Remint { reveal_selection: bool },
}

pub(crate) fn frame_disposition(handled: bool, frame_input_changed: bool) -> FrameDisposition {
    if handled {
        FrameDisposition::Remint {
            reveal_selection: true,
        }
    } else if frame_input_changed {
        FrameDisposition::Remint {
            reveal_selection: false,
        }
    } else {
        FrameDisposition::Retain
    }
}

/// Returns the vertical scroll needed to reveal `target`. A visible
/// top edge is already a useful orientation anchor, even when the
/// target extends below the viewport, so it is left undisturbed.
fn reveal_vertical_scroll(
    current: f64,
    maximum: f64,
    target: Rect,
    viewport: Rect,
    pad: f64,
    scale: f64,
) -> f64 {
    reveal_axis(
        current,
        maximum,
        target.y0,
        target.y1,
        viewport.y0,
        viewport.y1,
        pad,
        scale,
    )
}

fn reveal_axis(
    current: f64,
    maximum: f64,
    start: f64,
    end: f64,
    viewport_start: f64,
    viewport_end: f64,
    pad: f64,
    scale: f64,
) -> f64 {
    let current = current.clamp(0.0, maximum);
    if (viewport_start..=viewport_end).contains(&start) {
        return current;
    }
    let mut scroll = current;
    if end > viewport_end {
        scroll += (end + pad - viewport_end) / scale;
    }
    // Checked against the adjusted position, so a target taller than
    // the viewport lands with its top visible.
    let adjusted_start = start - (scroll - current) * scale;
    if adjusted_start < viewport_start {
        scroll += (adjusted_start - pad - viewport_start) / scale;
    }
    scroll.clamp(0.0, maximum)
}

fn drawing_source_descend<'a, World>(
    sources: &sources::Sources<'_>,
    descends: &'a [navigate::Descend<World>],
    source: &hover::SourceTrace,
) -> Option<&'a navigate::Descend<World>> {
    descends.iter().find_map(|descend| {
        (descend.root.is_some()
            && hover::SourceTrace::from_path(sources, descend.path.clone()) == *source)
            .then_some(descend)
    })
}

fn drawing_source_target<World>(
    sources: &sources::Sources<'_>,
    descends: &[navigate::Descend<World>],
    source: &hover::SourceTrace,
) -> Option<(workspace::Root, Rect)> {
    drawing_source_descend(sources, descends, source)
        .and_then(|descend| descend.root.clone().map(|root| (root, descend.rect)))
}

pub(crate) fn scroll_offset(
    stored: Vec2,
    update: &ui_events::pointer::PointerScrollEvent,
    scale: f64,
    viewport: Size,
    maximum: Vec2,
) -> (Vec2, ScrollOutcome) {
    let line = 40.0 * scale;
    // A split can transiently have no room while its window is being
    // resized. Page deltas still need finite units and remainders.
    let page = PhysicalPosition {
        x: viewport.width.max(1.0),
        y: viewport.height.max(1.0),
    };
    let delta = update
        .delta
        .to_pixel_delta(PhysicalPosition { x: line, y: line }, page);
    let current = Vec2::new(
        stored.x.clamp(0.0, maximum.x),
        stored.y.clamp(0.0, maximum.y),
    );
    let next = Vec2::new(
        (current.x - delta.x / scale).clamp(0.0, maximum.x),
        (current.y - delta.y / scale).clamp(0.0, maximum.y),
    );
    let outcome = if next != stored {
        let remaining = PhysicalPosition {
            x: delta.x - (current.x - next.x) * scale,
            y: delta.y - (current.y - next.y) * scale,
        };
        ScrollOutcome::with_remainder(match update.delta {
            ScrollDelta::PageDelta(_, _) => {
                ScrollDelta::PageDelta((remaining.x / page.x) as f32, (remaining.y / page.y) as f32)
            }
            ScrollDelta::LineDelta(_, _) => {
                ScrollDelta::LineDelta((remaining.x / line) as f32, (remaining.y / line) as f32)
            }
            ScrollDelta::PixelDelta(_) => ScrollDelta::PixelDelta(remaining),
        })
    } else {
        ScrollOutcome::pass(update)
    };
    (next, outcome)
}

/// The frame's hover, derived from this pass's settled geometry: a
/// direct claim under the pointer answers outright, an extension may
/// retain only the prior target, and an occluder blocks both. A
/// pressed gesture keeps the hover it began with.
pub(crate) fn derive_hover<C: 'static, Cv>(
    placed: &Placed<C, Cv>,
    prior: Option<Hovered>,
    pointer: Option<Point>,
    pressed: bool,
    reach: f64,
) -> (Option<Hovered>, Option<crate::workspace::Root>) {
    if pressed {
        return (prior, None);
    }
    match pointer.and_then(|point| placed.probe_scoped(point, prior.as_ref(), reach)) {
        Some((root, Claim::Direct(target) | Claim::Extended(target))) => (Some(target), root),
        Some((root, Claim::Occludes)) => (Some(Hovered::Blocked), root),
        None => (None, None),
    }
}

fn source_hover_visible(hover: Option<&Hovered>, linking: bool) -> bool {
    !matches!(hover, Some(Hovered::Tree(hover::Hover::Drawing(_)))) || linking
}

pub(crate) struct FrameDescription<'a> {
    drawn_menu: bool,
    model: &'a Model,
    stack: &'a stack::Stack<Editor>,
    menu: menu::State,
    availability: crate::command::Availability,
    toggles: crate::command::Toggles,
    scale: f64,
    viewport: Size,
    scrub: Option<crate::gesture::ScrubSpelling<'a>>,
}

pub(crate) struct FrameResources<'a> {
    fonts: &'a mut FontContext,
    layouts: &'a mut LayoutContext<Brush>,
    text_cache: &'a mut puri::text::TextCache,
}

/// The frame as one measured value, plus the scroll maxima its
/// measurement settled.
struct AppView {
    view: measured::Measured<Placed<Editor, Paint>>,
}

impl Editor {
    pub(crate) fn scroll_view(
        &mut self,
        root: Root,
        update: &ui_events::pointer::PointerScrollEvent,
        scale: f64,
        viewport: Size,
        max_scroll: f64,
        max_scroll_x: f64,
    ) -> ScrollOutcome {
        // ScrollDelta documents positive as viewport-down/right, but
        // ui-events-winit passes winit deltas through raw, where
        // positive is scroll-up/left; subtract to match reality.
        // Stepping from the clamped position keeps the first tick
        // responsive when a resize left the stored offset out of
        // bounds.
        let (next, outcome) = scroll_offset(
            self.model
                .workspace
                .view(&root)
                .expect("a retained view is live")
                .scroll,
            update,
            scale,
            viewport,
            Vec2::new(max_scroll_x, max_scroll),
        );
        if let Some(view) = self.model.workspace.view_mut(&root) {
            view.scroll = next;
        }
        outcome
    }

    pub(crate) fn select_drawing_source(
        &mut self,
        descends: &[navigate::Descend<Editor>],
        source: &hover::SourceTrace,
    ) {
        let select = drawing_source_descend(&self.sources(), descends, source)
            .map(|descend| descend.select.clone());
        if let Some(select) = select {
            select(self);
        }
    }

    /// Scroll-to-reveal, computed from the freshly retained dispatch
    /// pass BEFORE anything draws, so the reveal lands in the next
    /// presented frame with no corrective flash. Fires once per
    /// selection-identity change (path AND variant — Enter keeps the
    /// path while opening a pending), so it never fights manual
    /// scrolling. A pending's ordinary descend is its authoring row,
    /// not the out-of-flow completion card.
    pub(crate) fn reveal_selection(&mut self, dispatch: &Dispatch, scale: f64) -> bool {
        let reveal = self
            .model
            .selection
            .as_ref()
            .map(|s| (s.root().clone(), s.path().to_vec(), s.stage()));
        if reveal == self.revealed {
            false
        } else {
            self.revealed = reveal.clone();
            let target = reveal.as_ref().and_then(|(root, path, _)| {
                dispatch
                    .descends
                    .iter()
                    .find(|descend| {
                        descend.root.as_ref() == Some(root) && descend.path.as_ref() == path
                    })
                    .map(|descend| descend.rect)
                    .map(|rect| (root.clone(), rect))
            });
            target.is_some_and(|(view, rect)| self.reveal_rect(dispatch, &view, rect, scale))
        }
    }

    fn reveal_drawing_source(&mut self, dispatch: &Dispatch, scale: f64) -> bool {
        if !crate::modifiers::link(&self.modifiers) {
            return false;
        }
        let Some(Hovered::Tree(hover::Hover::Drawing(source))) = &self.hover else {
            return false;
        };
        let target = {
            let sources = self.sources();
            drawing_source_target(&sources, &dispatch.descends, source)
        };
        target.is_some_and(|(view, rect)| self.reveal_rect(dispatch, &view, rect, scale))
    }

    fn reveal_rect(
        &mut self,
        dispatch: &Dispatch,
        view: &workspace::Root,
        rect: Rect,
        scale: f64,
    ) -> bool {
        let pad = 12.0 * scale;
        let Some(region) = dispatch
            .view_regions
            .iter()
            .find(|region| &region.root == view)
        else {
            return false;
        };
        let Some(before) = self.model.workspace.view(view).map(|view| view.scroll) else {
            return false;
        };
        let next = Vec2::new(
            reveal_axis(
                before.x,
                region.maximum.x,
                rect.x0,
                rect.x1,
                region.rect.x0,
                region.rect.x1,
                pad,
                scale,
            ),
            reveal_vertical_scroll(before.y, region.maximum.y, rect, region.rect, pad, scale),
        );
        if let Some(view) = self.model.workspace.view_mut(view) {
            view.scroll = next;
        }
        next != before
    }

    pub(crate) fn view_flags(&self) -> ViewFlags {
        self.model.view
    }

    /// One staged pass over the UI: place, probe the pointer against
    /// the settled geometry, resolve hover, mint dispatch. Ink comes
    /// back deferred; the caller renders it or drops it.
    pub(crate) fn build_frame(&mut self, scale: f64, viewport: Size) -> Frame {
        let declarations = workspace::declarations(self.model.doc.root.as_ref());
        self.model.workspace.sync_declared(&declarations);
        if self
            .model
            .selection
            .as_ref()
            .is_some_and(|selection| self.model.workspace.view(selection.root()).is_none())
        {
            self.model.selection = None;
        }
        let view = self.view_flags();
        let debug_geometry = view.debug_geometry;
        let availability = self.menu_availability();
        let description = FrameDescription {
            drawn_menu: self.drawn_menu,
            toggles: self.menu_toggles(),
            model: &self.model,
            stack: &self.stack,
            menu: self.menu,
            availability,
            scale,
            viewport,
            scrub: self
                .gesture
                .as_ref()
                .and_then(|gesture| gesture.scrub_spelling()),
        };
        let resources = FrameResources {
            fonts: &mut self.font_cx,
            layouts: &mut self.layout_cx,
            text_cache: &mut self.text_cache,
        };
        let AppView { view } = app_view(description, resources);
        let placed = measured::place(
            view,
            Placement::root(Rect::from_origin_size(Point::ZERO, viewport)),
        )
        .raise_floaters();
        let hover_reach = HOVER_REACH * scale;
        let (hover, pointer_root) = derive_hover(
            &placed,
            self.hover.take(),
            self.pointer,
            self.pressed,
            hover_reach,
        );
        self.hover = hover;
        let sources = sources::Sources {
            doc: &self.model.doc,
            libraries: &self.stack.libraries,
        };
        let show_source_hover =
            source_hover_visible(self.hover.as_ref(), crate::modifiers::link(&self.modifiers));
        let hovered_secondary = match &self.hover {
            Some(Hovered::Tree(_)) if !show_source_hover => None,
            Some(Hovered::Tree(hover)) => {
                hover::hover_secondary(&sources, placed.completion.as_ref(), hover)
            }
            Some(Hovered::Menu(_)) => None,
            Some(Hovered::Divider(_)) => None,
            Some(Hovered::Blocked) => None,
            None => None,
        };
        let hovered_trace = match &self.hover {
            Some(Hovered::Tree(_)) if !show_source_hover => None,
            Some(Hovered::Tree(hover::Hover::Value(path))) => {
                Some(hover::SourceTrace::from_path(&sources, path.clone()))
            }
            Some(Hovered::Tree(hover::Hover::Drawing(source))) => Some(source.clone()),
            _ => None,
        };
        let extended_rects = debug_geometry
            .then(|| {
                self.hover
                    .as_ref()
                    .map(|hover| placed.extended_rects(hover, hover_reach))
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        let Placed {
            probes: _,
            handler,
            descends,
            view_regions,
            landmark_select,
            completion: _,
            floaters: _,
            mut renders,
        } = placed;
        debug_assert!(
            landmark_select.is_none(),
            "selection handler escaped its landmark"
        );
        if debug_geometry {
            renders.push(Box::new(move |canvas, _| {
                let guide = Color::new([0.92, 0.12, 0.58, 0.80]);
                for rect in extended_rects {
                    canvas.stroke(rect, Stroke::new(1.0), guide, Affine::IDENTITY);
                }
            }));
        }
        Frame {
            dispatch: Dispatch {
                handler: handler.unwrap_or_else(Handler::new),
                pointer_root,
                descends: descends.into(),
                view_regions,
                line: 14.0 * scale,
            },
            renders,
            hovered_secondary,
            hovered_trace,
        }
    }

    /// Mint dispatch data from the final state of a transition. A
    /// silent pass supplies reveal geometry and resolves hover;
    /// scrolling to reveal changes geometry and earns one rebuild.
    pub(crate) fn retain_dispatch(&mut self, scale: f64, viewport: Size, reveal_selection: bool) {
        let mut frame = self.build_frame(scale, viewport);
        let revealed_selection = reveal_selection && self.reveal_selection(&frame.dispatch, scale);
        let revealed_source = self.reveal_drawing_source(&frame.dispatch, scale);
        if revealed_selection || revealed_source {
            frame = self.build_frame(scale, viewport);
        }
        self.pending_paint = Some(self.install_frame(frame, scale, viewport));
    }

    fn install_frame(&mut self, frame: Frame, scale: f64, viewport: Size) -> PendingPaint {
        let Frame {
            dispatch,
            renders,
            hovered_secondary,
            hovered_trace,
        } = frame;
        self.dispatch = Some(dispatch);
        PendingPaint {
            scale,
            viewport,
            renders,
            hovered_secondary,
            hovered_trace,
        }
    }

    pub(crate) fn prepare_paint(&mut self, scale: f64, viewport: Size) -> PendingPaint {
        self.pending_paint
            .take()
            .filter(|pending| pending.scale == scale && pending.viewport == viewport)
            .unwrap_or_else(|| {
                let frame = self.build_frame(scale, viewport);
                self.install_frame(frame, scale, viewport)
            })
    }
}

fn projection_hooks(root: Root) -> projection::Hooks<Editor> {
    let select_root = root.clone();
    let edit_root = root.clone();
    let payload_root = root.clone();
    let toggle_root = root.clone();
    let insert_root = root.clone();
    let apply_root = root.clone();
    let point_root = root.clone();
    let completion_root = root.clone();
    let drag_root = root.clone();
    let scrub_root = root.clone();
    let state_root = root;
    let select: Rc<dyn Fn(&mut Editor, gid::Path)> = Rc::new(move |app, path| {
        let fresh = match app.model.selection.as_ref() {
            None => true,
            Some(current) => {
                current.root() != &select_root
                    || current.stage() == selection::Stage::Label
                    || current.path() != path
            }
        };
        if fresh {
            let next = selection::Selection::edge(&select_root, &app.sources(), path);
            app.model.selection = Some(next);
        } else if let Some(line) = app
            .model
            .selection
            .as_mut()
            .and_then(selection::Selection::edit_mut)
        {
            line.cursor_to_end();
        }
    });
    projection::Hooks {
        select: select.clone(),
        select_source: Rc::new(Editor::select_drawing_source),
        select_payload: Rc::new(move |app: &mut Editor, path, payload| {
            app.model.selection = Some(selection::Selection::from_payload(
                &payload_root,
                &app.sources(),
                path,
                payload,
            ));
        }),
        start_edit: Rc::new(move |app: &mut Editor, path, line| {
            app.model.selection = Some(selection::Selection::from_line(
                &edit_root,
                &app.sources(),
                path,
                line,
            ));
        }),
        toggle: Rc::new(move |app: &mut Editor, path| {
            let Some(view) = app.model.workspace.view_mut(&toggle_root) else {
                return;
            };
            selection::toggle_collapse(
                &sources::Sources {
                    doc: &app.model.doc,
                    libraries: &app.stack.libraries,
                },
                &mut view.annotations,
                &path,
            );
        }),
        update_state: Rc::new(move |app: &mut Editor, path, state| {
            let Some(view) = app.model.workspace.view_mut(&state_root) else {
                return false;
            };
            if view.annotations.at(&path) == Some(&state) {
                false
            } else {
                view.annotations.set(&path, Some(state));
                true
            }
        }),
        edit: Rc::new(edit_ctx),
        pick: Rc::new(|app: &mut Editor, id| app.pick_identity(id)),
        insert: Rc::new(move |app: &mut Editor, path| {
            if let Some(pending) = selection::pending_after(&insert_root, &app.sources(), &path) {
                app.model.selection = Some(pending);
            }
        }),
        delete: Rc::new(Editor::delete_selected_edge),
        apply: Rc::new(move |app, path, function, event| {
            crate::site::apply_event(app, apply_root.clone(), path, function, event)
        }),
        state_drag: Rc::new(move |app, path, handler, point, scale| {
            app.gesture = Some(crate::gesture::state_drag(
                point,
                scale,
                drag_root.clone(),
                path,
                handler,
            ));
        }),
        scrub: Rc::new(move |app, path, handler, point, scale| {
            if app.model.selection.as_ref().is_some_and(|selection| {
                matches!(
                    selection.stage(),
                    selection::Stage::Pending | selection::Stage::Label
                )
            }) {
                false
            } else {
                select(app, path.clone());
                app.gesture = Some(crate::gesture::scrub(
                    point,
                    scale,
                    scrub_root.clone(),
                    path,
                    handler,
                ));
                true
            }
        }),
        point: Rc::new(move |app, path, placement, handler, point| {
            app.gesture = Some(crate::gesture::point(
                point_root.clone(),
                path,
                placement.rect,
                handler,
            ));
            app.advance_gesture(point)
        }),
        commit_value: Rc::new(|app: &mut Editor, value, on_commit| {
            if app.model
                .selection
                .as_ref()
                .is_some_and(|current| current.stage() == selection::Stage::Pending)
            {
                app.commit_completion(value, None, on_commit);
            }
        }),
        commit_label: Rc::new(|app: &mut Editor, label, definition, on_commit| {
            if app.model
                .selection
                .as_ref()
                .is_some_and(|current| current.stage() == selection::Stage::Label)
            {
                app.commit_completion(label.into(), definition, on_commit);
            }
        }),
        set_completion_view: Rc::new(move |app: &mut Editor, scroll, choice, everything| {
            if let Some(selection) = app.model.selection.as_mut()
                && selection.root() == &completion_root
                && selection.stage() != selection::Stage::Edge
            {
                selection.set_completion_view(scroll, choice, everything);
            }
        }),
    }
}

#[allow(clippy::too_many_arguments)]
fn project_workspace_view(
    model: &Model,
    stack: &stack::Stack<Editor>,
    styles: &crate::styles::Styles,
    tcx: &mut TextCtx,
    sources: sources::Sources<'_>,
    view: &workspace::View,
    scrub: Option<&crate::gesture::ScrubSpelling<'_>>,
    size: Size,
    scale: f64,
) -> measured::Measured<Placed<Editor, Paint>> {
    let margin = 12.0 * scale;
    let body_width = (size.width - 2.0 * margin).max(0.0);
    let root_path;
    let root_completions = matches!(view.root.target(), workspace::Target::Document)
        .then_some(&stack.root_completions);
    let root_field_completions = matches!(view.root.target(), workspace::Target::Document)
        .then_some(&stack.root_field_completions);
    let (root, projection) = match view.root.target() {
        workspace::Target::Document => {
            root_path = Vec::new();
            (sources.root(), &stack.projection)
        }
        workspace::Target::Pane { path } => {
            root_path = path.clone();
            (sources.resolve_path(path), &stack.pane_projection)
        }
    };
    let raw = view.projection == workspace::Projection::Raw;
    let projected = projection::project(
        projection::ProjectDescription {
            sources,
            root,
            root_path: &root_path,
            selection: model
                .selection
                .as_ref()
                .filter(|selection| selection.root() == &view.root),
            scrub_spelling: scrub
                .filter(|scrub| scrub.root == &view.root)
                .map(|scrub| (scrub.path, scrub.spelling)),
            source_selection: model.selection.as_ref(),
            annotations: &view.annotations,
            raw,
            styles,
            width: body_width,
            projection: (!raw).then_some(projection),
            root_completions: (!raw).then_some(root_completions).flatten(),
            root_field_completions: (!raw).then_some(root_field_completions).flatten(),
        },
        tcx,
        projection_hooks(view.root.clone()),
    );
    let content = measured::pad(Insets::uniform(margin), projected);
    let maximum = Vec2::new(
        ((content.extent.width - size.width) / scale).max(0.0),
        ((content.extent.height() - size.height) / scale).max(0.0),
    );
    let offset = Vec2::new(
        view.scroll.x.clamp(0.0, maximum.x) * scale,
        view.scroll.y.clamp(0.0, maximum.y) * scale,
    );
    let root = view.root.clone();
    let scroll_root = root.clone();
    let scrolled = placed::scrolled_at(
        content,
        offset,
        Some((root.clone(), scale)),
        move |app: &mut Editor, update| {
            app.scroll_view(
                scroll_root.clone(),
                update,
                scale,
                size,
                maximum.y,
                maximum.x,
            )
        },
    );
    let scrolled = placed::in_view(scrolled, root);
    let frame = placed::leaf(
        measured::Extent {
            width: size.width,
            ascent: 0.0,
            descent: size.height,
        },
        |_, _| {},
    );
    measured::overlay(frame, scrolled, move |placement, _, _| {
        Some(Placement::new(placement.rect, placement.clip_rect))
    })
}

#[allow(clippy::too_many_arguments)]
fn project_workspace(
    model: &Model,
    stack: &stack::Stack<Editor>,
    styles: &crate::styles::Styles,
    tcx: &mut TextCtx,
    sources: sources::Sources<'_>,
    scrub: Option<&crate::gesture::ScrubSpelling<'_>>,
    size: Size,
    scale: f64,
) -> measured::Measured<Placed<Editor, Paint>> {
    let geometry = model.workspace.geometry(size, scale);
    let mut body = placed::leaf(
        measured::Extent {
            width: size.width,
            ascent: 0.0,
            descent: size.height,
        },
        |_, _| {},
    );
    for placed_view in geometry.views {
        let view = model
            .workspace
            .view(&placed_view.root)
            .expect("workspace geometry only names live views");
        let rect = placed_view.rect;
        let child = project_workspace_view(
            model,
            stack,
            styles,
            tcx,
            sources,
            view,
            scrub,
            rect.size(),
            scale,
        );
        body = measured::overlay(body, child, move |placement, _, _| {
            let rect = rect + placement.rect.origin().to_vec2();
            Some(Placement::new(rect, placement.clip_rect.intersect(rect)))
        });
    }
    for placed_divider in geometry.dividers {
        let rect = placed_divider.rect;
        let divider = placed_divider.divider;
        let vertical = matches!(divider, workspace::Divider::Columns(_));
        let hover_divider = divider.clone();
        let down_divider = divider.clone();
        let move_divider = divider.clone();
        let up_divider = divider;
        let rule = placed::leaf(
            measured::Extent {
                width: rect.width(),
                ascent: 0.0,
                descent: rect.height(),
            },
            move |p, placement| {
                let hit = if vertical {
                    placement.rect.inflate(4.0 * scale, 0.0)
                } else {
                    placement.rect.inflate(0.0, 4.0 * scale)
                };
                let clip = placement.clip_rect;
                p.fill(
                    placement.rect,
                    Color::new([0.82, 0.83, 0.86, 1.0]),
                    Affine::IDENTITY,
                );
                p.claim_exact(
                    Placement::new(hit, placement.clip_rect),
                    Hovered::Divider(hover_divider),
                );
                p.handler().on_pointer_down(move |app: &mut Editor, event| {
                    is_primary_contact(event)
                        && hit.contains(Point::new(event.state.position.x, event.state.position.y))
                        && app.model.workspace.start_resize(
                            down_divider.clone(),
                            Vec2::new(
                                event.state.position.x - clip.x0,
                                event.state.position.y - clip.y0,
                            ),
                        )
                });
                p.handler().on_pointer_move(move |app: &mut Editor, event| {
                    app.model.workspace.resize(
                        &move_divider,
                        Vec2::new(
                            event.current.position.x - clip.x0,
                            event.current.position.y - clip.y0,
                        ),
                        size,
                    )
                });
                p.handler().on_pointer_up(move |app: &mut Editor, _| {
                    app.model.workspace.finish_resize(&up_divider)
                });
            },
        );
        body = measured::overlay(body, rule, move |placement, _, _| {
            let rect = rect + placement.rect.origin().to_vec2();
            // The rule is inside the workspace, but its retained drag
            // needs the workspace origin after the pointer leaves the
            // narrow hit target.
            Some(Placement::new(rect, placement.clip_rect))
        });
    }
    body
}

fn app_view(description: FrameDescription<'_>, resources: FrameResources<'_>) -> AppView {
    let FrameDescription {
        drawn_menu,
        toggles,
        model,
        stack,
        menu,
        availability,
        scale,
        viewport,
        scrub,
    } = description;
    let FrameResources {
        fonts: font_cx,
        layouts: layout_cx,
        text_cache,
    } = resources;
    let viewport_width = viewport.width;
    // Mark-and-sweep by pass: entries the previous pass never used
    // are dropped here, everything else carries over — the steady
    // state is the visible text, shaped once.
    text_cache.sweep();
    let mut tcx = TextCtx {
        fonts: font_cx,
        layouts: layout_cx,
        scale: scale as f32,
        cache: text_cache,
    };
    let styles = crate::styles::editor(scale);
    let application_menu = drawn_menu.then(|| {
        menu::view(
            &mut tcx,
            menu::Description {
                state: menu,
                availability,
                toggles,
                scale,
                width: viewport_width,
            },
            menu::Hooks {
                toggle: Rc::new(|app: &mut Editor, section| app.menu.toggle(section)),
                select: Rc::new(|app: &mut Editor, selection| app.choose_menu(selection)),
            },
        )
    });
    let (menu_bar, menu_popup, menu_heading_width) = match application_menu {
        Some(menu) => (Some(menu.bar), menu.popup, menu.heading_width),
        None => (None, None, 0.0),
    };
    let content_viewport = content_viewport(drawn_menu, viewport, scale);
    let sources = sources::Sources {
        doc: &model.doc,
        libraries: &stack.libraries,
    };
    let body = project_workspace(
        model,
        stack,
        &styles,
        &mut tcx,
        sources,
        scrub.as_ref(),
        content_viewport.size(),
        scale,
    );
    // The stage is editor chrome around one workspace tree. Empty
    // space deselection remains the shell's final editor action.
    let mut stage = placed::leaf(
        measured::Extent {
            width: viewport.width,
            ascent: 0.0,
            descent: viewport.height,
        },
        |_, _| {},
    );
    if let Some(bar) = menu_bar {
        let bar_placement = Placement::new(
            Rect::new(0.0, 0.0, viewport_width, content_viewport.y0),
            Rect::new(0.0, 0.0, viewport_width, viewport.height),
        );
        stage = measured::overlay(stage, bar, move |_, _, _| Some(bar_placement));
    }
    stage = measured::overlay(stage, body, move |_, _, _| {
        Some(Placement::new(content_viewport, content_viewport))
    });

    if let Some((x, popup)) = menu_popup {
        let heading_width = menu_heading_width;
        // Outside presses close the popup. Its own Occludes claim
        // becomes Hovered::Blocked over separators and disabled
        // entries; enabled items resolve their semantic target above
        // it, so no raw inside-swallow may preempt them.
        let popup = placed::before(popup, move |p, placement| {
            let rect = placement.rect;
            let headings = Rect::new(0.0, 0.0, heading_width, content_viewport.y0);
            p.handler().on_pointer_down(move |app: &mut Editor, event| {
                let point = Point::new(event.state.position.x, event.state.position.y);
                is_primary_contact(event)
                    && !headings.contains(point)
                    && !rect.contains(point)
                    && app.menu.close()
            });
            p.handler().on_scroll(move |_: &mut Editor, event| {
                if rect.contains(Point::new(event.state.position.x, event.state.position.y)) {
                    ScrollOutcome::consume(event)
                } else {
                    ScrollOutcome::pass(event)
                }
            });
        });
        stage = placed::floating(stage, popup, move |placement, extent| {
            Some(Placement::new(
                extent.rect_at(Point::new(x, content_viewport.y0)),
                placement.clip_rect,
            ))
        });
    }
    AppView { view: stage }
}

/// Dispatch-time access to the selection's editor. Retained-frame
/// dispatch can outlive the editor by a frame — deselect, then a move
/// in the same gesture — so absence declines rather than panics.
pub(crate) fn edit_ctx(app: &mut Editor) -> Option<EditCtx<'_>> {
    let Editor {
        model,
        font_cx,
        layout_cx,
        text_clipboard,
        ..
    } = app;
    let state = model
        .selection
        .as_mut()
        .and_then(selection::Selection::edit_mut)?;
    Some(EditCtx {
        state,
        fonts: font_cx,
        layouts: layout_cx,
        clipboard: text_clipboard,
    })
}

#[cfg(test)]
mod frame_tests {
    use super::*;
    use gid::{CellId, Cells, Document, Step, Value};
    use measured::Output;

    #[test]
    fn only_transitions_and_changed_frame_inputs_remint() {
        assert_eq!(frame_disposition(false, false), FrameDisposition::Retain);
        assert_eq!(
            frame_disposition(false, true),
            FrameDisposition::Remint {
                reveal_selection: false,
            }
        );
        assert_eq!(
            frame_disposition(true, false),
            FrameDisposition::Remint {
                reveal_selection: true,
            }
        );
        assert_eq!(
            frame_disposition(true, true),
            FrameDisposition::Remint {
                reveal_selection: true,
            }
        );
    }

    #[test]
    fn source_hover_is_immediate_from_code_and_explicit_from_drawing() {
        let code = Hovered::Tree(hover::Hover::Value(Rc::from([])));
        let drawing = Hovered::Tree(hover::Hover::Drawing(hover::SourceTrace::Stored(Rc::from(
            [],
        ))));

        assert!(source_hover_visible(Some(&code), false));
        assert!(!source_hover_visible(Some(&drawing), false));
        assert!(source_hover_visible(Some(&drawing), true));
    }

    #[test]
    fn an_oversized_target_with_a_visible_top_does_not_scroll() {
        let viewport = Rect::new(0.0, 30.0, 400.0, 200.0);
        let target = Rect::new(20.0, 80.0, 380.0, 500.0);

        assert_eq!(
            reveal_vertical_scroll(120.0, 1_000.0, target, viewport, 12.0, 1.0),
            120.0
        );
    }

    #[test]
    fn a_target_starting_below_the_viewport_is_still_revealed() {
        let viewport = Rect::new(0.0, 30.0, 400.0, 200.0);
        let target = Rect::new(20.0, 220.0, 380.0, 260.0);

        assert_eq!(
            reveal_vertical_scroll(120.0, 1_000.0, target, viewport, 12.0, 1.0),
            192.0
        );
    }

    #[test]
    fn reveal_clamps_an_offset_left_stale_by_a_resize() {
        let viewport = Rect::new(0.0, 30.0, 400.0, 200.0);
        let target = Rect::new(20.0, 80.0, 380.0, 120.0);

        assert_eq!(
            reveal_vertical_scroll(300.0, 100.0, target, viewport, 12.0, 1.0),
            100.0
        );
    }

    #[test]
    fn a_drawing_source_selects_its_definition_among_equal_cell_paths() {
        let cell = CellId::from_u128(1);
        let call = CellId::from_u128(2);
        let library_id = CellId::from_u128(3);
        let mut cells = Cells::new();
        cells.set_value(cell, Value::record([(call, Value::from(vec![1]))]));
        let doc = Document {
            root: Some(Value::Cell(cell)),
            cells,
        };
        let libraries = progred_libraries::Libraries::from_contributions([(
            library_id,
            progred_libraries::Library::<(), ()>::named(
                "source",
                progred_libraries::Definitions::from_parts(
                    doc.cells.clone(),
                    grap::ForeignFunctions::default(),
                ),
                vec![],
            ),
        )])
        .0;
        let sources = sources::Sources {
            doc: &doc,
            libraries: &libraries,
        };
        let root = workspace::Root::document();
        let targets = [
            (gid::Resolution::Document, Rect::new(10.0, 20.0, 30.0, 40.0)),
            (
                gid::Resolution::Library(library_id),
                Rect::new(10.0, 50.0, 30.0, 70.0),
            ),
        ];
        let descends = targets.map(
            |(source, rect)| navigate::Descend::<Option<gid::Resolution>> {
                root: Some(root.clone()),
                path: Rc::from([Step::Follow(source), Step::Key(call)]),
                rect,
                select: Rc::new(move |selected| {
                    *selected = Some(source);
                    true
                }),
            },
        );
        for (source, rect) in targets {
            let trace = hover::SourceTrace::InCell {
                cell,
                source,
                path: Rc::from([Step::Key(call)]),
            };
            assert_eq!(
                drawing_source_target(&sources, &descends, &trace),
                Some((root.clone(), rect))
            );
            let mut selected = None;
            assert!((drawing_source_descend(&sources, &descends, &trace)
                .unwrap()
                .select)(&mut selected));
            assert_eq!(selected, Some(source));
        }
    }

    #[test]
    fn pane_presentations_do_not_run_in_the_document_and_raw_keeps_the_declaration() {
        use progred_libraries::{Definitions, presentation};
        use std::cell::{Cell, RefCell};

        let projector = CellId::from_u128(1);
        let linked = CellId::from_u128(2);
        let source = Value::from(b"source".to_vec());
        let declaration = Value::record([
            (presentation::vocabulary::VALUE, source.clone()),
            (presentation::vocabulary::PROJECTION, Value::from(projector)),
        ]);
        let mut cells = Cells::new();
        cells.set_value(linked, declaration.clone());
        let mut model = Model {
            doc: Document {
                root: Some(Value::record([])),
                cells,
            },
            selection: None,
            history: crate::history::History::default(),
            view: ViewFlags::default(),
            workspace: workspace::Workspace::default(),
        };
        for value in [
            declaration,
            Value::from(linked),
            source.clone(),
            Value::record([(presentation::vocabulary::VALUE, source.clone())]),
        ] {
            model.doc.root = Some(
                workspace::append(
                    model.doc.root.as_ref().unwrap(),
                    workspace::Side::Left,
                    value,
                )
                .unwrap()
                .0,
            );
        }
        model
            .workspace
            .sync_declared(&workspace::declarations(model.doc.root.as_ref()));
        for declaration in workspace::declarations(model.doc.root.as_ref()) {
            crate::annotations::set_collapsed(
                &mut model.workspace.document.annotations,
                &declaration.path,
                false,
                false,
            );
        }
        let calls = Rc::new(Cell::new(0));
        let result = Rc::new(RefCell::new(Value::from(b"presented".to_vec())));
        let mut stack = stack::load::<Editor>();
        stack.libraries.insert(
            CellId::from_u128(3),
            Value::record([]),
            Definitions::from_parts(
                Cells::new(),
                grap::ForeignFunctions::default().register(
                    projector,
                    grap::ForeignFunction::new({
                        let calls = calls.clone();
                        let result = result.clone();
                        move |context, call, environment| {
                            let value = context
                                .field(call, presentation::vocabulary::VALUE)
                                .unwrap();
                            assert_eq!(context.eval(value, environment)?, source);
                            calls.set(calls.get() + 1);
                            Ok(result.borrow().clone())
                        }
                    }),
                ),
            ),
        );
        let styles = crate::styles::editor(1.0);
        let mut fonts = FontContext::new();
        let mut layouts = LayoutContext::new();
        let mut cache = puri::text::TextCache::default();
        let mut tcx = TextCtx {
            fonts: &mut fonts,
            layouts: &mut layouts,
            scale: 1.0,
            cache: &mut cache,
        };
        let size = Size::new(1200.0, 1000.0);
        let mut place = |model: &Model| {
            measured::place(
                project_workspace(
                    model,
                    &stack,
                    &styles,
                    &mut tcx,
                    sources::Sources {
                        doc: &model.doc,
                        libraries: &stack.libraries,
                    },
                    None,
                    size,
                    1.0,
                ),
                Placement::root(Rect::from_origin_size(Point::ZERO, size)),
            )
        };
        let document = model.workspace.document_root();
        let sources: Vec<_> = model
            .workspace
            .left
            .panes
            .iter()
            .take(2)
            .enumerate()
            .map(|(index, pane)| {
                let workspace::Target::Pane { path } = pane.view.root.target() else {
                    panic!("pane path")
                };
                let mut path = path.clone();
                if index == 1 {
                    path.push(Step::Follow(gid::Resolution::Document));
                }
                (pane.view.root.clone(), path)
            })
            .collect();
        let shown = place(&model);
        assert_eq!(
            calls.replace(0),
            2,
            "only the two preview panes apply the projection"
        );
        for (pane, path) in &sources {
            for field in [
                presentation::vocabulary::VALUE,
                presentation::vocabulary::PROJECTION,
            ] {
                let mut field_path = path.clone();
                field_path.push(Step::Key(field));
                assert!(
                    shown
                        .descends
                        .iter()
                        .any(|target| target.root.as_ref() == Some(document)
                            && target.path.as_ref() == field_path)
                );
                assert!(
                    !shown
                        .descends
                        .iter()
                        .any(|target| target.root.as_ref() == Some(pane)
                            && target.path.as_ref() == field_path)
                );
            }
        }
        for pane in &mut model.workspace.left.panes {
            pane.view.projection = workspace::Projection::Raw;
        }
        let raw = place(&model);
        assert_eq!(calls.replace(0), 0);
        for (pane, path) in &sources {
            for field in [
                presentation::vocabulary::VALUE,
                presentation::vocabulary::PROJECTION,
            ] {
                let mut field_path = path.clone();
                field_path.push(Step::Key(field));
                assert!(
                    raw.descends
                        .iter()
                        .any(|target| target.root.as_ref() == Some(pane)
                            && target.path.as_ref() == field_path)
                );
            }
        }
        for pane in &mut model.workspace.left.panes {
            pane.view.projection = workspace::Projection::Standard;
        }
        *result.borrow_mut() = progred_libraries::absent::with_reason(projector);
        let absent = place(&model);
        assert_eq!(calls.get(), 2);
        for (pane, path) in &sources {
            let mut source_path = path.clone();
            source_path.push(Step::Key(presentation::vocabulary::VALUE));
            assert!(
                absent
                    .descends
                    .iter()
                    .any(|target| target.root.as_ref() == Some(pane)
                        && target.path.as_ref() == source_path)
            );
        }
    }

    #[test]
    fn workspace_columns_are_editor_geometry_with_independent_view_regions() {
        let cell = CellId::from_u128(1);
        let mut cells = Cells::new();
        cells.set_value(cell, Value::from(b"pane".to_vec()));
        let mut model = Model {
            doc: Document {
                root: Some(Value::record([])),
                cells,
            },
            selection: None,
            history: crate::history::History::default(),
            view: ViewFlags::default(),
            workspace: workspace::Workspace::default(),
        };
        let document = model.workspace.document_root().clone();
        for _ in 0..2 {
            model.doc.root = Some(
                workspace::append(
                    model.doc.root.as_ref().unwrap(),
                    workspace::Side::Left,
                    Value::from(cell),
                )
                .unwrap()
                .0,
            );
        }
        model
            .workspace
            .sync_declared(&workspace::declarations(model.doc.root.as_ref()));
        let upper = model.workspace.left.panes[0].view.root.clone();
        let lower = model.workspace.left.panes[1].view.root.clone();
        let stack = crate::stack::load::<Editor>();
        let styles = crate::styles::editor(1.0);
        let mut fonts = FontContext::new();
        let mut layouts = LayoutContext::new();
        let mut cache = puri::text::TextCache::default();
        let mut tcx = TextCtx {
            fonts: &mut fonts,
            layouts: &mut layouts,
            scale: 1.0,
            cache: &mut cache,
        };
        let size = Size::new(801.0, 600.0);
        let placed = measured::place(
            project_workspace(
                &model,
                &stack,
                &styles,
                &mut tcx,
                sources::Sources {
                    doc: &model.doc,
                    libraries: &stack.libraries,
                },
                None,
                size,
                1.0,
            ),
            Placement::root(Rect::from_origin_size(Point::ZERO, size)),
        );

        assert_eq!(placed.view_regions.len(), 3);
        let upper_region = placed
            .view_regions
            .iter()
            .find(|region| region.root == upper)
            .expect("upper pane");
        let lower_region = placed
            .view_regions
            .iter()
            .find(|region| region.root == lower)
            .expect("lower pane");
        let document_region = placed
            .view_regions
            .iter()
            .find(|region| region.root == document)
            .expect("document view");
        assert_eq!(
            (document_region.rect.y0, document_region.rect.y1),
            (0.0, 600.0)
        );
        assert_eq!(upper_region.rect.x0, lower_region.rect.x0);
        assert_eq!(upper_region.rect.x1, lower_region.rect.x1);
        assert_eq!(lower_region.rect.y0 - upper_region.rect.y1, 1.0);
        assert!(placed.descends.iter().all(|descend| descend.root.is_some()));
        let workspace::Target::Pane { path } = upper.target() else {
            panic!("pane root")
        };
        assert!(placed.descends.iter().any(|descend| {
            descend.root.as_ref() == Some(&upper) && descend.path.as_ref() == path.as_slice()
        }));
    }

    #[test]
    fn hover_prefers_direct_claims_and_uses_extensions_only_to_retain() {
        let target = |index| Hovered::Tree(hover::Hover::Entry(index));
        let viewport = Rect::new(-100.0, -100.0, 100.0, 100.0);
        let mut placed: Placed<Editor, Paint> = Placed::empty();
        placed.probes.push(placed::Probe::retaining(
            Placement::new(Rect::new(0.0, 0.0, 10.0, 10.0), viewport),
            target(0),
        ));
        placed.probes.push(placed::Probe::retaining(
            Placement::new(Rect::new(14.0, 0.0, 24.0, 10.0), viewport),
            target(1),
        ));
        // A direct answer establishes hover.
        assert_eq!(
            derive_hover(&placed, None, Some(Point::new(5.0, 5.0)), false, 8.0,).0,
            Some(target(0))
        );
        // In the gap, only the prior target's extension may retain.
        assert_eq!(
            derive_hover(
                &placed,
                Some(target(0)),
                Some(Point::new(12.0, 5.0)),
                false,
                8.0,
            )
            .0,
            Some(target(0))
        );
        assert_eq!(
            derive_hover(
                &placed,
                Some(target(1)),
                Some(Point::new(12.0, 5.0)),
                false,
                8.0,
            )
            .0,
            Some(target(1))
        );
        // Even when the prior target's extension is encountered
        // first in z-order, a later direct answer overrides it.
        assert_eq!(
            derive_hover(
                &placed,
                Some(target(1)),
                Some(Point::new(8.0, 5.0)),
                false,
                8.0,
            )
            .0,
            Some(target(0))
        );
        // The neighboring real target overrides the prior target's
        // overlapping extension.
        assert_eq!(
            derive_hover(
                &placed,
                Some(target(0)),
                Some(Point::new(16.0, 5.0)),
                false,
                8.0,
            )
            .0,
            Some(target(1))
        );
        assert_eq!(
            derive_hover(&placed, None, Some(Point::new(12.0, 5.0)), false, 8.0,).0,
            None
        );
        assert_eq!(
            derive_hover(&placed, Some(target(0)), None, false, 8.0).0,
            None
        );
        // A pressed gesture keeps the hover it began with.
        assert_eq!(
            derive_hover(
                &placed,
                Some(target(0)),
                Some(Point::new(40.0, 40.0)),
                true,
                8.0,
            )
            .0,
            Some(target(0))
        );
        // An occluder answers "blocked" outright and blocks the
        // fallback — an overlay's pointer never lights what sits
        // beneath it or triggers the empty-space action.
        placed.probes.push(placed::Probe::occludes(Placement::new(
            Rect::new(0.0, 0.0, 40.0, 40.0),
            viewport,
        )));
        assert_eq!(
            derive_hover(&placed, None, Some(Point::new(25.0, 5.0)), false, 8.0,).0,
            Some(Hovered::Blocked)
        );
    }

    #[test]
    fn exact_hover_claims_do_not_retain_outside_their_hit_geometry() {
        let target = Hovered::Divider(workspace::Divider::Columns(workspace::Side::Left));
        let mut placed: Placed<Editor, Paint> = Placed::empty();
        placed.probes.push(placed::Probe::exact(
            Placement::new(
                Rect::new(0.0, 0.0, 10.0, 10.0),
                Rect::new(-100.0, -100.0, 100.0, 100.0),
            ),
            target.clone(),
        ));

        assert_eq!(
            derive_hover(&placed, None, Some(Point::new(5.0, 5.0)), false, 8.0,).0,
            Some(target.clone())
        );
        assert_eq!(
            derive_hover(
                &placed,
                Some(target.clone()),
                Some(Point::new(11.0, 5.0)),
                false,
                8.0,
            )
            .0,
            None
        );
        assert_eq!(
            derive_hover(
                &placed,
                Some(target.clone()),
                Some(Point::new(11.0, 5.0)),
                true,
                8.0,
            )
            .0,
            Some(target)
        );
    }
}
