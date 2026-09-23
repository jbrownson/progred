//! Project → compute hover → bind paint and dispatch.
//! Paint stays latent in the returned frame; rendering it is the
//! caller's choice, so a silent mint never draws.

use crate::hover;
use crate::menu;
use crate::model::Model;
use crate::navigate;
use crate::placed::{self, HoverPass};
use crate::projection;
use crate::sources;
use crate::stack;
use crate::workspace::{self, Root};
use crate::{Editor, EditorRunner, PendingPaint, content_viewport};
use kurbo::{Affine, Insets, Point, Rect, Size, Stroke, Vec2};
use parley::{FontContext, LayoutContext};
use peniko::{Brush, Color};
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
    pub(crate) handler: Handler<Editor, placed::DispatchContext<Editor>>,
    pub(crate) pointer_root: Option<crate::workspace::Root>,
    pub(crate) descends: Rc<[navigate::Descend<Editor>]>,
    pub(crate) view_regions: Rc<[placed::ViewRegion]>,
    pub(crate) hover_geometry: crate::display::widget::frame::HoverGeometry<Hovered>,
    /// One nominal line height at the frame's scale — the quantum
    /// keyboard navigation reads rows with.
    pub(crate) line: f64,
}

/// A completed frame. Installing it retains hover and dispatch together;
/// its paint continuations can be run separately or discarded.
pub(crate) struct Frame {
    pub(crate) scroll_probes: Vec<crate::display::widget::scroll::Probe>,
    pub(crate) hover: Option<Hovered>,
    pub(crate) dispatch: Dispatch,
    pub(crate) renders: Vec<placed::Render>,
}

#[derive(Default)]
pub(crate) struct FrameState {
    pub(crate) hover: Option<Hovered>,
    pub(crate) dispatch: Dispatch,
    pub(crate) pending_paint: Option<PendingPaint>,
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
    pub(crate) fn geometry(&self, scale: f64) -> navigate::Geometry<'_> {
        navigate::Geometry {
            descends: &self.descends,
            view_regions: &self.view_regions,
            scale,
        }
    }

    pub(crate) fn context(&self, hover: Option<Hovered>) -> placed::DispatchContext<Editor> {
        placed::DispatchContext {
            descends: self.descends.clone(),
            view_regions: self.view_regions.clone(),
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

/// A fresh description of this frame's winner, shared by its continuations.
/// Resolve source paths here once, not in each painted occurrence.
fn attribute_hover(
    sources: &sources::Sources<'_>,
    descends: &[navigate::Descend<Editor>],
    root: Option<&workspace::Root>,
    completion: Option<&crate::completion::Offers<Editor>>,
    hovered: Option<Hovered>,
    link: bool,
) -> placed::ResolvedHover {
    // Keep the probe's call chain for subsequent input, but use the selected
    // source for this frame's ordinary source/secondary decoration.
    let hovered = match hovered {
        Some(Hovered::Tree(ref calls @ hover::Hover::Calls(_))) => {
            crate::projection::source_link::hover_source(sources, descends, calls)
                .map(|source| Hovered::Tree(hover::Hover::Source(source)))
        }
        other => other,
    };
    let visible = source_hover_visible(hovered.as_ref(), link);
    let source_path = match &hovered {
        Some(Hovered::Tree(hover::Hover::Value(path))) => descends
            .iter()
            .find(|d| d.root.as_ref() == root && d.path == *path)
            .and_then(|d| d.scope.source(path))
            .map(|path| std::rc::Rc::<[gid::Step]>::from(path.as_ref())),
        _ => None,
    };
    let hovered_secondary = match &hovered {
        Some(Hovered::Tree(hover::Hover::Value(_))) if visible => {
            source_path.as_ref().and_then(|path| {
                sources.resolve_path(path).map(|value| {
                    hover::Secondary::from_path(sources, path.clone(), value.as_cell())
                })
            })
        }
        Some(Hovered::Tree(hover)) if visible => hover::hover_secondary(sources, completion, hover),
        _ => None,
    };
    let hovered_trace = match &hovered {
        Some(Hovered::Tree(hover::Hover::Value(_))) if visible => {
            source_path.map(|path| hover::SourceTrace::from_path(sources, path))
        }
        Some(Hovered::Tree(hover::Hover::Source(source))) if visible => Some(source.clone()),
        _ => None,
    };
    placed::ResolvedHover {
        hovered,
        hovered_secondary,
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
    // Structural paste precedes a pending's text query, while menu handling
    // still takes precedence over all document editing when installed.
    frame.dispatch.handler.on(|editor, event, _| {
        let handled = match &event {
            Event::Key(key) => editor.pending_paste_key(key),
            _ => false,
        };
        puri::handler::EventOutcome::from_handled(event, handled)
    });
    frame.dispatch.handler = frame.dispatch.handler.over(keyboard);
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
            pointer: if pointer.pressed {
                None
            } else {
                pointer.position
            },
            prior: pointer.previous.map(|(_, hover)| hover),
            reach_px: HOVER_REACH_POINTS * description.scale,
            debug_geometry: description.model.view.debug_geometry,
        },
    );
    let (hover, pointer_root) = if pointer.pressed {
        pointer.previous.map_or((None, None), |(root, hover)| {
            (Some(hover.clone()), root.cloned())
        })
    } else {
        hover_target(output.claim.take())
    };
    let resolved = attribute_hover(
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
    if description.model.view.debug_geometry {
        let extended_rects: Vec<_> = output
            .debug_regions
            .iter()
            .filter_map(|(target, rect)| (Some(target) == hover.as_ref()).then_some(*rect))
            .collect();
        output.after_hover.push(move |_, effects| {
            effects.renders.push(Box::new(move |canvas| {
                let guide = Color::new([0.92, 0.12, 0.58, 0.80]);
                for rect in extended_rects {
                    canvas.stroke(rect, Stroke::new(1.0), guide, Affine::IDENTITY);
                }
            }));
        });
    }
    debug_assert!(
        output.landmark_select.is_none(),
        "selection handler escaped its landmark"
    );
    let crate::display::widget::frame::FrameOutput {
        scroll_probes,
        renders,
        handler,
        descends,
        view_regions,
        hover_geometry,
    } = output.bind(resolved);
    Frame {
        scroll_probes,
        hover,
        dispatch: Dispatch {
            handler: handler.unwrap_or_else(Handler::new),
            pointer_root,
            descends: descends.into(),
            view_regions: view_regions.into(),
            hover_geometry,
            line: 14.0 * description.scale,
        },
        renders,
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
            let mut input = self.frame.dispatch.context(self.frame.hover.clone());
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
        } = frame;
        #[cfg(target_arch = "wasm32")]
        crate::web_scroll::install(scroll_probes, scale);
        #[cfg(not(target_arch = "wasm32"))]
        drop(scroll_probes);
        self.frame.hover = hover;
        self.frame.dispatch = dispatch;
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
    command_modifier: puri::keyboard::CommandModifier,
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
    let viewport = match view.root.target() {
        workspace::Target::Pane { path } if !raw => projection::viewport::entry(sources, path),
        _ => None,
    };
    let viewport = viewport.map(|entry| {
        (
            entry,
            projection::viewport::projection(&stack.projection, size / scale),
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
            (sources.root(), &stack.projection)
        }
        workspace::Target::Pane { path } => {
            if let Some((entry, projection)) = &viewport {
                root_path = entry.path.clone();
                (Some(entry.value), projection)
            } else {
                root_path = path.clone();
                (sources.resolve_path(path), &stack.pane_projection)
            }
        }
    };
    let projected = projection::project(
        projection::ProjectDescription {
            command_modifier,
            computations: Some(computations),
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
            projection: (!raw).then_some(projection),
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
    command_modifier: puri::keyboard::CommandModifier,
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
        let view = model
            .workspace
            .view(&placed_view.root)
            .expect("workspace geometry only names live views");
        let rect = placed_view.rect;
        let child = project_workspace_view(
            model,
            command_modifier,
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
        toggles,
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
                toggles,
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
        command_modifier,
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
mod frame_tests {
    use super::*;
    use gid::{CellId, Cells, Document, Step, Value};

    type HoverLog = Rc<std::cell::RefCell<Vec<(&'static str, f64)>>>;
    const HOVER_VIEWPORT: Size = Size::new(500.0, 400.0);

    #[test]
    fn source_hover_conjects_the_occurrence_in_its_own_view() {
        let field = gid::new_cell_id();
        let occurrence: Rc<[Step]> = Rc::from([Step::Key(gid::new_cell_id())]);
        let mut editor = crate::test_editor(Document {
            root: Some(Value::record([(
                field,
                crate::libraries::text::value("shared"),
            )])),
            cells: Cells::new(),
        });
        let root = editor.model.workspace.document_root().clone();
        let source = vec![Step::Key(field)];
        let scope = crate::editing::Scope::default().with_conject(
            occurrence.to_vec(),
            source.clone(),
            crate::display::Conject::descend(),
        );
        let mut landmarks = [navigate::Descend::<Editor> {
            path: occurrence.clone(),
            root: Some(root.clone()),
            rect: Rect::new(0.0, 0.0, 10.0, 10.0),
            select: Rc::new(|_, _| true),
            scope,
        }];
        let hover = Some(Hovered::Tree(hover::Hover::Value(occurrence.clone())));
        let resolved = attribute_hover(
            &editor.sources(),
            &landmarks,
            Some(&root),
            None,
            hover.clone(),
            false,
        );
        let expected = hover::SourceTrace::Stored(Rc::from(source));
        assert_eq!(resolved.hovered_trace, Some(expected.clone()));
        assert_eq!(
            resolved.hovered_secondary,
            Some(hover::Secondary::from_trace(&expected))
        );
        // A held press must carry the owning view into successor attribution,
        // not just preserve the displayed hover path.
        let observed = Rc::new(std::cell::RefCell::new(None));
        let captured = observed.clone();
        let child = crate::display::widget::leaf(
            measured::Extent::default(),
            move |output: &mut placed::HoverContext<'_, Editor, Hovered>, _| {
                output.after_hover(move |hover, _| {
                    *captured.borrow_mut() =
                        Some((hover.hovered_trace.clone(), hover.hovered_secondary.clone()));
                });
            },
        );
        let layout = crate::display::widget::navigation::landmark(
            child,
            occurrence.clone(),
            Rc::new(|_, _| true),
            landmarks[0].scope.clone(),
        );
        let frame = hover_frame(
            &editor,
            placed::in_view(layout, root.clone()),
            None,
            Some((Some(&root), hover.as_ref().unwrap())),
            true,
        );
        assert_eq!(frame.dispatch.pointer_root, Some(root.clone()));
        assert_eq!(
            *observed.borrow(),
            Some((
                Some(expected.clone()),
                Some(hover::Secondary::from_trace(&expected))
            ))
        );
        // A detached occurrence must not acquire provenance even when its
        // spelling also names real document data.
        Rc::make_mut(&mut editor.model.doc).root = Some(Value::record([
            (field, crate::libraries::text::value("shared")),
            (
                match occurrence[0] {
                    Step::Key(key) => key,
                    _ => unreachable!(),
                },
                crate::libraries::text::value("coincidence"),
            ),
        ]));
        landmarks[0].scope = crate::editing::Scope::default().detached(occurrence.to_vec());
        let resolved = attribute_hover(
            &editor.sources(),
            &landmarks,
            Some(&root),
            None,
            hover,
            false,
        );
        assert!(resolved.hovered_trace.is_none());
        assert!(resolved.hovered_secondary.is_none());
    }

    fn hover_runner(log: &HoverLog, change_target: bool) -> EditorRunner {
        use crate::display::{Layout, partial, widget};
        use crate::libraries::f64;
        use puri::handler::EventOutcome;

        let mut editor = crate::test_editor(Document {
            root: Some(f64::value(0.0)),
            cells: Cells::new(),
        });
        editor.pointer = Some(Point::new(25.0, 25.0));
        editor.stack.projection = projection::Projection::new([partial({
            let log = log.clone();
            move |input| {
                let value = f64::read(input.value?)?;
                log.borrow_mut().push(("project", value));
                let log = log.clone();
                Some(Layout::widget(Rc::new(move |_| {
                    let log = log.clone();
                    widget::leaf(
                        measured::Extent {
                            width: 100.0,
                            ascent: 40.0,
                            descent: 0.0,
                        },
                        move |output, placement| {
                            let target = Hovered::Tree(if value as u64 % 2 == 0 {
                                hover::Hover::Value(Rc::from([]))
                            } else {
                                hover::Hover::Toggle(Rc::from([]))
                            });
                            output.claim(placed::Probe::exact(placement, target));
                            output.after_hover(move |_, effects| {
                                effects.handler().on({
                                    let log = log.clone();
                                    move |editor: &mut Editor, event, input| {
                                        if matches!(event, Event::HoverChanged) {
                                            log.borrow_mut().push((
                                                if input.hovered().is_some() {
                                                    "hover"
                                                } else {
                                                    "leave"
                                                },
                                                value,
                                            ));
                                            if input.hovered().is_some() {
                                                if change_target {
                                                    Rc::make_mut(&mut editor.model.doc).root =
                                                        Some(f64::value(value + 1.0));
                                                }
                                                return EventOutcome::accept();
                                            }
                                        }
                                        EventOutcome::decline(event)
                                    }
                                });
                                effects.renders.push(Box::new(move |_| {
                                    log.borrow_mut().push(("paint", value))
                                }));
                            });
                        },
                    )
                })))
            }
        })]);
        EditorRunner::new(editor)
    }

    fn paint_hover_frame(runner: &mut EditorRunner) -> bool {
        puri::frame::render(
            runner.prepare_paint(1.0, HOVER_VIEWPORT).renders,
            &mut puri::draw::DrawList::default(),
        );
        runner.frame_presented()
    }

    fn move_hover_pointer(runner: &mut EditorRunner, point: Point) {
        use ui_events::pointer::{PointerEvent, PointerInfo, PointerState, PointerUpdate};
        assert!(runner.pointer_event(
            &PointerEvent::Move(PointerUpdate {
                pointer: PointerInfo {
                    pointer_id: None,
                    persistent_device_id: None,
                    pointer_type: ui_events::pointer::PointerType::Mouse,
                },
                current: PointerState {
                    position: (point.x, point.y).into(),
                    ..Default::default()
                },
                coalesced: Vec::new(),
                predicted: Vec::new(),
            }),
            1.0,
            HOVER_VIEWPORT,
        ));
        assert!(runner.flush_pending_continuous());
    }

    #[test]
    fn pointer_hover_reacts_against_installed_geometry_before_one_successor_build() {
        let log = HoverLog::default();
        let mut runner = hover_runner(&log, false);
        runner.editor.pointer = None;
        runner.refresh_frame(1.0, HOVER_VIEWPORT);
        assert!(!paint_hover_frame(&mut runner));
        log.take();

        move_hover_pointer(&mut runner, Point::new(25.0, 25.0));
        assert_eq!(log.take(), [("hover", 0.0), ("project", 0.0)]);
        assert!(!paint_hover_frame(&mut runner));
        assert_eq!(log.take(), [("paint", 0.0)]);

        move_hover_pointer(&mut runner, Point::new(26.0, 25.0));
        assert_eq!(log.take(), [("project", 0.0)]);
        assert!(!paint_hover_frame(&mut runner));
        assert_eq!(log.take(), [("paint", 0.0)]);

        move_hover_pointer(&mut runner, Point::new(450.0, 300.0));
        assert_eq!(log.take(), [("leave", 0.0), ("project", 0.0)]);
        assert!(!paint_hover_frame(&mut runner));
    }

    #[test]
    fn pointer_hover_reactions_that_change_geometry_wait_for_paint_before_reacting_again() {
        let log = HoverLog::default();
        let mut runner = hover_runner(&log, true);
        runner.editor.pointer = None;
        runner.refresh_frame(1.0, HOVER_VIEWPORT);
        assert!(!paint_hover_frame(&mut runner));
        log.take();

        move_hover_pointer(&mut runner, Point::new(25.0, 25.0));
        assert_eq!(log.take(), [("hover", 0.0), ("project", 1.0)]);
        assert_eq!(
            runner.frame.hover,
            Some(Hovered::Tree(hover::Hover::Toggle(Rc::from([]))))
        );
        assert!(paint_hover_frame(&mut runner));
        assert_eq!(log.take(), [("paint", 1.0)]);
        assert!(paint_hover_frame(&mut runner));
        assert_eq!(
            log.take(),
            [
                ("project", 1.0),
                ("hover", 1.0),
                ("project", 2.0),
                ("paint", 2.0)
            ]
        );
    }

    #[test]
    fn hover_reactions_continue_across_painted_frames_without_recursion_or_a_limit() {
        let log = HoverLog::default();
        let mut runner = hover_runner(&log, true);
        runner.refresh_frame(1.0, HOVER_VIEWPORT);
        assert_eq!(
            log.take(),
            [("project", 0.0), ("hover", 0.0), ("project", 1.0)]
        );
        assert!(paint_hover_frame(&mut runner));
        assert_eq!(log.take(), [("paint", 1.0)]);
        for value in 1..20 {
            assert!(paint_hover_frame(&mut runner));
            let value = value as f64;
            assert_eq!(
                log.take(),
                [
                    ("project", value),
                    ("hover", value),
                    ("project", value + 1.0),
                    ("paint", value + 1.0),
                ]
            );
        }
        // An ordinary input can interrupt the oscillation between paintings.
        runner.update_frame(1.0, HOVER_VIEWPORT, |editor, _, _| {
            editor.pointer = None;
            FrameDisposition::Remint
        });
        assert_eq!(log.take(), [("project", 20.0), ("leave", 20.0)]);
        assert!(!paint_hover_frame(&mut runner));
        assert_eq!(log.take(), [("paint", 20.0)]);
    }

    #[test]
    fn accepting_hover_without_changing_it_only_builds_one_successor() {
        let log = HoverLog::default();
        let mut runner = hover_runner(&log, false);
        runner.refresh_frame(1.0, HOVER_VIEWPORT);
        assert_eq!(
            log.take(),
            [("project", 0.0), ("hover", 0.0), ("project", 0.0)]
        );
        assert!(!paint_hover_frame(&mut runner));
        assert_eq!(log.take(), [("paint", 0.0)]);
        runner.refresh_frame(1.0, HOVER_VIEWPORT);
        assert_eq!(log.take(), [("project", 0.0)]);
        assert!(!paint_hover_frame(&mut runner));
    }

    #[test]
    fn preparing_or_replacing_unpainted_frames_does_not_release_hover_feedback() {
        let log = HoverLog::default();
        let mut runner = hover_runner(&log, true);
        runner.refresh_frame(1.0, HOVER_VIEWPORT);
        log.borrow_mut().clear();
        drop(runner.prepare_paint(1.0, HOVER_VIEWPORT));
        runner.refresh_frame(1.0, HOVER_VIEWPORT);
        drop(runner.prepare_paint(1.0, HOVER_VIEWPORT));
        assert_eq!(log.take(), [("project", 1.0)]);
        assert!(paint_hover_frame(&mut runner));
        assert_eq!(log.take(), [("project", 1.0), ("paint", 1.0)]);
        assert!(paint_hover_frame(&mut runner));
        assert_eq!(
            log.take(),
            [
                ("project", 1.0),
                ("hover", 1.0),
                ("project", 2.0),
                ("paint", 2.0)
            ]
        );
    }

    #[test]
    fn new_input_supersedes_an_unpainted_target_without_replaying_stale_handlers() {
        let log = HoverLog::default();
        let mut runner = hover_runner(&log, true);
        runner.refresh_frame(1.0, HOVER_VIEWPORT);
        log.borrow_mut().clear();
        runner.update_frame(1.0, HOVER_VIEWPORT, |editor, _, _| {
            editor.pointer = None;
            FrameDisposition::Remint
        });
        assert_eq!(log.take(), [("project", 1.0)]);
        assert!(paint_hover_frame(&mut runner));
        assert_eq!(log.take(), [("paint", 1.0)]);
        assert!(!paint_hover_frame(&mut runner));
        assert_eq!(
            log.take(),
            [("project", 1.0), ("leave", 1.0), ("paint", 1.0)]
        );
    }

    #[test]
    fn hover_notifications_include_view_ownership_and_are_reset_with_the_document() {
        let mut runner = EditorRunner::new(crate::test_editor(Document {
            root: None,
            cells: Cells::new(),
        }));
        let roots = Rc::new(std::cell::RefCell::new(Vec::new()));
        runner.frame.dispatch.handler.on({
            let roots = roots.clone();
            move |_, event, input| {
                if matches!(event, Event::HoverChanged) {
                    roots.borrow_mut().push(input.root.clone());
                }
                puri::handler::EventOutcome::decline(event)
            }
        });
        runner.frame.hover = Some(Hovered::Blocked);
        let first = Root::document();
        let second = Root::document();
        runner.frame.dispatch.pointer_root = Some(first.clone());
        runner.notify_hover_changed(1.0, HOVER_VIEWPORT);
        runner.notify_hover_changed(1.0, HOVER_VIEWPORT);
        runner.frame.dispatch.pointer_root = Some(second.clone());
        runner.notify_hover_changed(1.0, HOVER_VIEWPORT);
        assert_eq!(roots.take(), [Some(first), Some(second)]);
        runner.frame.hover_awaits_paint = true;
        runner.adopt_model(
            Document {
                root: None,
                cells: Cells::new(),
            },
            None,
            Default::default(),
        );
        assert!(runner.frame.notified_hover.is_none());
        assert!(!runner.frame.hover_awaits_paint);
        assert!(!runner.frame_presented());
    }

    fn scrolling_runner(projected: &Rc<std::cell::Cell<usize>>) -> EditorRunner {
        let mut editor = crate::test_editor(Document {
            root: Some(Value::list(
                (0..40).map(|n| crate::libraries::f64::value(n as f64)),
            )),
            cells: Cells::new(),
        });
        let projected = projected.clone();
        editor.stack.projection =
            projection::Projection::new([crate::display::partial(move |input| {
                crate::libraries::f64::read(input.value?)?;
                projected.set(projected.get() + 1);
                Some(crate::display::Layout::widget(Rc::new(|_| {
                    crate::display::widget::leaf(
                        measured::Extent {
                            width: 100.0,
                            ascent: 25.0,
                            descent: 5.0,
                        },
                        |_, _| {},
                    )
                })))
            })]);
        EditorRunner::new(editor)
    }

    #[test]
    fn selection_reveal_uses_installed_geometry_before_one_successor_build() {
        let projected = Rc::new(std::cell::Cell::new(0));
        let mut runner = scrolling_runner(&projected);
        let viewport = Size::new(500.0, 400.0);
        runner.refresh_frame(1.0, viewport);
        let per_frame = projected.replace(0);
        let path = vec![Step::Element(
            runner
                .editor
                .model
                .doc
                .root
                .as_ref()
                .unwrap()
                .as_list()
                .unwrap()
                .keys()
                .last()
                .unwrap()
                .clone(),
        )];
        let target = runner
            .frame
            .dispatch
            .descends
            .iter()
            .find(|target| target.path.as_ref() == path)
            .unwrap();
        assert!(target.rect.y0 > viewport.height);
        let target = target.clone();
        assert!(runner.update_frame(1.0, viewport, |editor, dispatch, _| {
            frame_disposition(dispatch.geometry(1.0).arrive(editor, &target, None), false)
        }));
        assert_eq!(projected.replace(0), per_frame);
        assert!(runner.editor.model.workspace.document.scroll.y > 0.0);
        let target = runner
            .frame
            .dispatch
            .descends
            .iter()
            .find(|target| target.path.as_ref() == path)
            .unwrap();
        assert!((0.0..viewport.height).contains(&target.rect.y0));

        // Scrolling away from an unchanged selection must not reveal it again.
        runner.update_frame(1.0, viewport, |editor, dispatch, _| {
            frame_disposition(
                dispatch
                    .handler
                    .dispatch_scroll(
                        editor,
                        &ui_events::pointer::PointerScrollEvent {
                            pointer: ui_events::pointer::PointerInfo {
                                pointer_id: None,
                                persistent_device_id: None,
                                pointer_type: ui_events::pointer::PointerType::Mouse,
                            },
                            state: ui_events::pointer::PointerState {
                                position: (200.0, 200.0).into(),
                                ..Default::default()
                            },
                            delta: ui_events::ScrollDelta::PixelDelta((0.0, 2_000.0).into()),
                        },
                    )
                    .handled(),
                false,
            )
        });
        assert_eq!(runner.editor.model.workspace.document.scroll, Vec2::ZERO);
        assert_eq!(projected.get(), per_frame);
    }

    #[test]
    fn refreshes_and_unrelated_inputs_do_not_reveal_an_existing_selection() {
        let projected = Rc::new(std::cell::Cell::new(0));
        let mut runner = scrolling_runner(&projected);
        let viewport = Size::new(500.0, 400.0);
        runner.refresh_frame(1.0, viewport);
        let path = runner
            .frame
            .dispatch
            .descends
            .iter()
            .find(|target| target.rect.y0 > viewport.height)
            .unwrap()
            .path
            .to_vec();
        runner.editor.model.selection = Some(crate::selection::Selection::edge(
            runner.editor.model.workspace.document_root(),
            path,
        ));

        runner.refresh_frame(1.0, viewport);
        assert_eq!(runner.editor.model.workspace.document.scroll, Vec2::ZERO);
        runner.update_frame(1.0, viewport, |_, _, _| FrameDisposition::Remint);
        assert_eq!(runner.editor.model.workspace.document.scroll, Vec2::ZERO);
    }

    #[test]
    fn selection_changes_do_not_implicitly_request_reveal() {
        let projected = Rc::new(std::cell::Cell::new(0));
        let mut runner = scrolling_runner(&projected);
        let viewport = Size::new(500.0, 400.0);
        runner.refresh_frame(1.0, viewport);
        let path = runner
            .frame
            .dispatch
            .descends
            .iter()
            .find(|target| target.rect.y0 > viewport.height)
            .unwrap()
            .path
            .to_vec();
        runner.update_frame(1.0, viewport, |editor, _, _| {
            crate::editing::select(
                editor,
                &editor.model.workspace.document_root().clone(),
                &path,
            );
            FrameDisposition::Remint
        });
        assert_eq!(runner.editor.model.workspace.document.scroll, Vec2::ZERO);

        runner.update_frame(1.0, viewport, |editor, _, _| {
            editor.model.selection = Some(crate::selection::pending_value(
                editor.model.workspace.document_root(),
                path,
            ));
            FrameDisposition::Remint
        });
        assert_eq!(runner.editor.model.workspace.document.scroll, Vec2::ZERO);
    }

    fn select_offscreen(runner: &mut EditorRunner, viewport: Size) -> gid::Path {
        let target = runner
            .frame
            .dispatch
            .descends
            .iter()
            .find(|target| target.rect.y0 > viewport.height)
            .unwrap();
        let path = target.path.to_vec();
        crate::editing::select(&mut runner.editor, target.root.as_ref().unwrap(), &path);
        path
    }

    #[test]
    fn keyboard_navigation_deletion_and_pending_cancellation_explicitly_reveal() {
        use ui_events::keyboard::{Key, KeyState, KeyboardEvent, NamedKey};
        for key in [NamedKey::ArrowDown, NamedKey::Delete, NamedKey::Backspace] {
            let projected = Rc::new(std::cell::Cell::new(0));
            let mut runner = scrolling_runner(&projected);
            let viewport = Size::new(500.0, 400.0);
            runner.refresh_frame(1.0, viewport);
            let path = select_offscreen(&mut runner, viewport);
            if key == NamedKey::Backspace {
                runner.editor.model.selection = Some(crate::selection::pending_value(
                    runner.editor.model.workspace.document_root(),
                    path.clone(),
                ));
            }
            // Include the selected widget's installed delete handler, not only
            // the shell's structural fallback.
            runner.refresh_frame(1.0, viewport);
            assert_eq!(runner.editor.model.workspace.document.scroll, Vec2::ZERO);
            projected.set(0);
            assert!(runner.keyboard_event(
                &KeyboardEvent {
                    key: Key::Named(key),
                    state: KeyState::Down,
                    ..Default::default()
                },
                1.0,
                viewport
            ));
            assert_ne!(runner.editor.model.selection.as_ref().unwrap().path(), path);
            assert!(runner.editor.model.workspace.document.scroll.y > 0.0);
            assert_eq!(
                projected.get(),
                if key == NamedKey::Delete { 39 } else { 40 }
            );
        }
    }

    #[test]
    fn omitted_menu_keeps_history_but_not_menu_commands() {
        use ui_events::keyboard::{Key, KeyState, KeyboardEvent, Modifiers, NamedKey};
        let doc = crate::gid_text::parse(include_str!("../../website/public/lessons/values.gid"))
            .unwrap()
            .0;
        let mut runner = crate::EditorRunner::new(crate::test_editor(doc.clone()));
        runner.editor.command_modifier = puri::keyboard::CommandModifier::Control;
        let viewport = Size::new(620.0, 304.0);
        runner
            .editor
            .model
            .history
            .record(runner.editor.model.snapshot());
        Rc::make_mut(&mut runner.editor.model.doc).root = Some(gid::Value::list([]));
        runner.refresh_frame(1.0, viewport);
        let input = |key, modifiers| KeyboardEvent {
            key,
            modifiers,
            state: KeyState::Down,
            ..Default::default()
        };
        assert!(runner.keyboard_event(
            &input(Key::Character("z".into()), Modifiers::CONTROL),
            1.0,
            viewport
        ));
        assert_eq!(runner.editor.model.doc.root, doc.root);
        for event in [
            input(Key::Character("n".into()), Modifiers::CONTROL),
            input(Key::Character("r".into()), Modifiers::CONTROL),
            input(Key::Character("d".into()), Modifiers::CONTROL),
            input(Key::Character("p".into()), Modifiers::CONTROL),
            input(Key::Character("1".into()), Modifiers::CONTROL),
            input(Key::Named(NamedKey::F10), Modifiers::empty()),
        ] {
            assert!(!runner.keyboard_event(&event, 1.0, viewport));
        }
        assert!(runner.keyboard_event(
            &input(
                Key::Character("z".into()),
                Modifiers::CONTROL | Modifiers::SHIFT
            ),
            1.0,
            viewport
        ));
        assert_eq!(runner.editor.model.doc.root, Some(gid::Value::list([])));
    }

    #[test]
    fn raw_shortcut_is_contributed_only_by_the_menu() {
        use puri::keyboard::CommandModifier;
        use ui_events::keyboard::{Key, KeyState, KeyboardEvent, Modifiers};
        for (modifier, modifiers) in [
            (CommandModifier::Meta, Modifiers::META),
            (CommandModifier::Control, Modifiers::CONTROL),
        ] {
            for menu in [false, true] {
                let mut editor = crate::test_editor(Document {
                    root: Some(Value::list([])),
                    cells: Cells::new(),
                });
                editor.drawn_menu = menu;
                editor.command_modifier = modifier;
                let mut runner = crate::EditorRunner::new(editor);
                let viewport = Size::new(620.0, 304.0);
                runner.refresh_frame(1.0, viewport);
                assert!(!runner.editor.menu_toggles().raw);
                assert_eq!(
                    runner.keyboard_event(
                        &KeyboardEvent {
                            key: Key::Character("r".into()),
                            modifiers,
                            state: KeyState::Down,
                            ..Default::default()
                        },
                        1.0,
                        viewport
                    ),
                    menu
                );
                assert_eq!(runner.editor.menu_toggles().raw, menu);
            }
        }
    }

    #[test]
    fn native_and_drawn_history_commands_reveal_even_the_same_selection() {
        use ui_events::keyboard::{Key, KeyState, KeyboardEvent, Modifiers};
        for drawn in [false, true] {
            let mut runner = scrolling_runner(&Default::default());
            runner.editor.drawn_menu = drawn;
            runner.editor.command_modifier = puri::keyboard::CommandModifier::Control;
            let viewport = Size::new(500.0, 400.0);
            runner.refresh_frame(1.0, viewport);
            let path = select_offscreen(&mut runner, viewport);
            runner
                .editor
                .model
                .history
                .record(runner.editor.model.snapshot());
            // History restores the same selection. It still explicitly reveals;
            // selection equality is not involved in the command's policy.
            if drawn {
                assert!(runner.keyboard_event(
                    &KeyboardEvent {
                        key: Key::Character("z".into()),
                        modifiers: Modifiers::CONTROL,
                        state: KeyState::Down,
                        ..Default::default()
                    },
                    1.0,
                    viewport
                ));
            } else {
                runner.update_frame(1.0, viewport, |editor, dispatch, _| {
                    editor.run_doc_command(crate::DocCommand::Undo, dispatch.geometry(1.0));
                    FrameDisposition::Remint
                });
            }
            assert_eq!(runner.editor.model.selection.as_ref().unwrap().path(), path);
            assert!(runner.editor.model.workspace.document.scroll.y > 0.0);
        }
    }

    #[test]
    fn a_new_target_missing_from_the_previous_frame_does_not_trigger_a_retry() {
        let projected = Rc::new(std::cell::Cell::new(0));
        let mut runner = scrolling_runner(&projected);
        let viewport = Size::new(500.0, 400.0);
        runner.refresh_frame(1.0, viewport);
        projected.set(0);
        let content = runner
            .editor
            .model
            .doc
            .root
            .as_ref()
            .unwrap()
            .as_list()
            .unwrap();
        let position = gid::position::between(content.keys().last(), None).unwrap();
        let path = vec![Step::Element(position.clone())];
        let content = Value::List(content.update(position, crate::libraries::f64::value(40.0)));
        assert!(
            !runner
                .frame
                .dispatch
                .descends
                .iter()
                .any(|stop| stop.path.as_ref() == path)
        );
        runner.update_frame(1.0, viewport, |editor, dispatch, _| {
            Rc::make_mut(&mut editor.model.doc).root = Some(content);
            editor.model.selection = Some(crate::selection::Selection::edge(
                editor.model.workspace.document_root(),
                path.clone(),
            ));
            dispatch.geometry(1.0).reveal_selection(editor);
            FrameDisposition::Remint
        });
        assert_eq!(projected.replace(0), 41);
        assert_eq!(runner.editor.model.workspace.document.scroll, Vec2::ZERO);
        let target = runner
            .frame
            .dispatch
            .descends
            .iter()
            .find(|target| target.path.as_ref() == path)
            .unwrap();
        assert!(target.rect.y0 > viewport.height);

        runner.refresh_frame(1.0, viewport);
        assert_eq!(runner.editor.model.workspace.document.scroll, Vec2::ZERO);
        assert_eq!(projected.replace(0), 41);
        runner.update_frame(1.0, viewport, |_, _, _| FrameDisposition::Remint);
        assert_eq!(runner.editor.model.workspace.document.scroll, Vec2::ZERO);
        assert_eq!(projected.get(), 41);
    }

    #[test]
    fn only_transitions_and_changed_frame_inputs_remint() {
        assert_eq!(frame_disposition(false, false), FrameDisposition::Retain);
        assert_eq!(frame_disposition(false, true), FrameDisposition::Remint);
        assert_eq!(frame_disposition(true, false), FrameDisposition::Remint);
        assert_eq!(frame_disposition(true, true), FrameDisposition::Remint);
    }

    #[test]
    fn source_hover_is_immediate_from_code_and_explicit_from_drawing() {
        let code = Hovered::Tree(hover::Hover::Value(Rc::from([])));
        let drawing = Hovered::Tree(hover::Hover::Source(hover::SourceTrace::Stored(Rc::from(
            [],
        ))));

        assert!(source_hover_visible(Some(&code), false));
        assert!(!source_hover_visible(Some(&drawing), false));
        assert!(source_hover_visible(Some(&drawing), true));
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
        let libraries = crate::libraries::Libraries::from_contributions([(
            library_id,
            crate::libraries::Library::<(), ()>::named(
                library_id,
                "source",
                crate::libraries::Definitions::from_parts(
                    doc.cells.clone(),
                    grap::ForeignFunctions::default(),
                ),
                crate::display::partial(|_| None),
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
                scope: Default::default(),
                root: Some(root.clone()),
                path: Rc::from([Step::Follow(source), Step::Key(call)]),
                rect,
                select: Rc::new(move |selected, _| {
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
            let descend =
                projection::source_link::source_descend(&sources, &descends, &trace).unwrap();
            assert_eq!(descend.root, Some(root.clone()));
            assert_eq!(descend.rect, rect);
            let mut selected = None;
            assert!((descend.select)(&mut selected, None));
            assert_eq!(selected, Some(source));
        }
    }

    #[test]
    fn pane_presentations_apply_only_at_entry_and_raw_keeps_the_declaration() {
        use crate::libraries::{Definitions, presentation};
        use std::cell::{Cell, RefCell};

        let projector = CellId::from_u128(1);
        let linked = CellId::from_u128(2);
        let library = CellId::from_u128(3);
        let nested_projector = CellId::from_u128(4);
        let alias = CellId::from_u128(5);
        let source = Value::record([
            (
                presentation::vocabulary::VALUE,
                Value::from(b"source".to_vec()),
            ),
            (
                presentation::vocabulary::PROJECTION,
                Value::from(nested_projector),
            ),
        ]);
        let declaration = Value::record([
            (presentation::vocabulary::VALUE, source.clone()),
            (presentation::vocabulary::PROJECTION, Value::from(projector)),
        ]);
        let mut cells = Cells::new();
        cells.set_value(linked, declaration.clone());
        cells.set_value(alias, Value::from(linked));
        let mut library_cells = Cells::new();
        library_cells.set_value(linked, declaration.clone());
        let mut model = Model::new(Document {
            root: Some(Value::record([])),
            cells,
        });
        for value in [
            declaration,
            Value::from(alias),
            Value::list([source.clone(), Value::from(linked)]),
            Value::record([(presentation::vocabulary::VALUE, source.clone())]),
        ] {
            Rc::make_mut(&mut model.doc).root = Some(
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
        crate::annotations::set_collapsed(
            &mut model.workspace.document.annotations,
            &[Step::Key(workspace::vocabulary::PANES)],
            false,
            false,
        );
        let calls = Rc::new(Cell::new(0));
        let result = Rc::new(RefCell::new(source.clone()));
        let mut stack = stack::load();
        stack.libraries.insert(
            library,
            Definitions::from_parts(
                library_cells,
                grap::ForeignFunctions::default().register(
                    projector,
                    grap::ForeignFunction::from_value({
                        let calls = calls.clone();
                        let result = result.clone();
                        move |context, call, environment| {
                            let value = context
                                .field(call, presentation::vocabulary::VALUE)
                                .unwrap();
                            assert_eq!(context.eval_to_value(value, environment)?, source);
                            calls.set(calls.get() + 1);
                            Ok(result.borrow().clone())
                        }
                    }),
                ).register(
                    nested_projector,
                    grap::ForeignFunction::from_value(|_, _, _| {
                        panic!("nested declarations are ordinary data, including results and absent fallbacks")
                    }),
                ),
            ),
        );
        let styles = crate::styles::editor(crate::styles::Theme::Light.palette(), 1.0);
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
            crate::display::widget::frame::place(
                project_workspace(
                    model,
                    crate::modifiers::native(),
                    true,
                    &crate::computations::Computations::default(),
                    &stack,
                    &styles,
                    &mut tcx,
                    sources::Sources {
                        doc: &model.doc,
                        libraries: &stack.libraries,
                    },
                    size,
                    1.0,
                ),
                Placement::root(Rect::from_origin_size(Point::ZERO, size)),
                &Default::default(),
            )
        };
        let document = model.workspace.document_root();
        let sources: Vec<_> = [
            (0, vec![]),
            (
                1,
                vec![
                    Step::Follow(gid::Resolution::Document),
                    Step::Follow(gid::Resolution::Document),
                ],
            ),
        ]
        .into_iter()
        .map(|(index, steps)| {
            let pane = &model.workspace.left.panes[index];
            let workspace::Target::Pane { path } = pane.view.root.target() else {
                panic!("pane path")
            };
            let mut path = path.clone();
            path.extend(steps);
            (pane.view.root.clone(), path)
        })
        .collect();
        let shown = place(&model);
        assert_eq!(
            calls.replace(0),
            2,
            "only the inline declaration and the selected cell definition at pane entry apply"
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
        *result.borrow_mut() = crate::libraries::absent::with_reason(projector);
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
    fn viewport_panes_receive_their_size_without_margins_or_document_scrolling() {
        use crate::libraries::{Definitions, f64, layout, presentation};
        use std::cell::RefCell;
        let function = gid::new_cell_id();
        let source = Value::from(b"source".to_vec());
        let declaration = Value::record([
            (presentation::vocabulary::VALUE, source.clone()),
            (
                presentation::vocabulary::VIEWPORT,
                grap::lambda(
                    [
                        presentation::vocabulary::VALUE,
                        layout::vocabulary::WIDTH,
                        layout::vocabulary::HEIGHT,
                    ],
                    grap::call(
                        function.into(),
                        [
                            presentation::vocabulary::VALUE,
                            layout::vocabulary::WIDTH,
                            layout::vocabulary::HEIGHT,
                        ]
                        .map(|label| (label, label.into())),
                    ),
                ),
            ),
        ]);
        let mut model = Model::new(Document {
            root: Some(
                workspace::append(
                    &Value::record([]),
                    workspace::Side::Left,
                    declaration.clone(),
                )
                .unwrap()
                .0,
            ),
            cells: Cells::new(),
        });
        model
            .workspace
            .sync_declared(&workspace::declarations(model.doc.root.as_ref()));
        model.workspace.left.panes[0].view.scroll = Vec2::new(75.0, 150.0);
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut stack = stack::load();
        stack.libraries.insert(
            gid::new_cell_id(),
            Definitions::from_parts(
                Cells::new(),
                grap::ForeignFunctions::default().register(
                    function,
                    grap::ForeignFunction::from_value({
                        let calls = calls.clone();
                        move |context, call, environment| {
                            let value = context
                                .field(call, presentation::vocabulary::VALUE)
                                .unwrap();
                            assert_eq!(context.eval_to_value(value, environment)?, source);
                            let width = context.field(call, layout::vocabulary::WIDTH).unwrap();
                            let width =
                                f64::read(&context.eval_to_value(width, environment)?).unwrap();
                            let height = context.field(call, layout::vocabulary::HEIGHT).unwrap();
                            let height =
                                f64::read(&context.eval_to_value(height, environment)?).unwrap();
                            calls.borrow_mut().push(Size::new(width, height));
                            Ok(layout::hoverable(layout::drawing(width, 0.0, height, [])))
                        }
                    }),
                ),
            ),
        );
        let pane = &model.workspace.left.panes[0].view;
        let workspace::Target::Pane { path } = pane.root.target() else {
            panic!("pane")
        };
        for (size, scale) in [
            (Size::new(300.0, 500.0), 1.0),
            (Size::new(700.0, 400.0), 2.0),
        ] {
            let styles = crate::styles::editor(crate::styles::Theme::Light.palette(), scale);
            let mut fonts = FontContext::new();
            let mut layouts = LayoutContext::new();
            let mut cache = puri::text::TextCache::default();
            let mut tcx = TextCtx {
                fonts: &mut fonts,
                layouts: &mut layouts,
                scale: scale as f32,
                cache: &mut cache,
            };
            let mut place = |view, pointer| {
                crate::display::widget::frame::place(
                    project_workspace_view(
                        &model,
                        crate::modifiers::native(),
                        true,
                        &crate::computations::Computations::default(),
                        &stack,
                        &styles,
                        &mut tcx,
                        sources::Sources {
                            doc: &model.doc,
                            libraries: &stack.libraries,
                        },
                        view,
                        size,
                        scale,
                    ),
                    Placement::root(Rect::from_origin_size(Point::new(30.0, 40.0), size)),
                    &placed::HoverInput {
                        pointer,
                        ..Default::default()
                    },
                )
            };
            let placed = place(pane, Some(Point::new(30.5, 40.5)));
            assert_eq!(calls.borrow_mut().pop(), Some(size / scale));
            let rect = Rect::from_origin_size(Point::new(30.0, 40.0), size);
            assert_eq!(placed.view_regions[0].rect, rect);
            assert_eq!(placed.view_regions[0].maximum, Vec2::ZERO);
            assert!(
                placed
                    .descends
                    .iter()
                    .any(|node| node.path.as_ref() == path && node.rect == rect)
            );
            assert!(matches!(
                placed.claim.map(|(_, claim)| claim),
                Some(Claim::Direct(_))
            ));
            assert!(place(pane, Some(Point::new(29.5, 40.5))).claim.is_none());
            assert_eq!(
                model.workspace.left.panes[0].view.scroll,
                Vec2::new(75.0, 150.0)
            );

            let raw = workspace::View {
                root: pane.root.clone(),
                projection: workspace::Projection::Raw,
                annotations: Default::default(),
                scroll: Vec2::ZERO,
            };
            calls.borrow_mut().clear();
            let placed = place(&raw, None);
            assert!(calls.borrow().is_empty(), "Raw never invokes the viewport");
            let field_path: Vec<_> = path
                .iter()
                .cloned()
                .chain([Step::Key(presentation::vocabulary::VIEWPORT)])
                .collect();
            assert!(
                placed
                    .descends
                    .iter()
                    .any(|node| node.path.as_ref() == field_path)
            );
            place(&model.workspace.document, None);
            assert!(
                calls.borrow().is_empty(),
                "the document leaves declarations editable"
            );
        }
    }

    #[test]
    fn workspace_columns_are_editor_geometry_with_independent_view_regions() {
        let cell = CellId::from_u128(1);
        let mut cells = Cells::new();
        cells.set_value(cell, Value::from(b"pane".to_vec()));
        let mut model = Model::new(Document {
            root: Some(Value::record([])),
            cells,
        });
        let document = model.workspace.document_root().clone();
        for _ in 0..2 {
            Rc::make_mut(&mut model.doc).root = Some(
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
        let stack = crate::stack::load();
        let styles = crate::styles::editor(crate::styles::Theme::Light.palette(), 1.0);
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
        let placed = crate::display::widget::frame::place(
            project_workspace(
                &model,
                crate::modifiers::native(),
                true,
                &crate::computations::Computations::default(),
                &stack,
                &styles,
                &mut tcx,
                sources::Sources {
                    doc: &model.doc,
                    libraries: &stack.libraries,
                },
                size,
                1.0,
            ),
            Placement::root(Rect::from_origin_size(Point::ZERO, size)),
            &Default::default(),
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
        let editor = crate::test_editor(gid::Document {
            root: None,
            cells: gid::Cells::new(),
        });
        let target = |index| Hovered::Tree(hover::Hover::Entry(index));
        let viewport = Rect::new(-100.0, -100.0, 100.0, 100.0);
        for (prior, pointer, pressed, covered, expected) in [
            (None, Some(5.0), false, false, Some(target(0))),
            (Some(target(0)), Some(12.0), false, false, Some(target(0))),
            (Some(target(1)), Some(12.0), false, false, Some(target(1))),
            (Some(target(1)), Some(8.0), false, false, Some(target(0))),
            (Some(target(0)), Some(16.0), false, false, Some(target(1))),
            (None, Some(12.0), false, false, None),
            (Some(target(0)), None, false, false, None),
            (Some(target(0)), Some(40.0), true, false, Some(target(0))),
            (None, Some(25.0), false, true, Some(Hovered::Blocked)),
        ] {
            let layout = crate::display::widget::leaf(
                measured::Extent::default(),
                move |output: &mut placed::HoverContext<'_, Editor, Hovered>, _| {
                    output.claim(placed::Probe::retaining(
                        Placement::new(Rect::new(0.0, 0.0, 10.0, 10.0), viewport),
                        target(0),
                    ));
                    output.claim(placed::Probe::retaining(
                        Placement::new(Rect::new(14.0, 0.0, 24.0, 10.0), viewport),
                        target(1),
                    ));
                    if covered {
                        output.claim(placed::Probe::occludes(Placement::new(
                            Rect::new(0.0, 0.0, 40.0, 40.0),
                            viewport,
                        )));
                    }
                },
            );
            assert_eq!(
                hover_frame(
                    &editor,
                    layout,
                    pointer.map(|x| Point::new(x, 5.0)),
                    prior.as_ref().map(|hover| (None, hover)),
                    pressed
                )
                .hover,
                expected
            );
        }
    }

    #[test]
    fn exact_hover_claims_do_not_retain_outside_their_hit_geometry() {
        let editor = crate::test_editor(gid::Document {
            root: None,
            cells: gid::Cells::new(),
        });
        let target = Hovered::Divider(workspace::Divider::Columns(workspace::Side::Left));
        for (x, pressed, expected) in [
            (5.0, false, Some(target.clone())),
            (11.0, false, None),
            (11.0, true, Some(target.clone())),
        ] {
            let claimed = target.clone();
            let layout = crate::display::widget::leaf(
                measured::Extent::default(),
                move |output: &mut placed::HoverContext<'_, Editor, Hovered>, _| {
                    output.claim(placed::Probe::exact(
                        Placement::root(Rect::new(0.0, 0.0, 10.0, 10.0)),
                        claimed,
                    ));
                },
            );
            assert_eq!(
                hover_frame(
                    &editor,
                    layout,
                    Some(Point::new(x, 5.0)),
                    Some((None, &target)),
                    pressed
                )
                .hover,
                expected
            );
        }
    }

    fn hover_frame(
        editor: &Editor,
        layout: measured::Measured<HoverPass<Editor>>,
        position: Option<Point>,
        previous: Option<(Option<&Root>, &Hovered)>,
        pressed: bool,
    ) -> Frame {
        compute_hover(
            layout,
            &FrameDescription {
                palette: editor.palette,
                command_modifier: crate::modifiers::native(),
                computations: &editor.computations,
                focused: editor.focused,
                drawn_menu: false,
                model: &editor.model,
                stack: &editor.stack,
                menu: menu::State::default(),
                availability: editor.menu_availability(),
                toggles: editor.menu_toggles(),
                scale: 1.0,
                viewport: Size::new(100.0, 100.0),
            },
            PointerInput {
                position,
                previous,
                pressed,
                link_sources: false,
            },
        )
    }

    #[test]
    fn building_a_frame_leaves_installed_hover_and_dispatch_unchanged() {
        let mut runner = EditorRunner::new(crate::test_editor(gid::Document {
            root: None,
            cells: gid::Cells::new(),
        }));
        let prior = Hovered::Tree(hover::Hover::Entry(7));
        runner.frame.hover = Some(prior.clone());
        runner.frame.dispatch.line = 42.0;
        let frame =
            runner
                .editor
                .build_frame(1.0, Size::new(300.0, 200.0), runner.frame.hover_location());
        assert_eq!(runner.frame.hover, Some(prior));
        assert_eq!(runner.frame.dispatch.line, 42.0);
        assert!(runner.frame.dispatch.descends.is_empty());
        assert_eq!(frame.hover, None);
        let line = frame.dispatch.line;
        let descends = frame.dispatch.descends.clone();
        let paint = runner.install_frame(frame, 1.0, Size::new(300.0, 200.0));
        assert_eq!(runner.frame.hover, None);
        assert_eq!(runner.frame.dispatch.line, line);
        assert!(Rc::ptr_eq(&runner.frame.dispatch.descends, &descends));
        drop(paint);
    }

    #[test]
    fn pressed_hover_keeps_its_owner_and_skips_probes_until_release() {
        let editor = crate::test_editor(gid::Document {
            root: None,
            cells: gid::Cells::new(),
        });
        let root = editor.model.workspace.document_root().clone();
        let prior_root = Root::pane(vec![Step::Key(gid::new_cell_id())]);
        let prior = Hovered::Tree(hover::Hover::Entry(7));
        for pressed in [true, false] {
            let calls = Rc::new(std::cell::Cell::new(0));
            let counted = calls.clone();
            let layout = placed::in_view(
                crate::display::widget::leaf(
                    measured::Extent::default(),
                    move |output: &mut placed::HoverContext<'_, Editor, Hovered>, _| {
                        output.claim(placed::Probe::dynamic(
                            Placement::root(Rect::new(0.0, 0.0, 10.0, 10.0)),
                            move |_| {
                                counted.set(counted.get() + 1);
                                Some(Hovered::Blocked)
                            },
                        ));
                    },
                ),
                root.clone(),
            );
            let frame = hover_frame(
                &editor,
                layout,
                Some(Point::new(5.0, 5.0)),
                Some((Some(&prior_root), &prior)),
                pressed,
            );
            assert_eq!(calls.get(), usize::from(!pressed));
            assert_eq!(
                frame.hover,
                Some(if pressed {
                    prior.clone()
                } else {
                    Hovered::Blocked
                })
            );
            assert_eq!(
                frame.dispatch.pointer_root,
                Some(if pressed {
                    prior_root.clone()
                } else {
                    root.clone()
                })
            );
            let mut runner = EditorRunner::new(crate::test_editor(gid::Document {
                root: None,
                cells: gid::Cells::new(),
            }));
            runner.editor.pointer = Some(Point::new(6.0, 5.0));
            runner.editor.pressed = pressed;
            drop(runner.install_frame(frame, 1.0, Size::new(100.0, 100.0)));
            calls.set(0);
            runner.probe_pointer(1.0);
            assert_eq!(calls.get(), usize::from(!pressed));
            assert_eq!(
                runner.frame.hover,
                Some(if pressed {
                    prior.clone()
                } else {
                    Hovered::Blocked
                })
            );
            assert_eq!(
                runner.frame.dispatch.pointer_root,
                Some(if pressed {
                    prior_root.clone()
                } else {
                    root.clone()
                })
            );
            // Releasing allows the current geometry to choose a new owner.
            runner.editor.pressed = false;
            runner.probe_pointer(1.0);
            assert_eq!(runner.frame.hover, Some(Hovered::Blocked));
            assert_eq!(runner.frame.dispatch.pointer_root, Some(root.clone()));
        }
    }
}
