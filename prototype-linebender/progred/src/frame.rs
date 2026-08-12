//! One read-only UI pass: place the document, resolve hover, emit dispatch.

use crate::completion;
use crate::display;
use crate::graph_view;
use crate::hover;
use crate::layout;
use crate::menu;
use crate::model::{Model, Selected, ViewFlags};
use crate::navigate;
use crate::projection;
use crate::raw;
use crate::selection;
use crate::sources;
use crate::{App, content_viewport, graph_panel};
use winit::dpi::PhysicalPosition;
use parley::{FontContext, LayoutContext};
use progred_graph::Value;
use puri::draw::{Canvas, GlyphRun, Shape};
use puri::edit::{EditCtx, LineEditPointerDown};
use puri::geometry::Placement;
use puri::handler::{Handler, HasHandler};
use puri::text::TextCtx;
use puri_vello::VelloCanvas;
use std::rc::Rc;
use ui_events::pointer::PointerButton;
use vello::kurbo::{Affine, Point, Size, Stroke, Vec2};
use vello::peniko::Brush;
use vello::Scene;

pub(crate) struct Dispatch {
    pub(crate) handler: Handler<App>,
    pub(crate) descends: Vec<navigate::Descend>,
    /// One nominal line height at the frame's scale — the quantum
    /// keyboard navigation reads rows with.
    pub(crate) line: f64,
    pub(crate) max_scroll: f64,
    pub(crate) max_scroll_x: f64,
    pub(crate) popup: Option<completion::Popup>,
}

/// The app's one hover, the selection's shape: what the resting
/// pointer claims in whichever pane it rests over.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Hovered {
    Tree(hover::Hovering),
    Graph(graph_view::GraphNode),
    #[cfg(target_os = "linux")]
    Menu(menu::Hover),
}

pub(crate) enum HoverHit {
    Tree(hover::HoverClaim),
    Graph(Option<graph_view::GraphNode>),
    #[cfg(target_os = "linux")]
    Menu(Option<menu::Hover>),
}

pub(crate) struct HoverResolver<'a> {
    current: &'a mut Option<Hovered>,
    pointer: Option<Point>,
    pressed: bool,
    reach: f64,
    hit: Option<HoverHit>,
}

impl HoverResolver<'_> {
    fn resolve(self) {
        *self.current = resolved_hover(
            self.current.as_ref(),
            self.hit,
            self.pointer,
            self.pressed,
            self.reach,
        );
    }
}

#[derive(Clone, Copy)]
pub(crate) enum FrameVisibility {
    Silent,
    Visible,
}

pub(crate) struct FrameDescription<'a> {
    model: &'a Model,
    view: ViewFlags,
    menu: menu::State,
    availability: menu::Availability,
    hover: Option<Hovered>,
    scale: f64,
    viewport: Size,
}

pub(crate) struct FrameResources<'a> {
    fonts: &'a mut FontContext,
    layouts: &'a mut LayoutContext<Brush>,
    text_cache: &'a mut puri::text::TextCache,
}

/// One read-only pass over the UI. Drawing is optional; every pass
/// still produces transient dispatch data and resolves pointer hover.
pub(crate) struct Frame<'a> {
    scene: Option<&'a mut Scene>,
    hover: HoverResolver<'a>,
    handler: Handler<App>,
    descends: Vec<navigate::Descend>,
    /// How far the document can scroll given this frame's content and
    /// viewport; dispatch clamps against it.
    max_scroll: f64,
    max_scroll_x: f64,
    /// The pending row's completion popup, emitted during placement;
    /// drawn after the body and committed from at dispatch.
    popup: Option<completion::Popup>,
}

impl<'a> Frame<'a> {
    pub(crate) fn new(scene: Option<&'a mut Scene>, hover: HoverResolver<'a>) -> Self {
        Self {
            scene,
            hover,
            handler: Handler::new(),
            descends: Vec::new(),
            max_scroll: 0.0,
            max_scroll_x: 0.0,
            popup: None,
        }
    }

    pub(crate) fn finish(self, scale: f64) -> Dispatch {
        self.hover.resolve();
        Dispatch {
            handler: self.handler,
            descends: self.descends,
            line: 14.0 * scale,
            max_scroll: self.max_scroll,
            max_scroll_x: self.max_scroll_x,
            popup: self.popup,
        }
    }
}

impl hover::HasHover<hover::HoverClaim> for Frame<'_> {
    fn pointer(&self) -> Option<Point> {
        self.hover.pointer
    }

    fn claim_hover(&mut self, claim: hover::HoverClaim) {
        self.hover.hit = Some(HoverHit::Tree(claim));
    }
}

impl hover::HasHover<Option<graph_view::GraphNode>> for Frame<'_> {
    fn pointer(&self) -> Option<Point> {
        self.hover.pointer
    }

    fn claim_hover(&mut self, claim: Option<graph_view::GraphNode>) {
        self.hover.hit = Some(HoverHit::Graph(claim));
    }
}

#[cfg(target_os = "linux")]
impl hover::HasHover<Option<menu::Hover>> for Frame<'_> {
    fn pointer(&self) -> Option<Point> {
        self.hover.pointer
    }

    fn claim_hover(&mut self, claim: Option<menu::Hover>) {
        self.hover.hit = Some(HoverHit::Menu(claim));
    }
}

impl completion::HasPopup for Frame<'_> {
    fn popup(&mut self) -> &mut Option<completion::Popup> {
        &mut self.popup
    }
}

impl HasHandler<App> for Frame<'_> {
    fn handler(&mut self) -> &mut Handler<App> {
        &mut self.handler
    }
}

impl navigate::HasDescends for Frame<'_> {
    fn descends(&mut self) -> &mut Vec<navigate::Descend> {
        &mut self.descends
    }
}

impl Canvas for Frame<'_> {
    fn fill(&mut self, shape: impl Into<Shape>, brush: impl Into<Brush>, transform: Affine) {
        if let Some(scene) = self.scene.as_deref_mut() {
            VelloCanvas(scene).fill(shape, brush, transform);
        }
    }

    fn stroke(
        &mut self,
        shape: impl Into<Shape>,
        style: Stroke,
        brush: impl Into<Brush>,
        transform: Affine,
    ) {
        if let Some(scene) = self.scene.as_deref_mut() {
            VelloCanvas(scene).stroke(shape, style, brush, transform);
        }
    }

    fn glyph_run(&mut self, run: GlyphRun) {
        if let Some(scene) = self.scene.as_deref_mut() {
            VelloCanvas(scene).glyph_run(run);
        }
    }

    fn clip(
        &mut self,
        shape: impl Into<Shape>,
        transform: Affine,
        content: impl FnOnce(&mut Self),
    ) {
        let shape = shape.into();
        if let Some(scene) = self.scene.as_deref_mut() {
            VelloCanvas(scene).push_clip(&shape, transform);
        }
        content(self);
        if let Some(scene) = self.scene.as_deref_mut() {
            VelloCanvas(scene).pop_clip();
        }
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

pub(crate) fn resolved_hover(
    current: Option<&Hovered>,
    hit: Option<HoverHit>,
    pointer: Option<Point>,
    pressed: bool,
    reach: f64,
) -> Option<Hovered> {
    match (pressed, pointer) {
        (true, _) => current.cloned(),
        (false, None) => None,
        (false, Some(point)) => match hit {
            Some(HoverHit::Tree(hover::HoverClaim::Direct(hovering))) => hovering.map(Hovered::Tree),
            Some(HoverHit::Graph(node)) => node.map(Hovered::Graph),
            #[cfg(target_os = "linux")]
            Some(HoverHit::Menu(hover)) => hover.map(Hovered::Menu),
            Some(HoverHit::Tree(hover::HoverClaim::Air)) | None => {
                let tree = match current {
                    Some(Hovered::Tree(hovering)) => Some(hovering),
                    _ => None,
                };
                match hover::resolve_hover(hover::HoverClaim::Air, tree, point, reach) {
                    Some(next) => next.map(Hovered::Tree),
                    None => current.cloned(),
                }
            }
        },
    }
}

impl App {
    pub(crate) fn scroll_document(
        &mut self,
        update: &ui_events::pointer::PointerScrollEvent,
        scale: f64,
        viewport: f64,
        max_scroll: f64,
        max_scroll_x: f64,
    ) -> bool {
        let line = 40.0 * scale;
        let delta = update.delta.to_pixel_delta(
            PhysicalPosition { x: line, y: line },
            PhysicalPosition {
                x: viewport,
                y: viewport,
            },
        );
        // ScrollDelta documents positive as viewport-down/right, but
        // ui-events-winit passes winit deltas through raw, where
        // positive is scroll-up/left; subtract to match reality.
        // Stepping from the clamped position keeps the first tick
        // responsive when a resize left the stored offset out of
        // bounds.
        let next =
            (self.model.scroll.clamp(0.0, max_scroll) - delta.y / scale).clamp(0.0, max_scroll);
        let next_x = (self.model.scroll_x.clamp(0.0, max_scroll_x) - delta.x / scale)
            .clamp(0.0, max_scroll_x);
        (next != self.model.scroll || next_x != self.model.scroll_x) && {
            self.model.scroll = next;
            self.model.scroll_x = next_x;
            true
        }
    }

    /// Scroll-to-reveal, computed from the freshly retained dispatch
    /// pass BEFORE anything draws, so the reveal lands in the next
    /// presented frame with no corrective flash. Fires once per
    /// selection-identity change (path AND variant — Enter keeps the
    /// path while opening a pending), so it never fights manual
    /// scrolling. The target is the popup anchor while pending — it
    /// marks the authoring row — else the selection's rect.
    pub(crate) fn reveal_selection(&mut self, dispatch: &Dispatch, scale: f64, viewport: Size) -> bool {
        let reveal = self
            .model
            .tree_selection()
            .map(|s| (s.path().to_vec(), std::mem::discriminant(s)));
        if reveal == self.revealed {
            false
        } else {
            self.revealed = reveal.clone();
            let target = dispatch
                .popup
                .as_ref()
                .map(|popup| popup.anchor)
                .or_else(|| {
                    reveal.as_ref().and_then(|(path, _)| {
                        dispatch
                            .descends
                            .iter()
                            .find(|descend| &descend.path == path)
                            .map(|descend| descend.rect)
                    })
                });
            target.is_some_and(|rect| {
                let before = (self.model.scroll, self.model.scroll_x);
                let pad = 12.0 * scale;
                let content = content_viewport(viewport, scale);
                let mut scroll = self.model.scroll;
                // The pad is the landing margin, not the trigger: fully
                // visible rects are left alone, so a click near an edge
                // doesn't nudge.
                if rect.y1 > content.y1 {
                    scroll += (rect.y1 + pad - content.y1) / scale;
                }
                // Checked against the adjusted position, so when the rect
                // is taller than the viewport the top wins.
                let top = rect.y0 - (scroll - self.model.scroll) * scale;
                if top < content.y0 {
                    scroll += (top - pad - content.y0) / scale;
                }
                self.model.scroll = scroll.clamp(0.0, dispatch.max_scroll);
                // The same chase horizontally, against the width the
                // graph panel leaves visible.
                let visible = if self.view_flags().graph {
                    graph_panel(viewport, scale).x0
                } else {
                    viewport.width
                };
                let mut scroll_x = self.model.scroll_x;
                if rect.x1 > visible {
                    scroll_x += (rect.x1 + pad - visible) / scale;
                }
                let left = rect.x0 - (scroll_x - self.model.scroll_x) * scale;
                if left < 0.0 {
                    scroll_x += (left - pad) / scale;
                }
                self.model.scroll_x = scroll_x.clamp(0.0, dispatch.max_scroll_x);
                (self.model.scroll, self.model.scroll_x) != before
            })
        }
    }

    pub(crate) fn view_flags(&self) -> ViewFlags {
        self.model.view
    }

    pub(crate) fn build_frame(&mut self, visibility: FrameVisibility, scale: f64, viewport: Size) -> Dispatch {
        let view = self.view_flags();
        let availability = self.menu_availability();
        let presented_hover = self.hover.clone();
        let scene = match visibility {
            FrameVisibility::Silent => None,
            FrameVisibility::Visible => Some(&mut self.scene),
        };
        let description = FrameDescription {
            model: &self.model,
            view,
            menu: self.menu,
            availability,
            hover: presented_hover,
            scale,
            viewport,
        };
        let resources = FrameResources {
            fonts: &mut self.font_cx,
            layouts: &mut self.layout_cx,
            text_cache: &mut self.text_cache,
        };
        let hover = HoverResolver {
            current: &mut self.hover,
            pointer: self.pointer,
            pressed: self.pressed,
            reach: 8.0 * scale,
            hit: None,
        };
        let mut frame = Frame::new(scene, hover);
        run_frame(&mut frame, description, resources);
        frame.finish(scale)
    }

    /// Mint dispatch data from the final state of a transition. A
    /// silent pass supplies reveal geometry and resolves hover;
    /// scrolling to reveal changes geometry and earns one rebuild.
    pub(crate) fn retain_dispatch(&mut self, scale: f64, viewport: Size, reveal_selection: bool) -> bool {
        let before = self.hover.clone();
        let mut dispatch = self.build_frame(FrameVisibility::Silent, scale, viewport);
        if reveal_selection && self.reveal_selection(&dispatch, scale, viewport) {
            dispatch = self.build_frame(FrameVisibility::Silent, scale, viewport);
        }
        let hover_changed = self.hover != before;
        self.dispatch = Some(dispatch);
        self.hover_is_current = true;
        hover_changed
    }
}

pub(crate) fn run_frame(
    frame: &mut Frame<'_>,
    description: FrameDescription<'_>,
    resources: FrameResources<'_>,
) {
    let FrameDescription {
        model,
        view,
        menu,
        availability,
        hover,
        scale,
        viewport,
    } = description;
    let FrameResources {
        fonts: font_cx,
        layouts: layout_cx,
        text_cache,
    } = resources;
    let (viewport_width, viewport_height) = (viewport.width, viewport.height);
    // Empty space deselects — the one slot, whichever pane filled it.
    // Registered before the content places, so the descend handlers
    // (registered as they place) take precedence, and only a press
    // that claims no edge falls through to here.
    frame.handler().on_pointer_down(|app: &mut App, event| {
        event.button == Some(PointerButton::Primary) && app.model.selection.take().is_some()
    });
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
    let styles = display::Styles::new(scale);
    #[cfg(target_os = "linux")]
    let menu_hover = match hover.as_ref() {
        Some(Hovered::Menu(hover)) => Some(*hover),
        _ => None,
    };
    #[cfg(target_os = "linux")]
    let application_menu = menu::view(
        &mut tcx,
        menu::Description {
            state: menu,
            availability,
            raw: view.raw,
            graph: view.graph,
            hover: menu_hover,
            scale,
            width: viewport_width,
        },
        menu::Hooks {
            toggle: Rc::new(|app: &mut App, section| app.menu.toggle(section)),
            select: Rc::new(|app: &mut App, selection| app.choose_menu(selection)),
        },
    );
    #[cfg(not(target_os = "linux"))]
    let _ = (menu, availability);
    let content_viewport = content_viewport(viewport, scale);
    #[cfg(target_os = "linux")]
    layout::place(
        application_menu.bar,
        frame,
        Placement::new(
            Rect::new(0.0, 0.0, viewport_width, content_viewport.y0),
            Rect::new(0.0, 0.0, viewport_width, viewport_height),
        ),
    );
    let (tree_hover, graph_hover) = match hover.as_ref() {
        Some(Hovered::Tree(hovering)) => (Some(&hovering.hover), None),
        Some(Hovered::Graph(node)) => (None, Some(node)),
        #[cfg(target_os = "linux")]
        Some(Hovered::Menu(_)) => (None, None),
        None => (None, None),
    };
    // The Raw view is ONE bit, threaded as itself: name lookups
    // derive from it downstream, no policy swapped here, and the
    // model's configured policy rides along untouched.
    let sources = model.sources();
    let graph_node = model.graph_node();
    let margin = 12.0 * scale;
    // The width layout answers to: the window, less the graph panel
    // when it is up — the panel overlays the right side, and content
    // should break rather than run beneath it.
    let body_width = if view.graph {
        graph_panel(viewport, scale).x0 - 2.0 * margin
    } else {
        viewport_width - 2.0 * margin
    };
    let hover_node = graph_hover
        .and_then(|node| graph_view::node_value(&model.doc, node))
        .filter(|value| !matches!(value, Value::Record(_)));
    let body = raw::project(
        raw::ProjectDescription {
            sources,
            selection: model.tree_selection(),
            graph_node: graph_node.as_ref(),
            hover: tree_hover,
            hover_node: hover_node.as_ref(),
            collapse: &model.collapse,
            names: &model.names,
            raw: view.raw,
            styles: &styles,
            width: body_width,
            projection: projection::Projection::new(&model.foreign),
        },
        &mut tcx,
        raw::Hooks {
            // The selection transition: re-selecting the same path
            // keeps its editor state, and a reported text click seeds
            // or advances the editor's caret — focus and cursor
            // placement are one event.
            select: Rc::new(move |app: &mut App, path, click| {
                // A label pending has no path of its own — path()
                // names its PARENT — so a reported click is always a
                // real selection change (the pending row swallows its
                // own clicks before they can reach here).
                let fresh = match app.model.tree_selection() {
                    Some(selection::Selection::PendingEdge { .. }) | None => true,
                    Some(current) => current.path() != path,
                };
                if fresh {
                    app.model.selection = Some(Selected::Tree(selection::Selection::edge(
                        &app.model.sources(),
                        path,
                    )));
                } else if click.is_none()
                    && let Some(line) = app
                        .model
                        .tree_selection_mut()
                        .and_then(selection::Selection::edit_mut)
                {
                    // Re-selecting without a text click lands the
                    // caret at the end, same as a fresh mount.
                    line.cursor_to_end();
                }
                if let Some(click) = click
                    && let Some(line) = app
                        .model
                        .tree_selection_mut()
                        .and_then(selection::Selection::edit_mut)
                {
                    // A tap sequence never spans targets: the click
                    // that mounts an editor is its first, whatever
                    // the physical count says — selecting the cell
                    // was stage one, not half a double-click, and a
                    // quick click on a neighboring atom is not a
                    // double-click in this one.
                    let count = if fresh { 1 } else { click.count };
                    line.pointer_down(
                        &click.presentation,
                        &mut app.font_cx,
                        &mut app.layout_cx,
                        scale as f32,
                        LineEditPointerDown {
                            point: click.point,
                            shift: click.shift,
                            count,
                        },
                    );
                }
            }),
            toggle: Rc::new(|app: &mut App, path| {
                selection::toggle_collapse(
                    &sources::Sources {
                        doc: &app.model.doc,
                        library: &app.model.library,
                    },
                    &mut app.model.collapse,
                    &path,
                );
            }),
            rename: Rc::new(|app: &mut App, path, index| {
                if let Some(mut pending) = selection::pending_rename(&app.model.sources(), &path) {
                    // The index was hit-tested against the label that
                    // was clicked, in the label's own face; the seed
                    // shares its spelling, so the caret lands under
                    // the pointer in whatever face the editor draws.
                    if let Some(line) = pending.edit_mut() {
                        line.cursor_to(index);
                    }
                    app.model.selection = Some(Selected::Tree(pending));
                }
            }),
            edit: Rc::new(edit_ctx),
            pick: Rc::new(|app: &mut App, id| app.pick_identity(id)),
            insert: Rc::new(|app: &mut App, path| {
                if let Some(pending) = selection::pending_after(&app.model.sources(), &path) {
                    app.model.selection = Some(Selected::Tree(pending));
                }
            }),
        },
    );
    // The body rides Progred's scroll container: margins pad into the
    // content, the window is the viewport, and the app's clamped
    // offsets (ordinary model state) shift it. The horizontal
    // maximum answers to the LAYOUT width — content should only
    // scroll where even the block forms overflowed it — not the
    // window edge the viewport clips at.
    let content = layout::pad(vello::kurbo::Insets::uniform(margin), body);
    frame.max_scroll = ((content.extent.height() - content_viewport.height()) / scale).max(0.0);
    frame.max_scroll_x = ((content.extent.width - (body_width + 2.0 * margin)) / scale).max(0.0);
    let offset = Vec2::new(
        model.scroll_x.clamp(0.0, frame.max_scroll_x) * scale,
        model.scroll.clamp(0.0, frame.max_scroll) * scale,
    );
    let max_scroll = frame.max_scroll;
    let max_scroll_x = frame.max_scroll_x;
    let graph_panel_rect = view.graph.then(|| graph_panel(viewport, scale));
    layout::place_scrolled(
        content,
        frame,
        Placement::new(content_viewport, content_viewport),
        offset,
        move |app, update| {
            let point = Point::new(update.state.position.x, update.state.position.y);
            !graph_panel_rect.is_some_and(|panel| panel.contains(point))
                && app.scroll_document(
                    update,
                    scale,
                    content_viewport.height(),
                    max_scroll,
                    max_scroll_x,
                )
        },
    );
    // The graph pane draws over the document's right side; placed
    // after the body so its handlers win inside the panel.
    if view.graph {
        let panel = graph_panel(viewport, scale);
        let pane = graph_view::pane(
            &sources,
            &model.graph,
            model.graph_selection(),
            model.tree_selection(),
            graph_hover,
            tree_hover,
            &model.names,
            view.raw,
            &mut tcx,
            panel,
            &graph_view::Hooks {
                press_node: Rc::new(|app: &mut App, id, grab, world| {
                    // Grabbing a node drops a tree selection (its
                    // editor must not stay focused behind the drag);
                    // a graph selection stands until the release
                    // decides click or drag.
                    if matches!(app.model.selection, Some(Selected::Tree(_))) {
                        app.model.selection = None;
                    }
                    app.model.graph.press_node(id, grab, world);
                }),
                press_background: Rc::new(|app: &mut App, panel| {
                    app.model.graph.press_background(panel);
                }),
                drag_to: Rc::new(|app: &mut App, world, panel, px| {
                    app.model.graph.drag_to(world, panel, px)
                }),
                release: Rc::new(|app: &mut App| match app.model.graph.release() {
                    Some(graph_view::Release::ClickNode(id)) => {
                        app.model.selection =
                            Some(Selected::Graph(graph_view::GraphSelection::Node(id)));
                        true
                    }
                    Some(graph_view::Release::ClickBackground) => {
                        app.model.selection = None;
                        true
                    }
                    Some(graph_view::Release::Drag) => true,
                    None => false,
                }),
                scroll: Rc::new(|app: &mut App, delta, cursor, scale| {
                    app.model.graph.scroll(delta, cursor, scale);
                }),
                pick: Rc::new(|app: &mut App, id| app.pick_identity(id)),
            },
        );
        let rect = pane.extent.rect_at(Point::new(panel.x0, panel.y0));
        layout::place(pane, frame, Placement::new(rect, content_viewport));
    }

    // The pending row's popup draws after the body, so it overlays
    // and its click targets win.
    if let Some(popup) = frame.popup.take() {
        let hovered_entry = match tree_hover {
            Some(hover::Hover::Entry(index)) => Some(*index),
            _ => None,
        };
        let commit = |app: &mut App, action: &completion::EntryAction| match app.model.selection.take() {
            Some(Selected::Tree(selection::Selection::Pending { path, .. })) => {
                app.commit_value(path, action);
            }
            Some(Selected::Tree(selection::Selection::PendingEdge {
                parent, replacing, ..
            })) => {
                app.commit_label(parent, replacing, action);
            }
            selection => app.model.selection = selection,
        };
        let card = raw::popup_view(&mut tcx, &styles, &popup, hovered_entry, commit);
        // Below the anchor, unless it would run off the bottom and
        // fits above — then flip on top, as the TypeScript prototype
        // did. The card's extent is known before placement.
        let below = popup.anchor.y1 + 4.0 * scale;
        let above = popup.anchor.y0 - 4.0 * scale - card.extent.height();
        let y =
            if below + card.extent.height() > content_viewport.y1 && above >= content_viewport.y0 {
                above
            } else {
                below
            };
        let rect = card.extent.rect_at(Point::new(popup.anchor.x0, y));
        layout::place(card, frame, Placement::new(rect, content_viewport));
        frame.popup = Some(popup);
    }

    #[cfg(target_os = "linux")]
    if let Some((x, popup)) = application_menu.popup {
        let rect = popup.extent.rect_at(Point::new(x, content_viewport.y0));
        let headings = Rect::new(
            0.0,
            0.0,
            application_menu.heading_width,
            content_viewport.y0,
        );
        frame.handler().on_pointer_down(move |app, event| {
            let point = Point::new(event.state.position.x, event.state.position.y);
            event.button == Some(PointerButton::Primary)
                && !headings.contains(point)
                && !rect.contains(point)
                && app.menu.close()
        });
        frame.handler().on_pointer_down(move |_, event| {
            event.button == Some(PointerButton::Primary)
                && rect.contains(Point::new(event.state.position.x, event.state.position.y))
        });
        frame.handler().on_scroll(move |_, event| {
            rect.contains(Point::new(event.state.position.x, event.state.position.y))
        });
        layout::place(
            popup,
            frame,
            Placement::new(rect, Rect::new(0.0, 0.0, viewport_width, viewport_height)),
        );
    }
}

/// Dispatch-time access to the selection's editor. Retained-frame
/// dispatch can outlive the editor by a frame — deselect, then a move
/// in the same gesture — so absence declines rather than panics.
pub(crate) fn edit_ctx(app: &mut App) -> Option<EditCtx<'_>> {
    let App {
        model,
        font_cx,
        layout_cx,
        text_clipboard,
        ..
    } = app;
    let state = model
        .tree_selection_mut()
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
    fn hover_resolution_keeps_only_real_hysteresis_state() {
        let hovering = hover::Hovering {
            hover: hover::Hover::Value(Vec::new()),
            rect: vello::kurbo::Rect::new(10.0, 10.0, 20.0, 20.0),
        };
        let current = Hovered::Tree(hovering.clone());
        assert_eq!(
            resolved_hover(
                Some(&current),
                None,
                Some(Point::new(24.0, 15.0)),
                false,
                8.0,
            ),
            Some(current.clone())
        );
        assert_eq!(
            resolved_hover(
                Some(&current),
                Some(HoverHit::Graph(Some(graph_view::GraphNode::Root))),
                Some(Point::ZERO),
                false,
                8.0,
            ),
            Some(Hovered::Graph(graph_view::GraphNode::Root))
        );
        assert_eq!(
            resolved_hover(
                Some(&current),
                Some(HoverHit::Tree(hover::HoverClaim::Direct(None))),
                Some(Point::ZERO),
                true,
                8.0,
            ),
            Some(current)
        );
        assert_eq!(resolved_hover(None, None, None, false, 8.0), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn menu_hover_uses_the_ordinary_settled_resolver() {
        let hover = menu::Hover::Item(menu::Selection::Save);
        assert_eq!(
            resolved_hover(
                None,
                Some(HoverHit::Menu(Some(hover))),
                Some(Point::new(20.0, 40.0)),
                false,
                8.0,
            ),
            Some(Hovered::Menu(hover))
        );
        assert_eq!(
            resolved_hover(
                Some(&Hovered::Menu(hover)),
                Some(HoverHit::Menu(None)),
                Some(Point::new(20.0, 40.0)),
                false,
                8.0,
            ),
            None
        );
    }
}

