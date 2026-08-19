//! One staged pass: place the document, probe hover, mint dispatch.
//! Ink stays latent in the returned frame; rendering it is the
//! caller's choice, so a silent mint never draws.

use crate::completion;
use crate::graph_view;
use crate::hover;
use crate::menu;
use crate::model::{Model, Selected, ViewFlags};
use crate::navigate;
use crate::placed::{self, Placed};
use crate::projection;
use crate::selection;
use crate::sources;
use crate::stack;
use crate::{App, content_viewport, graph_panel};
use gid::Value;
use parley::{FontContext, LayoutContext};
use puri::draw::{Canvas, GlyphRun, Shape};
use puri::edit::{EditCtx, LineEditPointerDown};
use puri::geometry::Placement;
use measured::Output;
use puri::handler::Handler;
use puri::hover::Claim;
use puri::text::TextCtx;
use puri_vello::VelloCanvas;
use std::rc::Rc;
use ui_events::pointer::PointerButton;
use vello::Scene;
use vello::kurbo::{Affine, Point, Size, Stroke, Vec2};
use vello::peniko::Brush;
use winit::dpi::PhysicalPosition;

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

/// One minted frame: the dispatch the shell retains, and the ink the
/// pass deferred — run it into a [`Paint`] or drop it silently.
pub(crate) struct Frame {
    pub(crate) dispatch: Dispatch,
    pub(crate) renders: Vec<placed::Render<Paint>>,
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

/// The concrete canvas frame ink renders into: the vello scene,
/// owned so deferred ink closures need no lifetime.
pub(crate) struct Paint {
    pub(crate) scene: Scene,
}

impl Canvas for Paint {
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

/// The frame's hover, from this pass's settled geometry: pressed
/// keeps the hover a gesture began with, a claim answers outright,
/// and air within the little gap's reach of a tree footprint HOLDS —
/// crossing a separator or the leading between rows never flickers.
pub(crate) fn resolved_hover(
    current: Option<&Hovered>,
    hit: Option<Claim<Hovered>>,
    pointer: Option<Point>,
    pressed: bool,
    reach: f64,
) -> Option<Hovered> {
    match (pressed, pointer) {
        (true, _) => current.cloned(),
        (false, None) => None,
        (false, Some(point)) => match hit {
            Some(Claim::Names(next)) => Some(next),
            Some(Claim::Occludes) => None,
            None => match current {
                Some(Hovered::Tree(hovering))
                    if hovering.rect.inflate(reach, reach).contains(point) =>
                {
                    current.cloned()
                }
                _ => None,
            },
        },
    }
}

pub(crate) struct FrameDescription<'a> {
    model: &'a Model,
    stack: &'a stack::Stack<App>,
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

struct Built {
    placed: Placed<App, Paint>,
    max_scroll: f64,
    max_scroll_x: f64,
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
    pub(crate) fn reveal_selection(
        &mut self,
        dispatch: &Dispatch,
        scale: f64,
        viewport: Size,
    ) -> bool {
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

    /// One staged pass over the UI: place, probe the pointer against
    /// the settled geometry, resolve hover, mint dispatch. Ink comes
    /// back deferred; the caller renders it or drops it.
    pub(crate) fn build_frame(&mut self, scale: f64, viewport: Size) -> Frame {
        let view = self.view_flags();
        let availability = self.menu_availability();
        let presented_hover = self.hover.clone();
        let description = FrameDescription {
            model: &self.model,
            stack: &self.stack,
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
        let built = run_frame(description, resources);
        let hit = self.pointer.and_then(|point| built.placed.probe(point));
        self.hover = resolved_hover(
            self.hover.as_ref(),
            hit,
            self.pointer,
            self.pressed,
            8.0 * scale,
        );
        let Placed {
            probes: _,
            handler,
            descends,
            popup,
            renders,
        } = built.placed;
        Frame {
            dispatch: Dispatch {
                handler: handler.unwrap_or_else(Handler::new),
                descends,
                line: 14.0 * scale,
                max_scroll: built.max_scroll,
                max_scroll_x: built.max_scroll_x,
                popup,
            },
            renders,
        }
    }

    /// Mint dispatch data from the final state of a transition. A
    /// silent pass supplies reveal geometry and resolves hover;
    /// scrolling to reveal changes geometry and earns one rebuild.
    pub(crate) fn retain_dispatch(
        &mut self,
        scale: f64,
        viewport: Size,
        reveal_selection: bool,
    ) -> bool {
        let before = self.hover.clone();
        let mut dispatch = self.build_frame(scale, viewport).dispatch;
        if reveal_selection && self.reveal_selection(&dispatch, scale, viewport) {
            dispatch = self.build_frame(scale, viewport).dispatch;
        }
        let hover_changed = self.hover != before;
        self.last_descends = dispatch.descends.clone();
        self.dispatch = Some(dispatch);
        self.hover_is_current = true;
        hover_changed
    }
}

fn run_frame(description: FrameDescription<'_>, resources: FrameResources<'_>) -> Built {
    let FrameDescription {
        model,
        stack,
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
    let viewport_width = viewport.width;
    #[cfg(target_os = "linux")]
    let viewport_height = viewport.height;
    let mut placed: Placed<App, Paint> = measured::Output::empty();
    // Empty space deselects — the one slot, whichever pane filled it.
    // The bottom of the stack, so every content claim answers first
    // and only a press that claims no edge falls through to here.
    placed
        .handler_mut()
        .on_pointer_down(|app: &mut App, event| {
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
    let styles = crate::styles::editor(scale);
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
    {
        placed = placed.over(measured::place(
            application_menu.bar,
            Placement::new(
                vello::kurbo::Rect::new(0.0, 0.0, viewport_width, content_viewport.y0),
                vello::kurbo::Rect::new(0.0, 0.0, viewport_width, viewport_height),
            ),
        ));
    }
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
    let sources = sources::Sources {
        doc: &model.doc,
        library: &stack.library,
    };
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
    let body = projection::project(
        projection::ProjectDescription {
            sources,
            selection: model.tree_selection(),
            graph_node: graph_node.as_ref(),
            hover: tree_hover,
            hover_node: hover_node.as_ref(),
            collapse: &model.collapse,
            raw: view.raw,
            styles: &styles,
            width: body_width,
            projection: (!view.raw).then_some(&stack.projection),
            foreign: &stack.foreign,
        },
        &mut tcx,
        projection::Hooks {
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
                    let next = {
                        let sources = app.sources();
                        match click.as_ref() {
                            Some(click) => match &click.line {
                                Some(line) => selection::Selection::from_line(&sources, path, line),
                                None => selection::Selection::edge(
                                    &sources,
                                    &app.stack.projection,
                                    path,
                                ),
                            },
                            None => {
                                selection::Selection::edge(&sources, &app.stack.projection, path)
                            }
                        }
                    };
                    app.model.selection = Some(Selected::Tree(next));
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
                        library: &app.stack.library,
                    },
                    &app.stack.projection,
                    &mut app.model.collapse,
                    &path,
                );
            }),
            rename: Rc::new(|app: &mut App, path, index| {
                if let Some(mut pending) = selection::pending_rename(&app.sources(), &path) {
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
                if let Some(pending) = selection::pending_after(&app.sources(), &path) {
                    app.model.selection = Some(Selected::Tree(pending));
                }
            }),
            delete: Rc::new(|app: &mut App| {
                let descends = app.last_descends.clone();
                app.delete_selected_edge(&descends)
            }),
        },
    );
    // The body rides Progred's scroll container: margins pad into the
    // content, the window is the viewport, and the app's clamped
    // offsets (ordinary model state) shift it. The horizontal
    // maximum answers to the LAYOUT width — content should only
    // scroll where even the block forms overflowed it — not the
    // window edge the viewport clips at.
    let content = measured::pad(vello::kurbo::Insets::uniform(margin), body);
    let max_scroll = ((content.extent.height() - content_viewport.height()) / scale).max(0.0);
    let max_scroll_x = ((content.extent.width - (body_width + 2.0 * margin)) / scale).max(0.0);
    let offset = Vec2::new(
        model.scroll_x.clamp(0.0, max_scroll_x) * scale,
        model.scroll.clamp(0.0, max_scroll) * scale,
    );
    let graph_panel_rect = view.graph.then(|| graph_panel(viewport, scale));
    placed = placed.over(placed::place_scrolled(
        content,
        Placement::new(content_viewport, content_viewport),
        offset,
        move |app: &mut App, update| {
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
    ));
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
        placed = placed.over(measured::place(
            pane,
            Placement::new(rect, content_viewport),
        ));
    }

    // The pending row's popup draws after the body, so it overlays
    // and its click targets win. Its anchor came from the body's
    // placement; the stash rides the placed value, no side channel.
    if let Some(popup) = placed.popup.take() {
        let hovered_entry = match tree_hover {
            Some(hover::Hover::Entry(index)) => Some(*index),
            _ => None,
        };
        let commit =
            |app: &mut App, action: &completion::EntryAction| match app.model.selection.take() {
                Some(Selected::Tree(selection::Selection::Pending { path, .. })) => {
                    app.commit_value(path, action);
                }
                Some(Selected::Tree(selection::Selection::PendingEdge {
                    parent,
                    replacing,
                    ..
                })) => {
                    app.commit_label(parent, replacing, action);
                }
                selection => app.model.selection = selection,
            };
        let card = projection::popup_view(&mut tcx, &styles, &popup, hovered_entry, commit);
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
        placed = placed.over(measured::place(
            card,
            Placement::new(rect, content_viewport),
        ));
        placed.popup = Some(popup);
    }

    #[cfg(target_os = "linux")]
    if let Some((x, popup)) = application_menu.popup {
        let rect = popup.extent.rect_at(Point::new(x, content_viewport.y0));
        let headings = vello::kurbo::Rect::new(
            0.0,
            0.0,
            application_menu.heading_width,
            content_viewport.y0,
        );
        placed.handler_mut().on_pointer_down(move |app, event| {
            let point = Point::new(event.state.position.x, event.state.position.y);
            event.button == Some(PointerButton::Primary)
                && !headings.contains(point)
                && !rect.contains(point)
                && app.menu.close()
        });
        placed.handler_mut().on_pointer_down(move |_, event| {
            event.button == Some(PointerButton::Primary)
                && rect.contains(Point::new(event.state.position.x, event.state.position.y))
        });
        placed.handler_mut().on_scroll(move |_, event| {
            rect.contains(Point::new(event.state.position.x, event.state.position.y))
        });
        placed = placed.over(measured::place(
            popup,
            Placement::new(
                rect,
                vello::kurbo::Rect::new(0.0, 0.0, viewport_width, viewport_height),
            ),
        ));
    }
    Built {
        placed,
        max_scroll,
        max_scroll_x,
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
                Some(Claim::Names(Hovered::Graph(graph_view::GraphNode::Root))),
                Some(Point::ZERO),
                false,
                8.0,
            ),
            Some(Hovered::Graph(graph_view::GraphNode::Root))
        );
        assert_eq!(
            resolved_hover(
                Some(&current),
                Some(Claim::Occludes),
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
                Some(Claim::Names(Hovered::Menu(hover))),
                Some(Point::new(20.0, 40.0)),
                false,
                8.0,
            ),
            Some(Hovered::Menu(hover))
        );
        assert_eq!(
            resolved_hover(
                Some(&Hovered::Menu(hover)),
                Some(Claim::Occludes),
                Some(Point::new(20.0, 40.0)),
                false,
                8.0,
            ),
            None
        );
    }
}
