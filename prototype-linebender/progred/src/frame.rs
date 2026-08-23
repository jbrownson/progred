//! One staged pass: place the document, probe hover, mint dispatch.
//! Ink stays latent in the returned frame; rendering it is the
//! caller's choice, so a silent mint never draws.

use crate::completion;
use crate::hover;
use crate::menu;
use crate::model::{Model, ViewFlags};
use crate::navigate;
use crate::placed::{self, Placed};
use crate::projection;
use crate::selection;
use crate::sources;
use crate::stack;
use crate::{App, PendingPaint, content_viewport};
use parley::{FontContext, LayoutContext};
use puri::draw::{Canvas, GlyphRun, Shape};
use puri::edit::EditCtx;
use puri::geometry::Placement;
use puri::handler::{Handler, HasHandler};
use puri::hover::Claim;
use puri::text::TextCtx;
use puri_vello::VelloCanvas;
use std::rc::Rc;
use ui_events::pointer::PointerButton;
use vello::Scene;
use vello::kurbo::{Affine, Point, Size, Stroke, Vec2};
use vello::peniko::{Brush, Color};
use winit::dpi::PhysicalPosition;

pub(crate) const HOVER_REACH: f64 = 8.0;

pub(crate) struct Dispatch {
    pub(crate) handler: Handler<App>,
    pub(crate) activations: Vec<placed::TargetAction<App>>,
    pub(crate) picks: Vec<placed::TargetAction<App>>,
    pub(crate) descends: Vec<navigate::Descend<App>>,
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
    /// The cell-relative location the resolved hover refers to, for
    /// the render pass's secondary marks.
    pub(crate) hovered_secondary: Option<hover::Secondary>,
}

/// What the resting pointer claims in the document or application
/// menu.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Hovered {
    Tree(hover::Hover),
    Menu(menu::Hover),
    /// Pointer-occupied chrome with no editor action. Keeping this
    /// distinct from air prevents the shell's empty-space fallback
    /// without inventing a clickable identity for the chrome.
    Blocked,
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

/// Returns the vertical scroll needed to reveal `target`. A visible
/// top edge is already a useful orientation anchor, even when the
/// target extends below the viewport, so it is left undisturbed.
fn reveal_vertical_scroll(
    current: f64,
    maximum: f64,
    target: vello::kurbo::Rect,
    viewport: vello::kurbo::Rect,
    pad: f64,
    scale: f64,
) -> f64 {
    if (viewport.y0..=viewport.y1).contains(&target.y0) {
        return current;
    }
    let mut scroll = current;
    if target.y1 > viewport.y1 {
        scroll += (target.y1 + pad - viewport.y1) / scale;
    }
    // Checked against the adjusted position, so a target taller than
    // the viewport lands with its top visible.
    let top = target.y0 - (scroll - current) * scale;
    if top < viewport.y0 {
        scroll += (top - pad - viewport.y0) / scale;
    }
    scroll.clamp(0.0, maximum)
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
) -> Option<Hovered> {
    if pressed {
        return prior;
    }
    let point = pointer?;
    match placed.probe(point, prior.as_ref(), reach) {
        Some(Claim::Direct(target) | Claim::Extended(target)) => Some(target),
        Some(Claim::Occludes) => Some(Hovered::Blocked),
        None => None,
    }
}

pub(crate) struct FrameDescription<'a> {
    model: &'a Model,
    stack: &'a stack::Stack<App>,
    view: ViewFlags,
    menu: menu::State,
    availability: menu::Availability,
    scale: f64,
    viewport: Size,
}

pub(crate) struct FrameResources<'a> {
    fonts: &'a mut FontContext,
    layouts: &'a mut LayoutContext<Brush>,
    text_cache: &'a mut puri::text::TextCache,
}

/// The frame as one measured value, plus the scroll maxima its
/// measurement settled.
struct AppView {
    view: measured::Measured<Placed<App, Paint>>,
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
            .selection
            .as_ref()
            .map(|s| (s.path().to_vec(), s.stage()));
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
                            .find(|descend| descend.path.as_ref() == path)
                            .map(|descend| descend.rect)
                    })
                });
            target.is_some_and(|rect| {
                let before = (self.model.scroll, self.model.scroll_x);
                let pad = 12.0 * scale;
                let content = content_viewport(viewport, scale);
                self.model.scroll = reveal_vertical_scroll(
                    self.model.scroll,
                    dispatch.max_scroll,
                    rect,
                    content,
                    pad,
                    scale,
                );
                // The same chase horizontally, against the viewport.
                let visible = viewport.width;
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
        let debug_geometry = view.debug_geometry;
        let availability = self.menu_availability();
        let description = FrameDescription {
            model: &self.model,
            stack: &self.stack,
            view,
            menu: self.menu,
            availability,
            scale,
            viewport,
        };
        let resources = FrameResources {
            fonts: &mut self.font_cx,
            layouts: &mut self.layout_cx,
            text_cache: &mut self.text_cache,
        };
        let AppView {
            view,
            max_scroll,
            max_scroll_x,
        } = app_view(description, resources);
        let placed = measured::place(
            view,
            Placement::root(vello::kurbo::Rect::from_origin_size(
                vello::kurbo::Point::ZERO,
                viewport,
            )),
        );
        let hover_reach = HOVER_REACH * scale;
        self.hover = derive_hover(
            &placed,
            self.hover.take(),
            self.pointer,
            self.pressed,
            hover_reach,
        );
        let hovered_secondary = match &self.hover {
            Some(Hovered::Tree(hover)) => hover::hover_secondary(
                &sources::Sources {
                    doc: &self.model.doc,
                    library: &self.stack.library,
                },
                self.model.view.raw,
                self.model.selection.as_ref(),
                hover,
            ),
            Some(Hovered::Menu(_)) => None,
            Some(Hovered::Blocked) => None,
            None => None,
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
            activations,
            picks,
            handler,
            descends,
            landmark_select,
            popup,
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
                activations,
                picks,
                descends,
                line: 14.0 * scale,
                max_scroll,
                max_scroll_x,
                popup,
            },
            renders,
            hovered_secondary,
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
        let mut frame = self.build_frame(scale, viewport);
        if reveal_selection && self.reveal_selection(&frame.dispatch, scale, viewport) {
            frame = self.build_frame(scale, viewport);
        }
        let Frame {
            dispatch,
            renders,
            hovered_secondary,
        } = frame;
        let hover_changed = self.hover != before;
        self.last_descends = dispatch.descends.clone();
        self.dispatch = Some(dispatch);
        self.pending_paint = Some(PendingPaint {
            scale,
            viewport,
            renders,
            hovered_secondary,
        });
        hover_changed
    }
}

fn app_view(description: FrameDescription<'_>, resources: FrameResources<'_>) -> AppView {
    let FrameDescription {
        model,
        stack,
        view: flags,
        menu,
        availability,
        scale,
        viewport,
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
    let application_menu = menu::DRAWN.then(|| {
        menu::view(
            &mut tcx,
            menu::Description {
                state: menu,
                availability,
                raw: flags.raw,
                debug_geometry: flags.debug_geometry,
                scale,
                width: viewport_width,
            },
            menu::Hooks {
                toggle: Rc::new(|app: &mut App, section| app.menu.toggle(section)),
                select: Rc::new(|app: &mut App, selection| app.choose_menu(selection)),
            },
        )
    });
    let (menu_bar, menu_popup, menu_heading_width) = match application_menu {
        Some(menu) => (Some(menu.bar), menu.popup, menu.heading_width),
        None => (None, None, 0.0),
    };
    let content_viewport = content_viewport(viewport, scale);
    // The Raw view is ONE bit, threaded as itself: name lookups
    // derive from it downstream, no policy swapped here, and the
    // model's configured policy rides along untouched.
    let sources = sources::Sources {
        doc: &model.doc,
        library: &stack.library,
    };
    let margin = 12.0 * scale;
    let body_width = viewport_width - 2.0 * margin;
    let body = projection::project(
        projection::ProjectDescription {
            sources,
            selection: model.selection.as_ref(),
            annotations: &model.annotations,
            raw: flags.raw,
            styles: &styles,
            width: body_width,
            projection: (!flags.raw).then_some(&stack.projection),
            foreign: &stack.foreign,
        },
        &mut tcx,
        projection::Hooks {
            // The host's ordinary structural selection transition.
            // Editable text handles its coordinate-sensitive pointer
            // transition through the stock control's raw handler.
            select: Rc::new(move |app: &mut App, path| {
                let fresh = match app.model.selection.as_ref() {
                    None => true,
                    Some(current) => {
                        current.stage() == selection::Stage::Label || current.path() != path
                    }
                };
                if fresh {
                    let next = selection::Selection::edge(
                        &app.sources(),
                        path,
                    );
                    app.model.selection = Some(next);
                } else if let Some(line) = app
                        .model
                        .selection
                        .as_mut()
                        .and_then(selection::Selection::edit_mut)
                {
                    line.cursor_to_end();
                }
            }),
            start_edit: Rc::new(|app: &mut App, path, line| {
                app.model.selection = Some(selection::Selection::from_line(
                    &app.sources(),
                    path,
                    line,
                ));
            }),
            toggle: Rc::new(|app: &mut App, path| {
                selection::toggle_collapse(
                    &sources::Sources {
                        doc: &app.model.doc,
                        library: &app.stack.library,
                    },
                    &mut app.model.annotations,
                    &path,
                );
            }),
            edit: Rc::new(edit_ctx),
            pick: Rc::new(|app: &mut App, id| app.pick_identity(id)),
            insert: Rc::new(|app: &mut App, path| {
                if let Some(pending) = selection::pending_after(&app.sources(), &path) {
                    app.model.selection = Some(pending);
                }
            }),
            delete: Rc::new(|app: &mut App| {
                let descends = app.last_descends.clone();
                app.delete_selected_edge(&descends)
            }),
            apply: Rc::new(crate::site::apply_event),
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
    // The stage: one tree. Empty-space deselection is the shell's
    // final editor-action fallback after raw pointer handlers and a
    // resolved target's Activate/Pick have declined.
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
            vello::kurbo::Rect::new(0.0, 0.0, viewport_width, content_viewport.y0),
            vello::kurbo::Rect::new(0.0, 0.0, viewport_width, viewport.height),
        );
        stage = measured::overlay(stage, bar, move |_, _, _| Some(bar_placement));
    }
    stage = measured::overlay(
        stage,
        placed::scrolled(content, offset, move |app: &mut App, update| {
            app.scroll_document(
                update,
                scale,
                content_viewport.height(),
                max_scroll,
                max_scroll_x,
            )
        }),
        move |_, _, _| Some(Placement::new(content_viewport, content_viewport)),
    );

    // The pending row's popup floats above everything the body
    // placed, its click targets winning. The card is built while a
    // pending is engaged; its anchor is discovered at place time in
    // the stage's output, where the pending row stashed it.
    let engaged = model.selection.as_ref().and_then(|current| match current.stage() {
        selection::Stage::Pending => Some((current.edit()?, current.choice(), false)),
        selection::Stage::Label => Some((current.edit()?, current.choice(), true)),
        selection::Stage::Edge => None,
    });
    if let Some((query, choice, labels)) = engaged {
        // The same inputs the pending row's stash reads: the drawn
        // rows and the keyboard commit must answer from one list.
        let entries = completion::completion_entries(&sources, flags.raw, labels, query.text());
        let commit =
            |app: &mut App, action: &completion::EntryAction| match app.model.selection.take() {
                Some(current) => match current.stage() {
                    selection::Stage::Pending => {
                        app.commit_value(current.path().to_vec(), action);
                    }
                    selection::Stage::Label => {
                        app.commit_label(current.path().to_vec(), action);
                    }
                    selection::Stage::Edge => {
                        app.model.selection = Some(current);
                    }
                },
                selection => app.model.selection = selection,
            };
        let card = projection::popup_view(&mut tcx, &styles, &entries, choice, commit);
        stage = measured::overlay(stage, card, move |_, extent, out: &Placed<App, Paint>| {
            out.popup.as_ref().map(|popup| {
                // Below the anchor, unless it would run off the
                // bottom and fits above — then flip on top, as the
                // TypeScript prototype did.
                let below = popup.anchor.y1 + 4.0 * scale;
                let above = popup.anchor.y0 - 4.0 * scale - extent.height();
                let y = if below + extent.height() > content_viewport.y1
                    && above >= content_viewport.y0
                {
                    above
                } else {
                    below
                };
                Placement::new(
                    extent.rect_at(Point::new(popup.anchor.x0, y)),
                    content_viewport,
                )
            })
        });
    }

    if let Some((x, popup)) = menu_popup {
        let heading_width = menu_heading_width;
        // Outside presses close the popup. Its own Occludes claim
        // becomes Hovered::Blocked over separators and disabled
        // entries; enabled items resolve their semantic target above
        // it, so no raw inside-swallow may preempt them.
        let popup = placed::before(popup, move |p, placement| {
            let rect = placement.rect;
            let headings =
                vello::kurbo::Rect::new(0.0, 0.0, heading_width, content_viewport.y0);
            p.handler().on_pointer_down(move |app: &mut App, event| {
                let point = Point::new(event.state.position.x, event.state.position.y);
                event.button == Some(PointerButton::Primary)
                    && !headings.contains(point)
                    && !rect.contains(point)
                    && app.menu.close()
            });
            p.handler().on_scroll(move |_: &mut App, event| {
                rect.contains(Point::new(event.state.position.x, event.state.position.y))
            });
        });
        stage = measured::overlay(stage, popup, move |_, extent, _| {
            Some(Placement::new(
                extent.rect_at(Point::new(x, content_viewport.y0)),
                vello::kurbo::Rect::new(0.0, 0.0, viewport_width, viewport.height),
            ))
        });
    }
    AppView {
        view: stage,
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
    fn an_oversized_target_with_a_visible_top_does_not_scroll() {
        let viewport = vello::kurbo::Rect::new(0.0, 30.0, 400.0, 200.0);
        let target = vello::kurbo::Rect::new(20.0, 80.0, 380.0, 500.0);

        assert_eq!(
            reveal_vertical_scroll(120.0, 1_000.0, target, viewport, 12.0, 1.0),
            120.0
        );
    }

    #[test]
    fn a_target_starting_below_the_viewport_is_still_revealed() {
        let viewport = vello::kurbo::Rect::new(0.0, 30.0, 400.0, 200.0);
        let target = vello::kurbo::Rect::new(20.0, 220.0, 380.0, 260.0);

        assert_eq!(
            reveal_vertical_scroll(120.0, 1_000.0, target, viewport, 12.0, 1.0),
            192.0
        );
    }

    #[test]
    fn hover_prefers_direct_claims_and_uses_extensions_only_to_retain() {
        let target = |index| Hovered::Tree(hover::Hover::Entry(index));
        let viewport = vello::kurbo::Rect::new(-100.0, -100.0, 100.0, 100.0);
        let mut placed: Placed<App, Paint> = Placed::empty();
        placed.probes.push(placed::Probe::direct(
            Placement::new(
                vello::kurbo::Rect::new(0.0, 0.0, 10.0, 10.0),
                viewport,
            ),
            target(0),
        ));
        placed.probes.push(placed::Probe::direct(
            Placement::new(
                vello::kurbo::Rect::new(14.0, 0.0, 24.0, 10.0),
                viewport,
            ),
            target(1),
        ));
        // A direct answer establishes hover.
        assert_eq!(
            derive_hover(
                &placed,
                None,
                Some(Point::new(5.0, 5.0)),
                false,
                8.0,
            ),
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
            ),
            Some(target(0))
        );
        assert_eq!(
            derive_hover(
                &placed,
                Some(target(1)),
                Some(Point::new(12.0, 5.0)),
                false,
                8.0,
            ),
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
            ),
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
            ),
            Some(target(1))
        );
        assert_eq!(
            derive_hover(
                &placed,
                None,
                Some(Point::new(12.0, 5.0)),
                false,
                8.0,
            ),
            None
        );
        assert_eq!(derive_hover(&placed, Some(target(0)), None, false, 8.0), None);
        // A pressed gesture keeps the hover it began with.
        assert_eq!(
            derive_hover(
                &placed,
                Some(target(0)),
                Some(Point::new(40.0, 40.0)),
                true,
                8.0,
            ),
            Some(target(0))
        );
        // An occluder answers "blocked" outright and blocks the
        // fallback — an overlay's pointer never lights what sits
        // beneath it or triggers the empty-space action.
        placed.probes.push(placed::Probe::occludes(Placement::new(
            vello::kurbo::Rect::new(0.0, 0.0, 40.0, 40.0),
            viewport,
        )));
        assert_eq!(
            derive_hover(
                &placed,
                None,
                Some(Point::new(25.0, 5.0)),
                false,
                8.0,
            ),
            Some(Hovered::Blocked)
        );
    }
}
