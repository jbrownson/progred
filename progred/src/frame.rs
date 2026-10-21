//! Project → compute hover → bind paint and dispatch.
//! Paint stays latent in the returned frame; rendering it is the
//! caller's choice, so a silent mint never draws.

use crate::display::widget::frame::{Cursor, ViewRegion};
use crate::display::widget::navigation::{Geometry, Landmark, landmark_at};
use crate::display::widget::source::SourceTrace;
use crate::display::widget::view::{Root, Target};
use crate::display::widget::{container, placed, style, view};
use crate::hover;
use crate::menu;
use crate::model::Model;
use crate::navigate;
use crate::projection;
use crate::sources;
use crate::stack;
use crate::workspace;
use crate::{Editor, EditorRunner};
use kurbo::{Affine, Insets, Point, Rect, Size, Vec2};
use parley::{FontContext, LayoutContext};
use peniko::Brush;
use puri::draw::Canvas;
use puri::frame::Render;
use puri::handler::{Event, Handler, HasHandler, ScrollOutcome};
use puri::hover::Claim;
use puri::interact::is_primary_contact;
use puri::text::TextCtx;
use std::borrow::Cow;
use std::rc::Rc;
use uig::Placement;

pub(crate) const HOVER_REACH_POINTS: f64 = 8.0;

#[derive(Default)]
pub(crate) struct Dispatch {
    pub(crate) handler: Rc<Handler<Editor, DispatchContext<Editor>>>,
    pub(crate) pointer_root: Option<crate::display::widget::view::Root>,
    pub(crate) landmarks: Rc<[Landmark<Editor>]>,
    pub(crate) view_regions: Rc<[ViewRegion]>,
    pub(crate) hover_geometry: crate::display::widget::frame::HoverGeometry<Hovered>,
}

/// A completed frame. Installing it retains hover and dispatch together;
/// its paint continuations can be run separately or discarded.
pub(crate) struct Frame {
    pub(crate) scroll_probes: Vec<crate::display::widget::scroll::Probe>,
    pub(crate) hover: Option<Hovered>,
    pub(crate) dispatch: Dispatch,
    pub(crate) renders: Vec<Render>,
    pub(crate) input_area: Option<Rect>,
}

/// An installed frame's paint, waiting to be presented at the scale and
/// viewport it was laid out for.
pub(crate) struct PendingPaint {
    pub(crate) scale: f64,
    pub(crate) viewport: Size,
    pub(crate) renders: Vec<Render>,
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
    pub(crate) fn geometry(&self, scale: f64) -> Geometry<'_, Editor> {
        Geometry {
            landmarks: &self.landmarks,
            view_regions: &self.view_regions,
            scale,
        }
    }

    /// Dispatch `event` to `editor`, `hover` resting under its pointer;
    /// whether a handler took it.
    pub(crate) fn send(
        &self,
        editor: &mut Editor,
        hover: Option<&Hovered>,
        event: Event<'_>,
    ) -> bool {
        self.handler
            .dispatch(editor, event, &mut self.context(editor, hover.cloned()))
            .handled()
    }

    /// The handler is shared only while a dispatch context is alive.
    pub(crate) fn handler_mut(&mut self) -> &mut Handler<Editor, DispatchContext<Editor>> {
        Rc::get_mut(&mut self.handler).expect("no dispatch is in progress")
    }

    /// The context a dispatch to `editor` runs in, `hover` resting under its
    /// pointer.
    fn context(&self, editor: &Editor, hover: Option<Hovered>) -> DispatchContext<Editor> {
        DispatchContext {
            handler: self.handler.clone(),
            landmarks: self.landmarks.clone(),
            view_regions: self.view_regions.clone(),
            pointer: editor.pointer,
            modifiers: editor.modifiers,
            pressed: editor.pressed,
            ..DispatchContext::new(self.pointer_root.clone(), hover)
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

/// The toolkit's frame types over what the editor's pointer rests on.
pub(crate) type HoverPass<C> = crate::display::widget::HoverPass<C, Hovered>;
pub(crate) type DispatchContext<C> = crate::display::widget::frame::DispatchContext<C, Hovered>;
pub(crate) type Attribution = crate::display::widget::frame::Attribution<Hovered>;

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
    landmarks: &[Landmark<Editor>],
    root: Option<&Root>,
    completion: Option<&crate::display::widget::offers::Offers<Editor>>,
    hovered: Option<Hovered>,
    link: bool,
) -> Attribution {
    // Keep the probe's call chain for subsequent input, but use the selected
    // source for this frame's ordinary source/secondary decoration.
    let hovered = match hovered {
        Some(Hovered::Tree(ref calls @ hover::Hover::Calls(_))) => {
            crate::display::widget::source_link::hover_source(sources, landmarks, calls)
                .map(|source| Hovered::Tree(hover::Hover::Source(source)))
        }
        other => other,
    };
    let visible = source_hover_visible(hovered.as_ref(), link);
    let landmark = match &hovered {
        Some(Hovered::Tree(hover::Hover::Value(path))) => landmark_at(landmarks, root, path),
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
            .and_then(|d| d.path.location())
            .map(|location| SourceTrace::from_location(sources, &location)),
        Some(Hovered::Tree(hover::Hover::Source(source))) if visible => Some(source.clone()),
        _ => None,
    };
    Attribution {
        hovered,
        hovered_secondary,
        selected_secondary: crate::display::widget::navigation::selected_secondary(landmarks),
        hovered_trace,
    }
}

/// Interpret the winning claim as an editor target and its owning view.
fn hover_target(
    claim: Option<(Option<crate::display::widget::view::Root>, Claim<Hovered>)>,
) -> (Option<Hovered>, Option<crate::display::widget::view::Root>) {
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
    pub palette: style::Palette,
    pub command_modifier: puri::keyboard::CommandModifier,
    computations: &'a crate::computations::Computations,
    focused: bool,
    drawn_menu: bool,
    model: &'a Model,
    stack: &'a stack::Stack,
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

#[derive(Default)]
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
            |editor: &mut Editor, event, _: &mut DispatchContext<Editor>| {
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
        &crate::display::widget::HoverInput {
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
        &output.landmarks,
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
            &output.landmarks,
        )
    {
        output.handler_mut().stack(handler);
    }
    let crate::display::widget::frame::FrameOutput {
        scroll_probes,
        renders,
        handler,
        landmarks,
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
            landmarks: landmarks.into(),
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
        maximum: Vec2,
    ) -> ScrollOutcome {
        // A retained handler can outlive its view; decline rather than panic.
        match self.model.workspace.view_mut(&root) {
            Some(view) => {
                let (next, outcome) = crate::display::widget::scroll::offset(
                    view.scroll,
                    update,
                    scale,
                    viewport,
                    maximum,
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
        let pointer = PointerInput {
            position: self.pointer,
            previous: previous
                .as_ref()
                .map(|(root, hover)| (root.as_ref(), hover)),
            pressed: self.pressed,
            link_sources: self.command_modifier.pressed(&self.modifiers),
        };
        self.frame_with(scale, viewport, pointer)
    }

    /// The frame as it draws with nothing selected and no pointer over it,
    /// as a picture of part of it shows.
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub(crate) fn unmarked_frame(&mut self, scale: f64, viewport: Size) -> Frame {
        let selection = self.model.selection.take();
        // A held button stays as it is: the computations read it.
        let pointer = PointerInput {
            pressed: self.pressed,
            ..PointerInput::default()
        };
        let frame = self.frame_with(scale, viewport, pointer);
        self.model.selection = selection;
        frame
    }

    fn frame_with(&mut self, scale: f64, viewport: Size, pointer: PointerInput<'_>) -> Frame {
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
            pointer,
        )
    }
}

impl EditorRunner {
    pub(crate) fn probe_pointer(&mut self, scale: f64) {
        // Retain the complete hover location throughout an active press.
        if !self.editor.pressed {
            let (hover, pointer_root) = hover_target(self.frame.dispatch.hover_geometry.probe(
                self.editor.pointer,
                self.frame.hover.as_ref(),
                HOVER_REACH_POINTS * scale,
            ));
            self.frame.hover = hover;
            self.frame.dispatch.pointer_root = pointer_root;
        }
    }

    /// Handle an event with the installed frame. `handle_event` answers
    /// whether the frame it shows changed, which mints its successor;
    /// whether it did.
    pub(crate) fn update_frame(
        &mut self,
        scale: f64,
        viewport: Size,
        handle_event: impl FnOnce(&mut Editor, &Dispatch, Option<&Hovered>) -> bool,
    ) -> bool {
        let remint = handle_event(
            &mut self.editor,
            &self.frame.dispatch,
            self.frame.hover.as_ref(),
        );
        if remint {
            self.refresh_frame(scale, viewport);
        }
        remint
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
            let handled = self.frame.dispatch.send(
                &mut self.editor,
                self.frame.hover.as_ref(),
                Event::HoverChanged,
            );
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

/// What `view` draws with: the libraries' projections less those the view
/// leaves off, as a pane's when `pane`.
fn view_projection<'a>(
    stack: &'a stack::Stack,
    view: &workspace::View,
    pane: bool,
) -> Cow<'a, projection::Projection> {
    match &*hidden(stack, view) {
        [] => Cow::Borrowed(if pane {
            &stack.pane_projection
        } else {
            &stack.projection
        }),
        hidden => {
            let (document, pane_projection) = stack.without(hidden);
            Cow::Owned(if pane { pane_projection } else { document })
        }
    }
}

/// The libraries switched off in `view`.
pub(crate) fn hidden<'a>(
    stack: &stack::Stack,
    view: &'a workspace::View,
) -> Cow<'a, [gid::CellId]> {
    stack.hidden(&view.shown)
}

/// What `view` offers: none of what its libraries switched off offer.
pub(crate) fn completions<'a>(
    stack: &'a stack::Stack,
    view: &workspace::View,
) -> Cow<'a, crate::display::CompletionProvider> {
    match &*hidden(stack, view) {
        [] => Cow::Borrowed(&stack.completions),
        hidden => Cow::Owned(stack.completions_without(hidden)),
    }
}

/// What a view shows: the path of its root and the value there, the
/// projection drawing them, and whether that's a viewport filling its pane,
/// drawn without a margin or scrolling.
struct ViewContent<'a> {
    /// Where the view's root is drawn, which says where its data is: a
    /// computed root, such as a place that can't be found, has none.
    root_path: crate::occurrence::Path,
    root: Option<Cow<'a, gid::Value>>,
    projection: Cow<'a, projection::Projection>,
    viewport: bool,
}

/// What `view` shows in an area of `size` at `scale`: a pane whose value is
/// a viewport fills the pane with it, while it shows the presentation
/// library.
fn view_content<'a>(
    stack: &'a stack::Stack,
    sources: sources::Sources<'a>,
    view: &'a workspace::View,
    size: Size,
    scale: f64,
) -> ViewContent<'a> {
    let viewport = match view.root.target() {
        Target::Pane { path } if view.shown.shows(crate::libraries::presentation::ID) => {
            projection::viewport::entry(sources, path)
        }
        _ => None,
    };
    let interpreted = view.shown.shows(crate::libraries::editor::ID);
    let root_path = workspace::root_path(&sources, view.root.target(), interpreted);
    match (view.root.target(), viewport) {
        (Target::Pane { path }, Some(entry)) => ViewContent {
            // The cells the entry follows to its viewport, below the jump.
            root_path: crate::occurrence::below(&root_path, &entry.path[path.len()..]),
            root: Some(Cow::Borrowed(entry.value)),
            projection: Cow::Owned(projection::viewport::projection(
                &view_projection(stack, view, false),
                size / scale,
            )),
            viewport: true,
        },
        (target, _) => {
            let root = crate::occurrence::read(&sources, &root_path);
            let not_found = root
                .is_none()
                .then(|| not_found(sources, target, interpreted, &root_path))
                .flatten();
            let mut drawn = root_path;
            if not_found.is_some() {
                drawn.push(crate::occurrence::Step::Computed);
            }
            ViewContent {
                root: not_found.map(Cow::Owned).or(root.map(Cow::Borrowed)),
                root_path: drawn,
                projection: view_projection(stack, view, matches!(target, Target::Pane { .. })),
                viewport: false,
            }
        }
    }
}

/// What a pane showing a location draws when its place, where `root_path`
/// jumps, can't be reached, as after its container was deleted: an absent
/// naming it. A place that's only empty draws as any empty place does, ready
/// to write.
fn not_found(
    sources: sources::Sources<'_>,
    target: &Target,
    interpreted: bool,
    root_path: &[crate::occurrence::Step],
) -> Option<gid::Value> {
    let Target::Pane { path } = target else {
        return None;
    };
    let location = interpreted
        .then(|| crate::location::read(sources.resolve_path(path)?))
        .flatten()?;
    let place = crate::occurrence::location(root_path)?;
    (!crate::location::reachable(&sources, &place)).then(|| crate::location::not_found(&location))
}

/// Whether `view` labels cells with their names: only with the name library
/// on.
pub(crate) fn names(stack: &stack::Stack, view: &workspace::View) -> bool {
    !hidden(stack, view).contains(&crate::libraries::name::ID)
}

/// The margin around a view's content: none for a viewport filling its pane.
fn content_margin(viewport: bool, scale: f64) -> f64 {
    if viewport { 0.0 } else { 12.0 * scale }
}

/// An empty leaf filling `size`, for overlays to place into.
fn backdrop(size: Size) -> measured::Measured<HoverPass<Editor>> {
    placed::leaf(
        measured::Extent {
            width: size.width,
            ascent: 0.0,
            descent: size.height,
        },
        |_, _| {},
    )
}

fn project_workspace_view(
    description: &FrameDescription<'_>,
    styles: &style::Styles,
    tcx: &mut TextCtx,
    view: &workspace::View,
    size: Size,
) -> measured::Measured<HoverPass<Editor>> {
    let FrameDescription {
        computations,
        focused,
        model,
        stack,
        scale,
        ..
    } = *description;
    let sources = sources::Sources {
        doc: &model.doc,
        libraries: &stack.libraries,
    };
    let shown = view_content(stack, sources, view, size, scale);
    let margin = content_margin(shown.viewport, scale);
    let completions = completions(stack, view);
    let projected = projection::project(
        projection::ProjectDescription {
            computations,
            focused,
            completions: Some(&completions),
            sources,
            root: shown.root.as_deref(),
            root_path: &shown.root_path,
            selection: model
                .selection
                .as_ref()
                .filter(|selection| selection.root() == &view.root),
            source_selection: model.selection.as_ref(),
            annotations: &view.annotations,
            names: names(stack, view),
            styles,
            width: (size.width - 2.0 * margin).max(0.0),
            projection: &shown.projection,
        },
        tcx,
    );
    let content = measured::pad(Insets::uniform(margin), projected);
    let root = view.root.clone();
    let content = if shown.viewport {
        view::viewport(content, root.clone())
    } else {
        let maximum = crate::display::widget::scroll::maximum(content.extent, size, scale);
        let offset = Vec2::new(
            view.scroll.x.clamp(0.0, maximum.x) * scale,
            view.scroll.y.clamp(0.0, maximum.y) * scale,
        );
        let scroll_root = root.clone();
        view::scrolled_at(
            content,
            offset,
            root.clone(),
            scale,
            move |app: &mut Editor, update| {
                app.scroll_view(scroll_root.clone(), update, scale, size, maximum)
            },
        )
    };
    let content = view::in_view(content, root);
    measured::overlay_into(backdrop(size), content, move |placement, _| {
        Some(Placement::new(
            placement.rect,
            placement.clip_rect.intersect(placement.rect),
        ))
    })
}

/// The rule between two columns or panes, which drags to resize them.
fn divider_rule(
    divider: workspace::Divider,
    rect: Rect,
    border: peniko::Color,
    scale: f64,
    size: Size,
) -> measured::Measured<HoverPass<Editor>> {
    let vertical = matches!(divider, workspace::Divider::Columns(_));
    let hover_divider = divider.clone();
    let cursor_divider = divider.clone();
    let down_divider = divider.clone();
    let move_divider = divider.clone();
    let up_divider = divider;
    placed::leaf(
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
                    Cursor::ColumnResize
                } else {
                    Cursor::RowResize
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
    )
}

fn project_workspace(
    description: &FrameDescription<'_>,
    styles: &style::Styles,
    tcx: &mut TextCtx,
    size: Size,
) -> measured::Measured<HoverPass<Editor>> {
    let geometry = description
        .model
        .workspace
        .geometry(size, description.scale);
    let views = geometry
        .views
        .into_iter()
        .fold(backdrop(size), |body, placed_view| {
            let rect = placed_view.rect;
            let child =
                project_workspace_view(description, styles, tcx, placed_view.view, rect.size());
            measured::overlay_into(body, child, move |placement, _| {
                let rect = rect + placement.rect.origin().to_vec2();
                Some(Placement::new(rect, placement.clip_rect.intersect(rect)))
            })
        });
    geometry
        .dividers
        .into_iter()
        .fold(views, |body, placed_divider| {
            let rect = placed_divider.rect;
            let rule = divider_rule(
                placed_divider.divider,
                rect,
                styles.palette.border,
                description.scale,
                size,
            );
            measured::overlay_into(body, rule, move |placement, _| {
                let rect = rect + placement.rect.origin().to_vec2();
                // The rule is inside the workspace, but its retained drag
                // needs the workspace origin after the pointer leaves the
                // narrow hit target.
                Some(Placement::new(rect, placement.clip_rect))
            })
        })
}

/// What the workspace fills: the viewport below the drawn menu bar, if any.
fn content_viewport(drawn_menu: bool, viewport: Size, scale: f64) -> Rect {
    let menu_height = if drawn_menu {
        menu::bar_height(scale)
    } else {
        0.0
    };
    Rect::new(
        0.0,
        menu_height.min(viewport.height),
        viewport.width,
        viewport.height,
    )
}

fn project_frame(
    description: &FrameDescription<'_>,
    resources: FrameResources<'_>,
) -> (
    measured::Measured<HoverPass<Editor>>,
    Handler<Editor, DispatchContext<Editor>>,
) {
    let FrameDescription {
        palette,
        command_modifier,
        computations,
        drawn_menu,
        ref toggles,
        model,
        stack,
        menu,
        availability,
        scale,
        viewport,
        ..
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
    let styles = style::editor(palette, scale);
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
                    .switchable()
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
    let body = project_workspace(description, &styles, &mut tcx, content_viewport.size());
    // The stage is editor chrome around one workspace tree. Empty
    // space deselection remains the shell's final editor action.
    let stage = match menu_bar {
        Some(bar) => {
            let bar_placement = Placement::new(
                Rect::new(0.0, 0.0, viewport_width, content_viewport.y0),
                Rect::new(0.0, 0.0, viewport_width, viewport.height),
            );
            measured::overlay_into(backdrop(viewport), bar, move |_, _| Some(bar_placement))
        }
        None => backdrop(viewport),
    };
    let stage = measured::overlay_into(stage, body, move |_, _| {
        Some(Placement::new(content_viewport, content_viewport))
    });
    let stage = match menu_popup {
        Some((x, popup)) => {
            // Outside presses close the popup. Its own Occludes claim
            // becomes Hovered::Blocked over separators; command items resolve
            // their semantic target above it, so no raw inside-swallow may
            // preempt them.
            let popup = placed::before(popup, move |p, placement| {
                let rect = placement.rect;
                let headings = Rect::new(0.0, 0.0, menu_heading_width, content_viewport.y0);
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
            container::floating(stage, popup, move |placement, extent| {
                let x = x
                    .min(placement.clip_rect.x1 - extent.width)
                    .max(placement.clip_rect.x0);
                Some(Placement::new(
                    extent.rect_at(Point::new(x, content_viewport.y0)),
                    placement.clip_rect,
                ))
            })
        }
        None => stage,
    };
    (stage, keyboard)
}

#[cfg(test)]
mod placement_tests;
#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;
