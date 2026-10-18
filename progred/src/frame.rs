//! Project → compute hover → bind paint and dispatch.
//! Paint stays latent in the returned frame; rendering it is the
//! caller's choice, so a silent mint never draws.

use crate::hover;
use crate::input::PendingPaint;
use crate::menu;
use crate::model::Model;
use crate::navigate;
use crate::placed::{self, HoverPass};
use crate::projection;
use crate::sources;
use crate::stack;
use crate::workspace::{self, Root};
use crate::{Editor, EditorRunner, content_viewport};
use kurbo::{Affine, Insets, Point, Rect, Size, Vec2};
use parley::{FontContext, LayoutContext};
use peniko::Brush;
use puri::draw::Canvas;
use puri::geometry::Placement;
use puri::handler::{Event, Handler, HasHandler, ScrollOutcome};
use puri::hover::Claim;
use puri::interact::is_primary_contact;
use puri::text::TextCtx;
use std::rc::Rc;

pub(crate) const HOVER_REACH_POINTS: f64 = 8.0;

#[derive(Default)]
pub(crate) struct Dispatch {
    pub(crate) handler: Rc<Handler<Editor, placed::DispatchContext<Editor>>>,
    pub(crate) pointer_root: Option<crate::workspace::Root>,
    pub(crate) descends: Rc<[navigate::Descend<Editor>]>,
    pub(crate) view_regions: Rc<[placed::ViewRegion]>,
    pub(crate) hover_geometry: crate::display::widget::frame::HoverGeometry<Hovered>,
}

/// A completed frame. Installing it retains hover and dispatch together;
/// its paint continuations can be run separately or discarded.
pub(crate) struct Frame {
    pub(crate) scroll_probes: Vec<crate::display::widget::scroll::Probe>,
    pub(crate) hover: Option<Hovered>,
    pub(crate) dispatch: Dispatch,
    pub(crate) renders: Vec<placed::Render>,
    pub(crate) input_area: Option<Rect>,
}

#[derive(Default)]
pub(crate) struct FrameState {
    pub(crate) hover: Option<Hovered>,
    pub(crate) dispatch: Dispatch,
    pub(crate) pending_paint: Option<PendingPaint>,
    pub(crate) input_area: Option<Rect>,
    notified_hover: Option<(Option<Root>, Hovered)>,
    hover_awaits_paint: bool,
}

impl FrameState {
    fn hover_location(&self) -> Option<(Option<Root>, Hovered)> {
        self.hover
            .clone()
            .map(|hover| (self.dispatch.pointer_root.clone(), hover))
    }
}

impl Dispatch {
    pub(crate) fn geometry(&self, scale: f64) -> navigate::Geometry<'_, Editor> {
        navigate::Geometry {
            descends: &self.descends,
            view_regions: &self.view_regions,
            scale,
        }
    }

    /// The handler is shared only while a dispatch context is alive.
    pub(crate) fn handler_mut(&mut self) -> &mut Handler<Editor, placed::DispatchContext<Editor>> {
        Rc::get_mut(&mut self.handler).expect("no dispatch is in progress")
    }

    /// The context a dispatch to `editor` runs in, `hover` resting under its
    /// pointer.
    pub(crate) fn context(
        &self,
        editor: &Editor,
        hover: Option<Hovered>,
    ) -> placed::DispatchContext<Editor> {
        placed::DispatchContext {
            handler: self.handler.clone(),
            descends: self.descends.clone(),
            view_regions: self.view_regions.clone(),
            pointer: editor.pointer,
            modifiers: editor.modifiers,
            pressed: editor.pressed,
            ..placed::DispatchContext::new(self.pointer_root.clone(), hover)
        }
    }
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

impl From<hover::Hover> for Hovered {
    fn from(hover: hover::Hover) -> Self {
        Self::Tree(hover)
    }
}

/// The editor hosts the toolkit: its pointer rests on what the frame draws,
/// trees and chrome alike, and its dispatches act through its session.
impl crate::display::Host for Editor {
    type Hover = Hovered;

    fn tree(hover: &Hovered) -> Option<&hover::Hover> {
        match hover {
            Hovered::Tree(hover) => Some(hover),
            _ => None,
        }
    }

    fn session(&mut self) -> crate::editing::Session<'_> {
        Editor::session(self)
    }
}

/// What this frame's hover winner and selection refer to, shared by its
/// continuations. Resolve each once here, not in each painted occurrence.
fn attribute(
    sources: &sources::Sources<'_>,
    descends: &[navigate::Descend<Editor>],
    root: Option<&workspace::Root>,
    completion: Option<&crate::display::widget::offers::Offers<Editor>>,
    hovered: Option<Hovered>,
    link: bool,
) -> placed::Attribution {
    // Keep the probe's call chain for subsequent input, but use the selected
    // source for this frame's ordinary source/secondary decoration.
    let hovered = match hovered {
        Some(Hovered::Tree(ref calls @ hover::Hover::Calls(_))) => {
            crate::display::widget::source_link::hover_source(sources, descends, calls)
                .map(|source| Hovered::Tree(hover::Hover::Source(source)))
        }
        other => other,
    };
    let visible = source_hover_visible(hovered.as_ref(), link);
    let landmark = match &hovered {
        Some(Hovered::Tree(hover::Hover::Value(path))) => descends
            .iter()
            .find(|d| d.root.as_ref() == root && d.path == *path),
        _ => None,
    };
    let hovered_secondary = match &hovered {
        Some(Hovered::Tree(hover)) if visible => {
            hover::hover_secondary(landmark, completion, hover)
        }
        _ => None,
    };
    let hovered_trace = match &hovered {
        Some(Hovered::Tree(hover::Hover::Value(_))) if visible => landmark
            .and_then(|d| d.scope.location(&d.path))
            .map(|location| hover::SourceTrace::from_location(sources, &location)),
        Some(Hovered::Tree(hover::Hover::Source(source))) if visible => Some(source.clone()),
        _ => None,
    };
    placed::Attribution {
        hovered,
        hovered_secondary,
        selected_secondary: crate::display::widget::navigation::selected_secondary(descends),
        hovered_trace,
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) type Paint = puri_vello::compositor::SplitCanvas;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FrameDisposition {
    Retain,
    Remint,
}

pub(crate) fn frame_disposition(handled: bool, frame_input_changed: bool) -> FrameDisposition {
    if handled || frame_input_changed {
        FrameDisposition::Remint
    } else {
        FrameDisposition::Retain
    }
}

pub(crate) use crate::display::widget::scroll::offset as scroll_offset;

/// Interpret the winning claim as an editor target and its owning view.
fn hover_target(
    claim: Option<(Option<crate::workspace::Root>, Claim<Hovered>)>,
) -> (Option<Hovered>, Option<crate::workspace::Root>) {
    match claim {
        Some((root, Claim::Direct(target) | Claim::Extended(target))) => (Some(target), root),
        Some((root, Claim::Occludes)) => (Some(Hovered::Blocked), root),
        None => (None, None),
    }
}

fn source_hover_visible(hover: Option<&Hovered>, linking: bool) -> bool {
    !matches!(
        hover,
        Some(Hovered::Tree(
            hover::Hover::Source(_) | hover::Hover::Calls(_)
        ))
    ) || linking
}

pub(crate) struct FrameDescription<'a> {
    pub palette: crate::styles::Palette,
    pub command_modifier: puri::keyboard::CommandModifier,
    computations: &'a crate::computations::Computations,
    focused: bool,
    drawn_menu: bool,
    model: &'a Model,
    stack: &'a stack::Stack<Editor>,
    menu: menu::State,
    availability: crate::command::Availability,
    toggles: crate::command::Toggles,
    scale: f64,
    viewport: Size,
}

pub(crate) struct FrameResources<'a> {
    fonts: &'a mut FontContext,
    layouts: &'a mut LayoutContext<Brush>,
    text_cache: &'a mut puri::text::TextCache,
}

struct PointerInput<'a> {
    position: Option<Point>,
    previous: Option<(Option<&'a Root>, &'a Hovered)>,
    pressed: bool,
    link_sources: bool,
}

fn prepare_frame(
    description: FrameDescription<'_>,
    resources: FrameResources<'_>,
    pointer: PointerInput<'_>,
) -> Frame {
    description
        .computations
        .pointer_pressed
        .set(pointer.pressed);
    let (layout, keyboard) = project_frame(&description, resources);
    let mut frame = compute_hover(layout, &description, pointer);
    let placed = std::mem::take(frame.dispatch.handler_mut());
    // Structural paste precedes a pending's text query, while menu handling
    // still takes precedence over all document editing when installed. The
    // editor's structural keys answer only what every placed handler declines.
    *frame.dispatch.handler_mut() = crate::input::structural_keys(description.scale)
        .over(placed)
        .over(Handler::from_function(
            |editor: &mut Editor, event, _: &mut placed::DispatchContext<Editor>| {
                let handled = match &event {
                    Event::Key(key) => editor.pending_paste_key(key),
                    _ => false,
                };
                puri::handler::EventOutcome::from_handled(event, handled)
            },
        ))
        .over(keyboard);
    frame
}

fn compute_hover(
    layout: measured::Measured<HoverPass<Editor>>,
    description: &FrameDescription<'_>,
    pointer: PointerInput<'_>,
) -> Frame {
    let mut output = crate::display::widget::frame::place(
        layout,
        Placement::root(Rect::from_origin_size(Point::ZERO, description.viewport)),
        &placed::HoverInput {
            command_modifier: description.command_modifier,
            picking: pointer.link_sources,
            pointer: if pointer.pressed {
                None
            } else {
                pointer.position
            },
            prior: pointer.previous.map(|(_, hover)| hover),
            reach_px: HOVER_REACH_POINTS * description.scale,
        },
    );
    let (hover, pointer_root) = if pointer.pressed {
        pointer.previous.map_or((None, None), |(root, hover)| {
            (Some(hover.clone()), root.cloned())
        })
    } else {
        hover_target(output.claim.take())
    };
    let resolved = attribute(
        &sources::Sources {
            doc: &description.model.doc,
            libraries: &description.stack.libraries,
        },
        &output.descends,
        pointer_root.as_ref(),
        output.completion.as_ref(),
        hover.clone(),
        pointer.link_sources,
    );
    debug_assert!(
        output.landmark_select.is_none(),
        "selection handler escaped its landmark"
    );
    if description.model.selection.is_none()
        && let Some(handler) = navigate::initial_navigation(
            description.model.workspace.document_root(),
            &output.descends,
        )
    {
        *output.handler_mut() = std::mem::take(output.handler_mut()).over(handler);
    }
    let crate::display::widget::frame::FrameOutput {
        scroll_probes,
        renders,
        handler,
        descends,
        view_regions,
        hover_geometry,
        input_area,
    } = output.bind(resolved);
    Frame {
        scroll_probes,
        hover,
        dispatch: Dispatch {
            handler: Rc::new(handler.unwrap_or_else(Handler::new)),
            pointer_root,
            descends: descends.into(),
            view_regions: view_regions.into(),
            hover_geometry,
        },
        renders,
        input_area,
    }
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
        // A retained handler can outlive its view; decline rather than panic.
        match self.model.workspace.view_mut(&root) {
            Some(view) => {
                let (next, outcome) = scroll_offset(
                    view.scroll,
                    update,
                    scale,
                    viewport,
                    Vec2::new(max_scroll_x, max_scroll),
                );
                view.scroll = next;
                outcome
            }
            None => ScrollOutcome::pass(update),
        }
    }

    fn sync_views(&mut self) {
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
    }

    fn build_frame(
        &mut self,
        scale: f64,
        viewport: Size,
        previous: Option<(Option<Root>, Hovered)>,
    ) -> Frame {
        self.sync_views();
        self.computations.frame_time.set(web_time::Instant::now());
        prepare_frame(
            FrameDescription {
                palette: self.palette,
                command_modifier: self.command_modifier,
                computations: &self.computations,
                focused: self.focused,
                drawn_menu: self.drawn_menu,
                toggles: self.menu_toggles(),
                availability: self.menu_availability(),
                model: &self.model,
                stack: &self.stack,
                menu: self.menu,
                scale,
                viewport,
            },
            FrameResources {
                fonts: &mut self.font_cx,
                layouts: &mut self.layout_cx,
                text_cache: &mut self.text_cache,
            },
            PointerInput {
                position: self.pointer,
                previous: previous
                    .as_ref()
                    .map(|(root, hover)| (root.as_ref(), hover)),
                pressed: self.pressed,
                link_sources: self.command_modifier.pressed(&self.modifiers),
            },
        )
    }
}

impl EditorRunner {
    pub(crate) fn probe_pointer(&mut self, scale: f64) {
        // Retain the complete hover location throughout an active press.
        if self.editor.pressed {
            return;
        }
        let (hover, pointer_root) = hover_target(self.frame.dispatch.hover_geometry.probe(
            self.editor.pointer,
            self.frame.hover.as_ref(),
            HOVER_REACH_POINTS * scale,
        ));
        self.frame.hover = hover;
        self.frame.dispatch.pointer_root = pointer_root;
    }

    pub(crate) fn update_frame(
        &mut self,
        scale: f64,
        viewport: Size,
        handle_event: impl FnOnce(&mut Editor, &Dispatch, Option<&Hovered>) -> FrameDisposition,
    ) -> bool {
        match handle_event(
            &mut self.editor,
            &self.frame.dispatch,
            self.frame.hover.as_ref(),
        ) {
            FrameDisposition::Retain => false,
            FrameDisposition::Remint => {
                self.refresh_frame(scale, viewport);
                true
            }
        }
    }

    /// Build the successor's projection, hover, handlers, and deferred paint.
    pub(crate) fn refresh_frame(&mut self, scale: f64, viewport: Size) {
        self.rebuild_frame(scale, viewport);
        self.notify_hover_changed(scale, viewport);
    }

    fn rebuild_frame(&mut self, scale: f64, viewport: Size) {
        // Handlers' edits can't reach the window, so the title follows here.
        self.editor.refresh_title();
        let frame = self
            .editor
            .build_frame(scale, viewport, self.frame.hover_location());
        self.frame.pending_paint = Some(self.install_frame(frame, scale, viewport));
    }

    fn notify_hover_changed(&mut self, scale: f64, viewport: Size) {
        if self.dispatch_hover_changed() {
            self.rebuild_frame(scale, viewport);
        }
    }

    pub(crate) fn dispatch_hover_changed(&mut self) -> bool {
        if !self.frame.hover_awaits_paint
            && self.frame.notified_hover != self.frame.hover_location()
        {
            self.frame.notified_hover = self.frame.hover_location();
            let mut input = self
                .frame
                .dispatch
                .context(&self.editor, self.frame.hover.clone());
            let handled = self
                .frame
                .dispatch
                .handler
                .dispatch(&mut self.editor, Event::HoverChanged, &mut input)
                .handled();
            self.frame.hover_awaits_paint = handled;
            handled
        } else {
            false
        }
    }

    /// Only a submitted frame releases the next hover reaction. Unpainted
    /// successors may be replaced by input, just like other pending frames.
    pub(crate) fn frame_presented(&mut self) -> bool {
        self.frame.hover_awaits_paint = false;
        self.frame.notified_hover != self.frame.hover_location()
    }

    fn install_frame(&mut self, frame: Frame, scale: f64, viewport: Size) -> PendingPaint {
        let Frame {
            scroll_probes,
            hover,
            dispatch,
            renders,
            input_area,
        } = frame;
        #[cfg(target_arch = "wasm32")]
        crate::web_scroll::install(scroll_probes, scale);
        #[cfg(not(target_arch = "wasm32"))]
        drop(scroll_probes);
        #[cfg(target_arch = "wasm32")]
        crate::web::report_content_height(&dispatch.view_regions, scale);
        self.frame.hover = hover;
        self.frame.dispatch = dispatch;
        self.frame.input_area = input_area;
        PendingPaint {
            scale,
            viewport,
            renders,
        }
    }

    pub(crate) fn prepare_paint(&mut self, scale: f64, viewport: Size) -> PendingPaint {
        if self
            .frame
            .pending_paint
            .as_ref()
            .is_none_or(|pending| pending.scale != scale || pending.viewport != viewport)
        {
            self.rebuild_frame(scale, viewport);
        }
        self.notify_hover_changed(scale, viewport);
        self.frame
            .pending_paint
            .take()
            .expect("prepared frame has paint")
    }
}

#[allow(clippy::too_many_arguments)]
fn project_workspace_view(
    model: &Model,
    focused: bool,
    computations: &crate::computations::Computations,
    stack: &stack::Stack<Editor>,
    styles: &crate::styles::Styles,
    tcx: &mut TextCtx,
    sources: sources::Sources<'_>,
    view: &workspace::View,
    size: Size,
    scale: f64,
) -> measured::Measured<HoverPass<Editor>> {
    let raw = view.projection == workspace::Projection::Raw;
    let standard = view.projection == workspace::Projection::Standard;
    let viewport = match view.root.target() {
        workspace::Target::Pane { path } if standard => projection::viewport::entry(sources, path),
        _ => None,
    };
    let plain = (view.projection == workspace::Projection::Plain).then(|| stack.plain());
    let peeled = (!view.hidden.is_empty()).then(|| stack.without(&view.hidden));
    let (document_projection, pane_projection) = match &peeled {
        Some((document, pane)) => (document, pane),
        None => (&stack.projection, &stack.pane_projection),
    };
    let viewport = viewport.map(|entry| {
        (
            entry,
            projection::viewport::projection(document_projection, size / scale),
        )
    });
    let margin = if viewport.is_some() {
        0.0
    } else {
        12.0 * scale
    };
    let body_width = (size.width - 2.0 * margin).max(0.0);
    let root_path;
    let (root, projection) = match view.root.target() {
        workspace::Target::Document => {
            root_path = Vec::new();
            (sources.root(), document_projection)
        }
        workspace::Target::Pane { path } => {
            if let Some((entry, projection)) = &viewport {
                root_path = entry.path.clone();
                (Some(entry.value), projection)
            } else {
                root_path = path.clone();
                (sources.resolve_path(path), pane_projection)
            }
        }
    };
    let projected = projection::project(
        projection::ProjectDescription {
            computations,
            focused,
            view: &view.root,
            completions: Some(&stack.completions),
            sources,
            root,
            root_path: &root_path,
            selection: model
                .selection
                .as_ref()
                .filter(|selection| selection.root() == &view.root),
            source_selection: model.selection.as_ref(),
            annotations: &view.annotations,
            raw,
            styles,
            width: body_width,
            projection: match &plain {
                Some(plain) => Some(plain),
                None => (!raw).then_some(projection),
            },
        },
        tcx,
    );
    let content = measured::pad(Insets::uniform(margin), projected);
    let root = view.root.clone();
    let content = if viewport.is_some() {
        placed::viewport(content, root.clone())
    } else {
        let maximum = Vec2::new(
            ((content.extent.width - size.width) / scale).max(0.0),
            ((content.extent.height() - size.height) / scale).max(0.0),
        );
        let offset = Vec2::new(
            view.scroll.x.clamp(0.0, maximum.x) * scale,
            view.scroll.y.clamp(0.0, maximum.y) * scale,
        );
        let scroll_root = root.clone();
        placed::scrolled_at(
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
        )
    };
    let content = placed::in_view(content, root);
    let frame = placed::leaf(
        measured::Extent {
            width: size.width,
            ascent: 0.0,
            descent: size.height,
        },
        |_, _| {},
    );
    measured::overlay_into(frame, content, move |placement, _| {
        Some(Placement::new(
            placement.rect,
            placement.clip_rect.intersect(placement.rect),
        ))
    })
}

#[allow(clippy::too_many_arguments)]
fn project_workspace(
    model: &Model,
    focused: bool,
    computations: &crate::computations::Computations,
    stack: &stack::Stack<Editor>,
    styles: &crate::styles::Styles,
    tcx: &mut TextCtx,
    sources: sources::Sources<'_>,
    size: Size,
    scale: f64,
) -> measured::Measured<HoverPass<Editor>> {
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
        let view = placed_view.view;
        let rect = placed_view.rect;
        let child = project_workspace_view(
            model,
            focused,
            computations,
            stack,
            styles,
            tcx,
            sources,
            view,
            rect.size(),
            scale,
        );
        body = measured::overlay_into(body, child, move |placement, _| {
            let rect = rect + placement.rect.origin().to_vec2();
            Some(Placement::new(rect, placement.clip_rect.intersect(rect)))
        });
    }
    for placed_divider in geometry.dividers {
        let rect = placed_divider.rect;
        let divider = placed_divider.divider;
        let vertical = matches!(divider, workspace::Divider::Columns(_));
        let hover_divider = divider.clone();
        let cursor_divider = divider.clone();
        let down_divider = divider.clone();
        let move_divider = divider.clone();
        let up_divider = divider;
        let border = styles.palette.border;
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
                p.fill(placement.rect, border, Affine::IDENTITY);
                p.claim_exact(
                    Placement::new(hit, placement.clip_rect),
                    Hovered::Divider(hover_divider),
                );
                p.cursor(
                    Hovered::Divider(cursor_divider),
                    if vertical {
                        placed::Cursor::ColumnResize
                    } else {
                        placed::Cursor::RowResize
                    },
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
        body = measured::overlay_into(body, rule, move |placement, _| {
            let rect = rect + placement.rect.origin().to_vec2();
            // The rule is inside the workspace, but its retained drag
            // needs the workspace origin after the pointer leaves the
            // narrow hit target.
            Some(Placement::new(rect, placement.clip_rect))
        });
    }
    body
}

fn project_frame(
    description: &FrameDescription<'_>,
    resources: FrameResources<'_>,
) -> (
    measured::Measured<HoverPass<Editor>>,
    Handler<Editor, placed::DispatchContext<Editor>>,
) {
    let FrameDescription {
        palette,
        command_modifier,
        computations,
        focused,
        drawn_menu,
        ref toggles,
        model,
        stack,
        menu,
        availability,
        scale,
        viewport,
    } = *description;
    computations.begin(model.doc.clone(), stack.libraries.clone());
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
    let styles = crate::styles::editor(palette, scale);
    let application_menu = drawn_menu.then(|| {
        menu::view(
            &mut tcx,
            menu::Description {
                palette,
                command_modifier,
                state: menu,
                availability,
                toggles: toggles.clone(),
                projections: stack
                    .projections()
                    .map(|(library, name)| (library, name.to_owned()))
                    .collect(),
                scale,
                width: viewport_width,
            },
        )
    });
    let history = crate::command::history_handler(scale);
    let (menu_bar, menu_popup, menu_heading_width, keyboard) = match application_menu {
        Some(menu) => (
            Some(menu.bar),
            menu.popup,
            menu.heading_width,
            history.over(menu.keyboard),
        ),
        None => (None, None, 0.0, history),
    };
    let content_viewport = content_viewport(drawn_menu, viewport, scale);
    let sources = sources::Sources {
        doc: &model.doc,
        libraries: &stack.libraries,
    };
    let body = project_workspace(
        model,
        focused,
        computations,
        stack,
        &styles,
        &mut tcx,
        sources,
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
        stage = measured::overlay_into(stage, bar, move |_, _| Some(bar_placement));
    }
    stage = measured::overlay_into(stage, body, move |_, _| {
        Some(Placement::new(content_viewport, content_viewport))
    });

    if let Some((x, popup)) = menu_popup {
        let heading_width = menu_heading_width;
        // Outside presses close the popup. Its own Occludes claim
        // becomes Hovered::Blocked over separators; command items resolve
        // their semantic target above it, so no raw inside-swallow may
        // preempt them.
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
            let x = x
                .min(placement.clip_rect.x1 - extent.width)
                .max(placement.clip_rect.x0);
            Some(Placement::new(
                extent.rect_at(Point::new(x, content_viewport.y0)),
                placement.clip_rect,
            ))
        });
    }
    (stage, keyboard)
}

#[cfg(test)]
mod frame_tests;
