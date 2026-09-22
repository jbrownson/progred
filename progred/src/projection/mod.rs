//! The editor's projection runtime: resolve locations, supply borrowed widget inputs,
//! retain source provenance, and fall back to total structural display.

pub(crate) mod completion;
pub(crate) mod drawing;
pub(crate) mod line_control;
mod location;
pub(crate) mod source_link;
mod structure;
#[cfg(test)]
mod tests;
pub(crate) mod viewport;

use crate::annotations::Annotations;
use crate::display::widget::style::{face_style, highlight_outline, hover_highlight};
use crate::frame::Hovered;
use crate::hover::{Hover, Secondary, SourceTrace};
#[cfg(test)]
use crate::navigate::Descend;
use crate::placed::{self, HoverPass, before, decorate};
use crate::render;
use crate::selection::{Selection, Stage, last_follow};
use crate::sources::Sources;
use crate::styles::Styles;
use completion::pending_view;
use gid::{CellId, Path, Step, Value};
#[cfg(test)]
use kurbo::Point;
use kurbo::{Affine, RoundedRect, Stroke};
use measured::Measured;
use measured::choices::{ChoiceBuild, ChoiceGraph, ChoiceLayout, resolve_choices};
use peniko::Color;
use puri::draw::Canvas;
use puri::edit::{LineEditDescription, LineEditPresentation, LineEditState};
use puri::geometry::Placement;
use puri::handler::HasHandler;
use puri::text::{TextCtx, TextStyle};
use std::collections::HashSet;
use std::rc::Rc;
use ui_events::keyboard::{Key, NamedKey};

type SharedPath = Rc<[Step]>;

/// One ordered composition of partial value projections. The
/// structural fallback lives in this runtime and is always total.
pub struct Projection<World> {
    partial: crate::display::Partial<World, Hovered>,
    entry: Option<crate::display::Partial<World, Hovered>>,
}

impl<World> Clone for Projection<World> {
    fn clone(&self) -> Self {
        Self {
            partial: self.partial.clone(),
            entry: self.entry.clone(),
        }
    }
}

impl<World: 'static> Default for Projection<World> {
    fn default() -> Self {
        Self {
            partial: crate::display::partial(|_| None),
            entry: None,
        }
    }
}

impl<World: 'static> Projection<World> {
    pub fn new(
        partials: impl IntoIterator<Item = crate::display::Partial<World, Hovered>>,
    ) -> Self {
        Self {
            partial: crate::display::compose_partials(partials),
            entry: None,
        }
    }

    /// Prepend a partial at entry, following cells to their definitions.
    /// Its children and computed results use the ordinary projection.
    pub fn with_entry(self, partial: crate::display::Partial<World, Hovered>) -> Self {
        Self {
            entry: Some(partial),
            ..self
        }
    }

    fn without_entry(&self) -> Self {
        Self {
            partial: self.partial.clone(),
            entry: None,
        }
    }

    fn apply(
        &self,
        input: &crate::display::ProjectionInput<'_, World, Hovered>,
    ) -> Option<crate::display::Layout<World, Hovered>> {
        self.entry
            .as_ref()
            .and_then(|entry| entry(input))
            .or_else(|| (self.partial)(input))
    }
}

/// Read-only projection context threaded through every view.
#[derive(Clone)]
pub(crate) struct Cx<'a> {
    pub(crate) command_modifier: puri::keyboard::CommandModifier,
    pub(crate) computations: Option<&'a crate::computations::Computations>,
    pub(crate) focused: bool,
    pub(crate) view: &'a crate::workspace::Root,
    pub(crate) completions: Option<&'a crate::display::CompletionProvider>,
    /// The reading context: the document read over its library.
    pub(crate) sources: Sources<'a>,
    pub(crate) edits: crate::editing::Scope,
    /// Names and field order derive from this view bit. Value
    /// projections come from the editor's stack; `grap` is one of them.
    pub(crate) raw: bool,
    pub(crate) annotations: &'a Annotations,
    pub(crate) styles: &'a Styles,
    pub(crate) selection: Option<&'a Selection>,
    /// The selected cell-relative location whose other projections
    /// carry the secondary mark.
    pub(crate) secondary: Option<Secondary>,
    /// The selected structural source, normalized across projections
    /// for execution-linked output.
    pub(crate) selected_trace: Option<SourceTrace>,
}

#[derive(Clone, Default)]
struct Ancestry {
    /// Cells crossed by `Follow`, used to stop projection cycles.
    cells: HashSet<CellId>,
    /// The nearest followed definition and the start of its relative path.
    enclosing: Option<(CellId, gid::Resolution, usize)>,
}

struct ProjectEnv<'a, 's> {
    cx: &'a Cx<'s>,
    path: &'a [Step],
}

impl crate::display::Env for ProjectEnv<'_, '_> {
    fn apply_scoped(
        &self,
        function: &Value,
        arguments: &[(CellId, Value)],
        scope: Option<&grap::ForeignOverlay<'_>>,
    ) -> grap::Evaluation {
        self.cx.sources.apply_scoped(function, arguments, scope)
    }

    fn evaluate(&self, expression: &Value) -> Value {
        self.cx.sources.evaluate(expression)
    }

    fn evaluate_with_fuel(&self, expression: &Value, fuel: usize) -> Value {
        self.cx.sources.evaluate_with_fuel(expression, fuel)
    }

    fn evaluate_memo(&self, expression: &Value, fuel: usize) -> Value {
        match self.cx.computations {
            Some(computations) => computations.evaluate(self.cx.view, self.path, expression, fuel),
            None => self.evaluate_with_fuel(expression, fuel),
        }
    }

    fn apply_memo(&self, function: &Value, arguments: &[(CellId, Value)], fuel: usize) -> Value {
        match self.cx.computations {
            Some(computations) => {
                computations.apply(self.cx.view, self.path, function, arguments, fuel)
            }
            None => grap::apply(function, arguments.iter().cloned(), &self.cx.sources, fuel).result,
        }
    }

    fn name(&self, cell: CellId) -> Option<&str> {
        self.cx.name(cell)
    }

    fn resolve(&self, cell: CellId) -> Option<crate::display::ResolvedCell<'_>> {
        self.cx.sources.definition(cell)
    }
}

/// Prepare a projection with its explicit source and current widget inputs.
/// Box composition itself belongs to the display measurement interpreter.
#[allow(clippy::too_many_arguments)]
fn prepare(
    cx: &Cx,
    projection: &Projection<crate::Editor>,
    tcx: &mut TextCtx,
    path: &[Step],
    ancestors: &Ancestry,
    value: Option<&Value>,
    layout: crate::display::Layout<crate::Editor, Hovered>,
    build: &mut ChoiceBuild<HoverPass<crate::Editor>>,
) -> ChoiceLayout<HoverPass<crate::Editor>> {
    let project = ProjectionScope {
        cx,
        projection,
        path,
        ancestors,
        value,
    };
    layout.measure(
        &mut crate::display::widget::Context {
            project: &project,
            text: tcx,
            inputs: cx,
            path,
            value,
        },
        build,
    )
}

struct ProjectionScope<'a, 's> {
    cx: &'a Cx<'s>,
    projection: &'a Projection<crate::Editor>,
    path: &'a [Step],
    ancestors: &'a Ancestry,
    value: Option<&'a Value>,
}

impl crate::display::widget::project::Project<crate::Editor, Hovered> for ProjectionScope<'_, '_> {
    fn descend(
        &self,
        text: &mut TextCtx,
        build: &mut ChoiceBuild<HoverPass<crate::Editor>>,
        steps: &[Step],
        current: Option<crate::display::Partial<crate::Editor, Hovered>>,
        default: Option<crate::display::Partial<crate::Editor, Hovered>>,
    ) -> ChoiceLayout<HoverPass<crate::Editor>> {
        prepare_descend_path(
            self.cx,
            self.projection,
            text,
            self.path,
            self.ancestors,
            self.value,
            steps,
            current,
            default,
            build,
        )
    }
    fn jump(
        &self,
        text: &mut TextCtx,
        build: &mut ChoiceBuild<HoverPass<crate::Editor>>,
        steps: Vec<Step>,
        document: Vec<Step>,
        conject: crate::display::Conject,
        current: Option<crate::display::Partial<crate::Editor, Hovered>>,
        default: Option<crate::display::Partial<crate::Editor, Hovered>>,
    ) -> ChoiceLayout<HoverPass<crate::Editor>> {
        let path: Vec<_> = self.path.iter().cloned().chain(steps).collect();
        let cx = Cx {
            edits: self.cx.edits.with_conject(path.clone(), document, conject),
            ..self.cx.clone()
        };
        let value = cx.edits.read(&cx.sources, &path);
        let mut ancestry = self.ancestors.clone();
        // Following the same source through jumps must not bypass cycle checks.
        if let Some(source) = cx.edits.source(&path) {
            for (i, step) in source.iter().enumerate() {
                if matches!(step, Step::Follow(_)) {
                    if let Some(cell) = cx
                        .sources
                        .resolve_path(&source[..i])
                        .and_then(Value::as_cell)
                    {
                        ancestry.cells.insert(cell);
                    }
                }
            }
        }
        prepare_value(
            &cx,
            self.projection,
            current.as_ref(),
            default.as_ref(),
            text,
            &path,
            &ancestry,
            value,
            build,
        )
    }
    fn at(
        &self,
        text: &mut TextCtx,
        build: &mut ChoiceBuild<HoverPass<crate::Editor>>,
        steps: Vec<Step>,
        value: Value,
        current: Option<crate::display::Partial<crate::Editor, Hovered>>,
        default: Option<crate::display::Partial<crate::Editor, Hovered>>,
    ) -> ChoiceLayout<HoverPass<crate::Editor>> {
        prepare_at(
            self.cx,
            self.projection,
            text,
            self.path,
            steps,
            value,
            current,
            default,
            build,
        )
    }
}

fn prepare_at(
    cx: &Cx,
    projection: &Projection<crate::Editor>,
    tcx: &mut TextCtx,
    path: &[Step],
    steps: Vec<Step>,
    nested: Value,
    current_projection: Option<crate::display::Partial<crate::Editor, Hovered>>,
    default_projection: Option<crate::display::Partial<crate::Editor, Hovered>>,
    build: &mut ChoiceBuild<HoverPass<crate::Editor>>,
) -> ChoiceLayout<HoverPass<crate::Editor>> {
    let path: Vec<_> = path.iter().cloned().chain(steps).collect();
    let cx = Cx {
        edits: cx.edits.detached(path.clone()),
        ..cx.clone()
    };
    prepare_value(
        &cx,
        projection,
        current_projection.as_ref(),
        default_projection.as_ref(),
        tcx,
        &path,
        &Ancestry::default(),
        Some(&nested),
        build,
    )
}

/// The resolved hover's tree identity, for ink that lights its own
/// claim.
fn tree_hovered(hover: &placed::ResolvedHover) -> Option<&Hover> {
    match hover.hovered.as_ref() {
        Some(Hovered::Tree(hover)) => Some(hover),
        _ => None,
    }
}

pub(crate) fn select_handler(
    path: SharedPath,
    cx: &Cx,
) -> crate::display::ActionHandler<crate::Editor> {
    let root = cx.view.clone();
    let destination = path;
    let edits = cx.edits.clone();
    Rc::new(move |world| {
        edits
            .open(crate::editing::Access::new(world))
            .select(&root, &destination);
        true
    })
}

fn navigation_select_handler(path: SharedPath, cx: &Cx) -> crate::navigate::Select<crate::Editor> {
    let root = cx.view.clone();
    let destination = path;
    let edits = cx.edits.clone();
    Rc::new(move |world, _| {
        edits
            .open(crate::editing::Access::new(world))
            .select(&root, &destination);
        true
    })
}

fn projection_target(
    cx: &Cx,
    path: &[Step],
    steps: Vec<Step>,
) -> crate::display::ProjectionTarget<crate::Editor, Hovered> {
    let path: SharedPath = Rc::from(path.iter().cloned().chain(steps).collect::<Path>());
    let root = cx.view.clone();
    let destination = path.clone();
    let edits = cx.edits.clone();
    let payload_root = root.clone();
    let payload_destination = destination.clone();
    let payload_edits = edits.clone();
    crate::display::ProjectionTarget {
        select: Rc::new(move |world| {
            edits
                .open(crate::editing::Access::new(world))
                .select(&root, &destination);
            true
        }),
        select_with: Rc::new(move |world, payload| {
            payload_edits
                .open(crate::editing::Access::new(world))
                .select_payload(&payload_root, payload_destination.to_vec(), payload);
            true
        }),
        hover: Hovered::Tree(Hover::Value(path)),
    }
}

impl Cx<'_> {
    /// The display name at this projection. Raw interprets no naming
    /// convention and therefore falls back to the short id.
    fn name(&self, cell: CellId) -> Option<&str> {
        (!self.raw).then(|| self.sources.name(cell)).flatten()
    }

    /// Whether `path` carries the primary highlight. A label-stage
    /// pending deliberately does not mark its parent — nothing is
    /// selected there, something is being authored inside; the
    /// pending row carries the highlight itself.
    fn selected(&self, path: &[Step]) -> bool {
        self.selection.is_some_and(|current| {
            current.path() == path && current.stage(&self.sources) != Stage::Label
        })
    }

    /// The pending child step under `path`, when the selection is
    /// authoring one there.
    fn pending_child_of(&self, path: &[Step]) -> Option<Step> {
        let current = self.selection?;
        (current
            .path()
            .split_last()
            .is_some_and(|(_, parent)| parent == path)
            && current.stage(&self.sources) == Stage::Pending)
            .then(|| current.path().last().cloned())
            .flatten()
    }

    /// The label query of a new field being authored on the record at
    /// `path`.
    pub(crate) fn pending_edge_under(&self, path: &[Step]) -> Option<(&LineEditState, usize)> {
        let current = self.selection?;
        (current.path() == path && current.stage(&self.sources) == Stage::Label)
            .then(|| Some((current.edit()?, current.choice())))
            .flatten()
    }
}

fn edit_presentation(style: &TextStyle) -> LineEditPresentation {
    LineEditPresentation::new(style.size, style.brush.clone())
}

fn placeholder_box(tcx: &mut TextCtx, styles: &Styles) -> Measured<HoverPass<crate::Editor>> {
    crate::display::widget::empty(tcx, styles)
}

/// The pointer over this settled rect names `key`, with the visible
/// ink as its footprint. Placement order is precedence: descendants
/// and overlays contribute later and answer first.
fn hover_claim(p: &mut placed::Builder<'_, '_, crate::Editor>, placement: Placement, key: Hover) {
    p.claim(placement, Hovered::Tree(key));
}

/// An occluder: takes the pointer and names nothing, so targets
/// beneath an overlay never light.
fn hover_block(p: &mut placed::Builder<'_, '_, crate::Editor>, placement: Placement) {
    p.occlude(placement);
}

/// The pane-local primary: translucent system blue, like the Swift
/// version's selection, ringed at full strength — the strongest mark
/// in the shared vocabulary.
fn primary_highlight<P: Canvas + ?Sized>(
    palette: crate::styles::Palette,
    scale: f64,
    p: &mut P,
    outline: RoundedRect,
) {
    p.fill(outline, palette.accent.with_alpha(0.22), Affine::IDENTITY);
    p.stroke(
        outline,
        primary_highlight_stroke(scale),
        palette.accent,
        Affine::IDENTITY,
    );
}

fn primary_highlight_stroke(scale: f64) -> Stroke {
    Stroke::new(2.5 * scale)
}

/// The selected location shared by repeated projections of a cell.
fn secondary_of(sources: &Sources, selection: Option<&Selection>) -> Option<Secondary> {
    match selection? {
        current if current.stage(sources) == Stage::Edge => {
            let path: SharedPath = Rc::from(current.source_path()?.as_ref());
            sources
                .resolve_path(&path)
                .map(|value| Secondary::from_path(sources, path.clone(), value))
        }
        _ => None,
    }
}

/// The explicit-state boundary: everything a projection pass reads.
/// `width` is the space the projection may fill. `root` and
/// `root_path` let an editor pane begin at a value occurrence
/// while retaining ordinary document-relative interaction paths.
pub struct ProjectDescription<'a> {
    pub command_modifier: puri::keyboard::CommandModifier,
    pub computations: Option<&'a crate::computations::Computations>,
    pub focused: bool,
    pub view: &'a crate::workspace::Root,
    pub completions: Option<&'a crate::display::CompletionProvider>,
    pub sources: Sources<'a>,
    pub root: Option<&'a Value>,
    pub root_path: &'a [Step],
    /// Selection belonging to this editable view.
    pub selection: Option<&'a Selection>,
    /// Selection from any view, used only to link generated output
    /// back to its structural source.
    pub source_selection: Option<&'a Selection>,
    pub annotations: &'a Annotations,
    pub raw: bool,
    pub styles: &'a Styles,
    pub width: f64,
    pub projection: Option<&'a Projection<crate::Editor>>,
}

pub(crate) fn project(
    description: ProjectDescription<'_>,
    tcx: &mut TextCtx,
) -> Measured<HoverPass<crate::Editor>> {
    let width = description.width;
    resolve_choices(
        prepare_project(description, tcx),
        width,
        std::env::var_os("PROGRED_LAYOUT_TRACE").is_some(),
    )
}

fn prepare_project(
    description: ProjectDescription<'_>,
    tcx: &mut TextCtx,
) -> ChoiceGraph<HoverPass<crate::Editor>> {
    let ProjectDescription {
        command_modifier,
        computations,
        focused,
        view,
        completions,
        sources,
        root,
        root_path,
        selection,
        source_selection,
        annotations,
        raw,
        styles,
        width: _,
        projection,
    } = description;
    let projection = projection.cloned().unwrap_or_default();
    let cx = Cx {
        command_modifier,
        computations,
        focused,
        view,
        completions,
        sources,
        edits: Default::default(),
        raw,
        annotations,
        styles,
        selection,
        // Other projections of the selected cell are secondary. The
        // HOVERED value's faint marks come from the render pass's ResolvedHover.
        secondary: secondary_of(&sources, selection),
        selected_trace: source_selection.and_then(|selection| {
            Some(SourceTrace::from_path(
                &sources,
                Rc::from(selection.source_path()?.as_ref()),
            ))
        }),
    };
    // An empty document is a selectable placeholder at the root path.
    let mut build = ChoiceBuild::default();
    let mut ancestry = Ancestry::default();
    if let Some(Step::Follow(source)) = root_path.last()
        && let Some(cell) = sources
            .resolve_path(&root_path[..root_path.len() - 1])
            .and_then(Value::as_cell)
    {
        ancestry.cells.insert(cell);
        ancestry.enclosing = Some((cell, *source, root_path.len()));
    }
    let layout = prepare_value(
        &cx,
        &projection,
        None,
        None,
        tcx,
        root_path,
        &ancestry,
        root,
        &mut build,
    );
    build.finish(layout)
}

/// Marks `child` as the projection of `path` WITHOUT claiming any
/// clicks: the highlight, reveal rect, and keyboard reach of
/// [`descend`] over the full bounds, while pointer selection belongs
/// to the content targets the view registers — heads, delimiters,
/// rows — so clicks on structural whitespace (gutters, inter-row
/// gaps, the dead space inside a bounding box) fall through to the
/// background's deselect.
#[allow(clippy::too_many_arguments)]
fn descend_landmark_with(
    palette: crate::styles::Palette,
    selected: bool,
    scale: f64,
    path: SharedPath,
    secondary: Option<(Secondary, bool)>,
    select: crate::navigate::Select<crate::Editor>,
    scope: crate::editing::Scope,
    child: Measured<HoverPass<crate::Editor>>,
) -> Measured<HoverPass<crate::Editor>> {
    let highlight_path = path.clone();
    let marked = decorate(child, move |p, rect| {
        let highlight_path = highlight_path.clone();
        let outline = highlight_outline(scale, rect);
        p.render(move |cv, hover| {
            if selected {
                primary_highlight(palette, scale, cv, outline);
            } else if matches!(&secondary, Some((_, true))) {
                secondary_highlight(palette, scale, cv, outline, true);
            } else if matches!(
                tree_hovered(hover),
                Some(Hover::Value(hovered)) if hovered.as_ref() == highlight_path.as_ref()
            ) {
                hover_highlight(palette, scale, cv, outline);
            } else if secondary
                .as_ref()
                .is_some_and(|(secondary, _)| hover.hovered_secondary.as_ref() == Some(secondary))
            {
                secondary_highlight(palette, scale, cv, outline, false);
            }
        });
    });
    crate::display::widget::navigation::landmark(marked, path, select, scope)
}

/// Standard commands for this displayed value, below its own widget handlers.
/// The occurrence identifies the selection; only mutations need its conject.
fn bind_selection(
    child: Measured<HoverPass<crate::Editor>>,
    root: crate::workspace::Root,
    path: SharedPath,
    value: Value,
    fold_default: Option<bool>,
    scale: f64,
) -> Measured<HoverPass<crate::Editor>> {
    before(child, move |p, _| {
        p.handler().on_key_with(move |ctx, event, input| {
            if !event.state.is_down()
                || ctx.model.selection.as_ref().is_none_or(|selection| {
                    selection.root() != &root
                        || selection.path() != path.as_ref()
                        || selection.stage(&ctx.sources()) != Stage::Edge
                })
            {
                return false;
            }
            if ctx.command_modifier.pressed(&event.modifiers)
                && let Key::Character(key) = &event.key
                && (key.eq_ignore_ascii_case("c") || key.eq_ignore_ascii_case("x"))
            {
                if !ctx.copy_value(&value) {
                    return false;
                }
                if key.eq_ignore_ascii_case("x") {
                    ctx.delete_selected_edge(input.geometry(scale));
                }
                return true;
            }
            if let Some(default) = fold_default {
                let closed = match &event.key {
                    Key::Character(key) if key.as_str() == " " => Some(None),
                    Key::Named(NamedKey::ArrowUp)
                        if ctx.command_modifier.pressed(&event.modifiers) =>
                    {
                        Some(Some(true))
                    }
                    Key::Named(NamedKey::ArrowDown)
                        if ctx.command_modifier.pressed(&event.modifiers) =>
                    {
                        Some(Some(false))
                    }
                    _ => None,
                };
                if let Some(closed) = closed {
                    return ctx.set_collapsed(&root, &path, default, closed);
                }
            }
            crate::modifiers::plain(&event.modifiers)
                && matches!(
                    &event.key,
                    Key::Named(NamedKey::Backspace | NamedKey::Delete)
                )
                && ctx.delete_selected_edge(input.geometry(scale))
        })
    })
}

/// A cell projection's ground, painted only at authority
/// TRANSITIONS: an external cell under document authority takes the
/// library tint — no lock, just "from elsewhere" — and a
/// document-authority cell under an external one takes its light
/// ground back (opaque, since an alpha wash can't be undone by
/// another wash). Runs of the same authority draw nothing, so
/// nesting never stacks tints. The enclosing authority is the owning
/// cell at the path's last Follow, so a cell inside a list carries
/// its list's owner as context. Wraps outside the descend so the
/// cell's own selection highlight draws over its ground.
fn ground_decoration(cx: &Cx, path: &[Step], value: &Value) -> Option<(f64, Color)> {
    let Some(cell) = value.as_cell() else {
        return None;
    };
    let external = cx.sources.external(cell);
    let path = cx.edits.source(path)?;
    let parent_external = last_follow(&path)
        .is_some_and(|index| matches!(path[index], Step::Follow(gid::Resolution::Library(_))));
    if external == parent_external {
        return None;
    }
    let scale = cx.styles.scale;
    let color = if external {
        cx.styles.palette.library_ground
    } else {
        cx.styles.palette.paper
    };
    Some((scale, color))
}

fn ground_with(
    scale: f64,
    color: Color,
    content: Measured<HoverPass<crate::Editor>>,
) -> Measured<HoverPass<crate::Editor>> {
    decorate(content, move |p, rect| {
        let bg = RoundedRect::from_rect(rect.inset(3.0 * scale), 5.0 * scale);
        p.fill(bg, color, Affine::IDENTITY);
    })
}

fn secondary_highlight<P: Canvas + ?Sized>(
    palette: crate::styles::Palette,
    scale: f64,
    p: &mut P,
    outline: RoundedRect,
    strong: bool,
) {
    p.fill(
        outline,
        palette.accent.with_alpha(if strong { 0.10 } else { 0.05 }),
        Affine::IDENTITY,
    );
    if strong {
        p.stroke(
            outline,
            Stroke::new(1.5 * scale),
            palette.accent.with_alpha(0.55),
            Affine::IDENTITY,
        );
    }
}

/// Adds GID steps to the occurrence and resolves through its conject.
/// Current and descendant projections are independent replacements.
/// Missing locations use the standard empty picker, without a value.
#[allow(clippy::too_many_arguments)]
fn prepare_descend_path(
    cx: &Cx,
    projection: &Projection<crate::Editor>,
    tcx: &mut TextCtx,
    parent_path: &[Step],
    ancestors: &Ancestry,
    parent: Option<&Value>,
    steps: &[Step],
    current_projection: Option<crate::display::Partial<crate::Editor, Hovered>>,
    default_projection: Option<crate::display::Partial<crate::Editor, Hovered>>,
    build: &mut ChoiceBuild<HoverPass<crate::Editor>>,
) -> ChoiceLayout<HoverPass<crate::Editor>> {
    let mut path = parent_path.to_vec();
    let mut ancestors = ancestors.clone();
    let mut value = parent;
    for step in steps {
        if let Step::Follow(source) = &step {
            if let Some(cell) = value.and_then(Value::as_cell) {
                ancestors.cells.insert(cell);
                ancestors.enclosing = Some((cell, *source, path.len() + 1));
            }
        }
        path.push(step.clone());
        let mapped = (!cx.edits.is_identity())
            .then(|| cx.edits.source(&path))
            .flatten();
        value = match mapped {
            Some(source) => cx.sources.resolve_path(&source),
            None => value.and_then(|value| {
                location::child(value, step, |cell, source| cx.sources.value(cell, source))
            }),
        };
    }
    prepare_value(
        cx,
        projection,
        current_projection.as_ref(),
        default_projection.as_ref(),
        tcx,
        &path,
        &ancestors,
        value,
        build,
    )
}

#[allow(clippy::too_many_arguments)]
fn prepare_value(
    cx: &Cx,
    projection: &Projection<crate::Editor>,
    current_projection: Option<&crate::display::Partial<crate::Editor, Hovered>>,
    default_projection: Option<&crate::display::Partial<crate::Editor, Hovered>>,
    tcx: &mut TextCtx,
    path: &[Step],
    ancestors: &Ancestry,
    value: Option<&Value>,
    build: &mut ChoiceBuild<HoverPass<crate::Editor>>,
) -> ChoiceLayout<HoverPass<crate::Editor>> {
    let child_projection = default_projection
        .map(|partial| Projection {
            partial: partial.clone(),
            entry: None,
        })
        .or_else(|| {
            (projection.entry.is_some() && !matches!(value, Some(Value::Cell(_))))
                .then(|| projection.without_entry())
        });
    let child_projection = child_projection.as_ref().unwrap_or(projection);
    // Ancestry is already available from projection; folds must not re-read an
    // occurrence as a document path (and computed values have no such path).
    let fold_default = value.and_then(|value| {
        let in_cycle = value
            .as_cell()
            .is_some_and(|cell| ancestors.cells.contains(&cell));
        crate::selection::collapse_default_for_value(&cx.sources, value, in_cycle)
    });
    let layout = value_layout(
        cx,
        projection,
        current_projection,
        &child_projection.partial,
        path,
        value,
        fold_default,
    );
    match layout {
        None => ChoiceLayout::fixed(pending_view(cx, tcx, path.to_vec(), None)),
        Some(layout) => {
            let inner = prepare(
                cx,
                child_projection,
                tcx,
                path,
                ancestors,
                value,
                layout,
                build,
            );
            let landmark_path: SharedPath = Rc::from(path);
            // Other projections of the selected location carry the secondary
            // mark; the selected one has the primary highlight.
            let secondary = value.filter(|_| !cx.selected(path)).and_then(|value| {
                let secondary = if cx.edits.is_identity() {
                    Secondary::from_context(landmark_path.clone(), value, ancestors.enclosing)
                } else {
                    let source = cx.edits.source(path)?;
                    Secondary::from_path(&cx.sources, Rc::from(source.as_ref()), value)
                };
                let strong = cx.secondary.as_ref() == Some(&secondary);
                Some((secondary, strong))
            });
            // A landmark, not a target: highlight and keyboard reach span
            // the full bounds, while clicks belong to the content each arm
            // claimed above — structural whitespace deselects.
            let selected = cx.selected(path);
            let selected_value = value.filter(|_| selected).cloned();
            let root = cx.view.clone();
            let scale = cx.styles.scale;
            let palette = cx.styles.palette;
            let select = navigation_select_handler(landmark_path.clone(), cx);
            let ground = value.and_then(|value| ground_decoration(cx, path, value));
            let target = value.map(|value| (value.clone(), cx.view.clone(), landmark_path.clone()));
            let edits = cx.edits.clone();
            let pick_edits = edits.clone();
            ChoiceLayout::map(inner, 0.0, move |inner| {
                let placed = descend_landmark_with(
                    palette,
                    selected,
                    scale,
                    landmark_path.clone(),
                    secondary,
                    select,
                    edits.clone(),
                    inner,
                );
                let placed = match selected_value {
                    Some(value) => bind_selection(
                        placed,
                        root,
                        landmark_path.clone(),
                        value,
                        fold_default,
                        scale,
                    ),
                    None => placed,
                };
                let grounded = match ground {
                    Some((scale, color)) => ground_with(scale, color, placed),
                    None => placed,
                };
                match target {
                    Some((value, root, destination)) => pick_target_with(
                        landmark_path,
                        value,
                        root,
                        destination,
                        pick_edits,
                        grounded,
                    ),
                    None => grounded,
                }
            })
        }
    }
}

fn value_layout(
    cx: &Cx,
    projection: &Projection<crate::Editor>,
    current_projection: Option<&crate::display::Partial<crate::Editor, Hovered>>,
    default_projection: &crate::display::Partial<crate::Editor, Hovered>,
    path: &[Step],
    value: Option<&Value>,
    fold_default: Option<bool>,
) -> Option<crate::display::Layout<crate::Editor, Hovered>> {
    #[cfg(all(test, feature = "layout-profile"))]
    let _profile = crate::display::profile::enter(crate::display::profile::Kind::Projection);
    if let Some(value) = value
        && let Some(default) = fold_default
        && crate::annotations::collapsed(cx.annotations, path, default)
        && let Some(collapsed) = structure::collapsed_layout(cx, path, value, default)
    {
        return Some(collapsed);
    }
    let selection = cx
        .selection
        .filter(|current| current.path() == path)
        .map(Selection::payload);
    let pending = if cx.pending_edge_under(path).is_some() {
        Some(crate::display::Pending::Field)
    } else {
        cx.pending_child_of(path)
            .map(crate::display::Pending::Child)
    };
    let state = cx.annotations.at(path);
    let target = |steps| projection_target(cx, path, steps);
    let input = crate::display::ProjectionInput {
        env: &ProjectEnv { cx, path },
        default_projection: default_projection.clone(),
        value,
        scale_factor: cx.styles.scale,
        writable: cx.edits.writable(&cx.sources, path),
        selection: selection.as_ref(),
        pending,
        state,
        targets: crate::display::ProjectionTargets::new(&target),
    };
    match current_projection {
        Some(current) => current(&input),
        None => projection.apply(&input),
    }
    .or_else(|| value.map(|value| structure::of(cx, path, value, &input)))
}

/// Every projected value's Pick backstop: pick the value into an open
/// pending, or — nothing pending — select it like Activate, so a stray
/// modifier never deadens the gesture. Inner Pick registrations answer
/// first and this catches what they refused.
fn pick_target_with(
    path: SharedPath,
    value: Value,
    root: crate::workspace::Root,
    destination: SharedPath,
    edits: crate::editing::Scope,
    child: Measured<HoverPass<crate::Editor>>,
) -> Measured<HoverPass<crate::Editor>> {
    before(child, move |p, _| {
        let path = path.clone();
        let value = value.clone();
        p.pick(Hovered::Tree(Hover::Value(path.clone())), move |world| {
            if !world.pick_identity(value.clone()) {
                edits
                    .open(crate::editing::Access::new(world))
                    .select(&root, &destination);
            }
            true
        });
    })
}

/// An editable atom's content: the selection's focused editor when
/// this atom is being edited — with `placeholder` as its ghost while
/// empty — its static text otherwise.
fn atom_content(
    editing: Option<&LineEditState>,
    focused: bool,
    fallback: Measured<HoverPass<crate::Editor>>,
    presentation: LineEditPresentation,
    placeholder: Option<(&str, &TextStyle)>,
    tcx: &mut TextCtx,
    styles: &Styles,
) -> Measured<HoverPass<crate::Editor>> {
    match editing {
        Some(line) => render::text_edit(
            LineEditDescription {
                state: line,
                focused,
                presentation,
                style: &styles.edit,
                placeholder,
            },
            tcx,
            crate::editing::edit_query,
        ),
        None => fallback,
    }
}
