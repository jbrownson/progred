//! The editor's projection runtime: resolve locations, supply borrowed widget inputs,
//! retain source provenance, and fall back to total structural display.

pub(crate) mod completion;
pub(crate) mod drawing;
pub(crate) mod line_control;
mod location;
mod structure;
#[cfg(test)]
mod tests;
pub(crate) mod viewport;

use crate::annotations::Annotations;
use crate::display::widget::style::{face_style, highlight_outline};
use crate::frame::Hovered;
use crate::hover::{Hover, Secondary, SourceTrace};
#[cfg(test)]
use crate::navigate::Descend;
use crate::placed::{self, HoverPass, before, decorate};
use crate::render;
use crate::selection::{Selection, Stage, last_follow, writable_at};
use crate::sources::Sources;
use crate::styles::Styles;
use completion::pending_view;
use gid::{CellId, Path, Step, Value};
#[cfg(test)]
use kurbo::Point;
use kurbo::{Affine, RoundedRect, Stroke};
use location::Location;
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
#[cfg_attr(test, derive(Clone))]
pub(crate) struct Cx<'a> {
    pub(crate) view: &'a crate::workspace::Root,
    pub(crate) completions: Option<&'a crate::display::CompletionProvider>,
    /// The reading context: the document read over its library.
    pub(crate) sources: Sources<'a>,
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
    pub(crate) source: Source<'a>,
    pub(crate) fuel: std::cell::Cell<usize>,
}

#[derive(Clone, Default)]
struct Ancestry {
    /// Cells crossed by `Follow`, used to stop projection cycles.
    cells: HashSet<CellId>,
    /// The nearest followed definition and the start of its relative path.
    enclosing: Option<(CellId, gid::Resolution, usize)>,
}

#[derive(Clone, Copy)]
pub(crate) enum Source<'a> {
    Stored,
    Transient { owner: &'a [Step] },
}

impl Source<'_> {
    pub(crate) fn transient(self) -> bool {
        matches!(self, Self::Transient { .. })
    }
}

struct ProjectEnv<'a, 's> {
    cx: &'a Cx<'s>,
}

impl crate::display::Env for ProjectEnv<'_, '_> {
    fn apply_scoped(
        &self,
        function: &Value,
        arguments: &[(CellId, Value)],
        scope: Option<&grap::ForeignOverlay<'_>>,
    ) -> grap::Evaluation {
        let fuel = if self.cx.source.transient() {
            self.cx.fuel.get()
        } else {
            grap::DEFAULT_FUEL
        };
        let evaluation = match scope {
            Some(scope) => grap::apply_scoped(
                function,
                arguments.iter().cloned(),
                &self.cx.sources,
                scope,
                fuel,
            ),
            None => grap::apply(function, arguments.iter().cloned(), &self.cx.sources, fuel),
        };
        self.cx.fuel.set(evaluation.remaining_fuel);
        evaluation
    }

    fn evaluate(&self, expression: &Value) -> (Value, usize) {
        let fuel = if self.cx.source.transient() {
            self.cx.fuel.get()
        } else {
            grap::DEFAULT_FUEL
        };
        let evaluation = evaluate(self.cx, expression, fuel);
        self.cx.fuel.set(evaluation.remaining_fuel);
        (evaluation.result, evaluation.remaining_fuel)
    }

    fn evaluate_with_fuel(&self, expression: &Value, fuel: usize) -> (Value, usize) {
        let fuel = if self.cx.source.transient() {
            self.cx.fuel.get().min(fuel)
        } else {
            fuel
        };
        let evaluation = evaluate(self.cx, expression, fuel);
        self.cx.fuel.set(evaluation.remaining_fuel);
        (evaluation.result, evaluation.remaining_fuel)
    }

    fn name(&self, cell: CellId) -> Option<&str> {
        self.cx.name(cell)
    }

    fn resolve(&self, cell: CellId) -> Option<crate::display::ResolvedCell<'_>> {
        self.cx.sources.definition(cell)
    }
}

fn evaluate(cx: &Cx<'_>, expression: &Value, fuel: usize) -> grap::Evaluation {
    grap::evaluate(expression, &cx.sources, fuel)
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
        step: Step,
        current: Option<crate::display::Partial<crate::Editor, Hovered>>,
        default: Option<crate::display::Partial<crate::Editor, Hovered>>,
    ) -> ChoiceLayout<HoverPass<crate::Editor>> {
        prepare_descend(
            self.cx,
            self.projection,
            text,
            self.path,
            self.ancestors,
            self.value,
            step,
            current,
            default,
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
            self.ancestors,
            steps,
            value,
            current,
            default,
            build,
        )
    }
    fn transient(
        &self,
        text: &mut TextCtx,
        build: &mut ChoiceBuild<HoverPass<crate::Editor>>,
        value: Value,
        fuel: usize,
    ) -> ChoiceLayout<HoverPass<crate::Editor>> {
        prepare_transient_root(
            self.cx,
            self.projection,
            text,
            self.path,
            value,
            fuel,
            build,
        )
    }
}

fn prepare_at(
    cx: &Cx,
    projection: &Projection<crate::Editor>,
    tcx: &mut TextCtx,
    path: &[Step],
    ancestors: &Ancestry,
    steps: Vec<Step>,
    nested: Value,
    current_projection: Option<crate::display::Partial<crate::Editor, Hovered>>,
    default_projection: Option<crate::display::Partial<crate::Editor, Hovered>>,
    build: &mut ChoiceBuild<HoverPass<crate::Editor>>,
) -> ChoiceLayout<HoverPass<crate::Editor>> {
    let mut path = path.to_vec();
    let mut follow_ancestors = ancestors.clone();
    for step in &steps {
        if let Step::Follow(source) = step {
            if let Some(cell) = cx.sources.resolve_path(&path).and_then(Value::as_cell) {
                follow_ancestors.cells.insert(cell);
                follow_ancestors.enclosing = Some((cell, *source, path.len() + 1));
            }
        }
        path.push(step.clone());
    }
    prepare_value(
        cx,
        projection,
        current_projection.as_ref(),
        default_projection.as_ref(),
        tcx,
        &path,
        &follow_ancestors,
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
    let destination = match cx.source {
        Source::Stored => path,
        Source::Transient { owner } => Rc::from(owner),
    };
    Rc::new(move |world| {
        crate::editing::select(world, &root, &destination);
        true
    })
}

fn navigation_select_handler(path: SharedPath, cx: &Cx) -> crate::navigate::Select<crate::Editor> {
    let root = cx.view.clone();
    let destination = match cx.source {
        Source::Stored => path,
        Source::Transient { owner } => Rc::from(owner),
    };
    Rc::new(move |world, _| {
        crate::editing::select(world, &root, &destination);
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
    let destination = match cx.source {
        Source::Stored => path.clone(),
        Source::Transient { owner } => Rc::from(owner),
    };
    let payload_root = root.clone();
    let payload_destination = destination.clone();
    crate::display::ProjectionTarget {
        select: Rc::new(move |world| {
            crate::editing::select(world, &root, &destination);
            true
        }),
        select_with: Rc::new(move |world, payload| {
            crate::editing::select_payload(
                world,
                &payload_root,
                payload_destination.to_vec(),
                payload,
            );
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

/// The pointer's preview of a click's meaning, washed faint.
fn hover_highlight<P: Canvas + ?Sized>(p: &mut P, outline: RoundedRect) {
    p.fill(
        outline,
        crate::display::widget::style::hover_wash(),
        Affine::IDENTITY,
    );
}

/// The pane-local primary: translucent system blue, like the Swift
/// version's selection, ringed at full strength — the strongest mark
/// in the shared vocabulary.
fn primary_highlight<P: Canvas + ?Sized>(scale: f64, p: &mut P, outline: RoundedRect) {
    p.fill(
        outline,
        Color::new([0.0, 0.48, 1.0, 0.22]),
        Affine::IDENTITY,
    );
    p.stroke(
        outline,
        primary_highlight_stroke(scale),
        Color::new([0.0, 0.48, 1.0, 1.0]),
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
            let path: SharedPath = Rc::from(current.path());
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
        view,
        completions,
        sources,
        raw,
        annotations,
        styles,
        selection,
        source: Source::Stored,
        fuel: std::cell::Cell::new(grap::DEFAULT_FUEL),
        // Other projections of the selected cell are secondary. The
        // HOVERED value's faint marks come from the render pass's ResolvedHover.
        secondary: secondary_of(&sources, selection),
        selected_trace: source_selection
            .map(|selection| SourceTrace::from_path(&sources, Rc::from(selection.path()))),
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
    let layout = prepare_location(
        &cx,
        &projection,
        tcx,
        root_path,
        &ancestry,
        Location::Root(root),
        None,
        None,
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
    transient: bool,
    selected: bool,
    scale: f64,
    path: SharedPath,
    secondary: Option<(Secondary, bool)>,
    select: crate::navigate::Select<crate::Editor>,
    child: Measured<HoverPass<crate::Editor>>,
) -> Measured<HoverPass<crate::Editor>> {
    let highlight_path = path.clone();
    let marked = decorate(child, move |p, rect| {
        let highlight_path = highlight_path.clone();
        let outline = highlight_outline(scale, rect);
        p.render(move |cv, hover| {
            if selected && !transient {
                primary_highlight(scale, cv, outline);
            } else if matches!(&secondary, Some((_, true))) {
                secondary_highlight(scale, cv, outline, true);
            } else if !transient
                && matches!(
                    tree_hovered(hover),
                    Some(Hover::Value(hovered)) if hovered.as_ref() == highlight_path.as_ref()
                )
            {
                hover_highlight(cv, outline);
            } else if secondary
                .as_ref()
                .is_some_and(|(secondary, _)| hover.hovered_secondary.as_ref() == Some(secondary))
            {
                secondary_highlight(scale, cv, outline, false);
            }
        });
    });
    if transient {
        return marked;
    }
    let marked = crate::display::widget::navigation::landmark(marked, path, select);
    if selected {
        bind_delete(marked)
    } else {
        marked
    }
}

fn bind_delete(child: Measured<HoverPass<crate::Editor>>) -> Measured<HoverPass<crate::Editor>> {
    before(child, move |p, _| {
        p.handler().on_key_with(move |ctx, event, input| {
            crate::modifiers::plain(&event.modifiers)
                && matches!(
                    &event.key,
                    Key::Named(NamedKey::Backspace | NamedKey::Delete)
                )
                && event.state.is_down()
                && ctx.delete_selected_edge(&input.descends)
        })
    })
}

/// A cell projection's ground, painted only at authority
/// TRANSITIONS: an external cell under document authority takes the
/// dark tint — no lock, just "from elsewhere" — and a
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
    let parent_external = last_follow(path)
        .is_some_and(|index| matches!(path[index], Step::Follow(gid::Resolution::Library(_))));
    if external == parent_external {
        return None;
    }
    let scale = cx.styles.scale;
    let color = if external {
        Color::new([0.13, 0.14, 0.16, 0.05])
    } else {
        Color::new([0.965, 0.965, 0.972, 1.0])
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
    scale: f64,
    p: &mut P,
    outline: RoundedRect,
    strong: bool,
) {
    let (fill, line) = if strong { (0.10, 0.55) } else { (0.05, 0.25) };
    p.fill(
        outline,
        Color::new([0.0, 0.48, 1.0, fill]),
        Affine::IDENTITY,
    );
    p.stroke(
        outline,
        Stroke::new(1.5 * scale),
        Color::new([0.0, 0.48, 1.0, line]),
        Affine::IDENTITY,
    );
}

/// Starts the ordinary projection at a value with no document source.
/// Interaction attributes the transient tree to `owner`, while its
/// children remain read-only and have no document paths of their own.
fn prepare_transient_root(
    cx: &Cx,
    projection: &Projection<crate::Editor>,
    tcx: &mut TextCtx,
    path: &[Step],
    result: Value,
    fuel: usize,
    build: &mut ChoiceBuild<HoverPass<crate::Editor>>,
) -> ChoiceLayout<HoverPass<crate::Editor>> {
    let ordinary_projection = projection.without_entry();
    let projection = &ordinary_projection;
    let result_cx = Cx {
        view: cx.view,
        completions: cx.completions,
        sources: cx.sources,
        raw: false,
        annotations: cx.annotations,
        styles: cx.styles,
        selection: None,
        secondary: None,
        selected_trace: cx.selected_trace.clone(),
        source: Source::Transient { owner: path },
        fuel: std::cell::Cell::new(fuel),
    };
    prepare_location(
        &result_cx,
        projection,
        tcx,
        path,
        &Ancestry::default(),
        Location::Root(Some(&result)),
        None,
        None,
        build,
    )
}

/// Adds one GID step to the active source and resolves that location.
/// Current and descendant projections are independent replacements.
/// Missing locations use the standard empty picker, without a value.
#[allow(clippy::too_many_arguments)]
fn prepare_descend(
    cx: &Cx,
    projection: &Projection<crate::Editor>,
    tcx: &mut TextCtx,
    parent_path: &[Step],
    ancestors: &Ancestry,
    parent: Option<&Value>,
    step: Step,
    current_projection: Option<crate::display::Partial<crate::Editor, Hovered>>,
    default_projection: Option<crate::display::Partial<crate::Editor, Hovered>>,
    build: &mut ChoiceBuild<HoverPass<crate::Editor>>,
) -> ChoiceLayout<HoverPass<crate::Editor>> {
    let mut path = parent_path.to_vec();
    path.push(step.clone());
    if let Step::Follow(source) = &step
        && let Some(parent) = parent
    {
        let mut ancestors = ancestors.clone();
        if let Some(cell) = parent.as_cell() {
            ancestors.cells.insert(cell);
            ancestors.enclosing = Some((cell, *source, path.len()));
        }
        prepare_location(
            cx,
            projection,
            tcx,
            &path,
            &ancestors,
            Location::Child { parent, step },
            current_projection.as_ref(),
            default_projection.as_ref(),
            build,
        )
    } else {
        prepare_location(
            cx,
            projection,
            tcx,
            &path,
            ancestors,
            match parent {
                Some(parent) => Location::Child { parent, step },
                None => Location::Root(None),
            },
            current_projection.as_ref(),
            default_projection.as_ref(),
            build,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn prepare_location(
    cx: &Cx,
    projection: &Projection<crate::Editor>,
    tcx: &mut TextCtx,
    path: &[Step],
    ancestors: &Ancestry,
    location: Location<'_>,
    current_projection: Option<&crate::display::Partial<crate::Editor, Hovered>>,
    default_projection: Option<&crate::display::Partial<crate::Editor, Hovered>>,
    build: &mut ChoiceBuild<HoverPass<crate::Editor>>,
) -> ChoiceLayout<HoverPass<crate::Editor>> {
    prepare_value(
        cx,
        projection,
        current_projection,
        default_projection,
        tcx,
        path,
        ancestors,
        location.value(|cell, resolution| cx.sources.value(cell, resolution)),
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
    let layout = value_layout(
        cx,
        projection,
        current_projection,
        &child_projection.partial,
        path,
        ancestors,
        value,
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
            let secondary = value.filter(|_| !cx.selected(path)).map(|value| {
                let secondary =
                    Secondary::from_context(landmark_path.clone(), value, ancestors.enclosing);
                let strong = cx.secondary.as_ref() == Some(&secondary);
                (secondary, strong)
            });
            // A landmark, not a target: highlight and keyboard reach span
            // the full bounds, while clicks belong to the content each arm
            // claimed above — structural whitespace deselects.
            let transient = cx.source.transient();
            let selected = cx.selected(path);
            let scale = cx.styles.scale;
            let select = navigation_select_handler(landmark_path.clone(), cx);
            let ground = value.and_then(|value| ground_decoration(cx, path, value));
            let target = value.map(|value| {
                (
                    value.clone(),
                    cx.view.clone(),
                    match cx.source {
                        Source::Stored => landmark_path.clone(),
                        Source::Transient { owner } => Rc::from(owner),
                    },
                )
            });
            ChoiceLayout::map(inner, 0.0, move |inner| {
                let placed = descend_landmark_with(
                    transient,
                    selected,
                    scale,
                    landmark_path.clone(),
                    secondary,
                    select,
                    inner,
                );
                let grounded = match ground {
                    Some((scale, color)) => ground_with(scale, color, placed),
                    None => placed,
                };
                match target {
                    Some((value, root, destination)) => {
                        pick_target_with(landmark_path, value, root, destination, grounded)
                    }
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
    ancestors: &Ancestry,
    value: Option<&Value>,
) -> Option<crate::display::Layout<crate::Editor, Hovered>> {
    #[cfg(all(test, feature = "layout-profile"))]
    let _profile = crate::display::profile::enter(crate::display::profile::Kind::Projection);
    // Ancestry has already accumulated the cells crossed by Follow
    // edges. A repeated cell is the graph cycle; re-resolving this
    // path and all its prefixes here made every frame walk from the
    // root once per projected value.
    let in_cycle = value
        .and_then(Value::as_cell)
        .is_some_and(|cell| ancestors.cells.contains(&cell));
    if let Some(value) = value
        && crate::selection::collapse_default_for_value(&cx.sources, value, in_cycle)
            .is_some_and(|default| crate::annotations::collapsed(cx.annotations, path, default))
        && let Some(collapsed) = structure::collapsed_layout(cx, path, value)
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
    let insert = |position| {
        let target: SharedPath = Rc::from(
            path.iter()
                .cloned()
                .chain([Step::Element(position)])
                .collect::<Path>(),
        );
        let root = cx.view.clone();
        let writable = !cx.source.transient();
        let destination = target.clone();
        let action: crate::display::ActionHandler<crate::Editor> = Rc::new(move |world| {
            if writable {
                crate::editing::insert(world, &root, &destination);
            }
            true
        });
        (Hovered::Tree(Hover::Insert(target)), action)
    };
    let input = crate::display::ProjectionInput {
        env: &ProjectEnv { cx },
        default_projection: default_projection.clone(),
        value,
        scale_factor: cx.styles.scale,
        writable: !cx.source.transient() && writable_at(&cx.sources, path),
        selection: selection.as_ref(),
        pending,
        state,
        targets: crate::display::ProjectionTargets::new(&target).with_insert_after(&insert),
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
    child: Measured<HoverPass<crate::Editor>>,
) -> Measured<HoverPass<crate::Editor>> {
    before(child, move |p, _| {
        let path = path.clone();
        let value = value.clone();
        p.pick(Hovered::Tree(Hover::Value(path.clone())), move |world| {
            if !world.pick_identity(value.clone()) {
                crate::editing::select(world, &root, &destination);
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
                focused: true,
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
