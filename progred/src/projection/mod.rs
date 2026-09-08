//! The editor's projection runtime: resolve locations, supply widget capabilities,
//! retain source provenance, and fall back to total structural display.

mod completion;
mod drawing;
pub(crate) mod line_control;
mod location;
mod structure;
#[cfg(test)]
mod tests;
pub(crate) mod viewport;

use crate::annotations::Annotations;
use crate::frame::Hovered;
use crate::hover::{Hover, Secondary, SourceTrace};
use crate::navigate::Descend;
use crate::placed::{self, Placed, before, decorate};
use crate::render;
use crate::selection::{Selection, Stage, last_follow, writable_at};
use crate::sources::Sources;
use crate::styles::Styles;
use completion::{label_query, pending_view};
use gid::{CellId, Path, Step, Value};
use kurbo::{Affine, Point, RoundedRect, Stroke};
use location::Location;
use measured::Measured;
use measured::choices::{ChoiceBuild, ChoiceGraph, ChoiceLayout, resolve_choices};
use peniko::Color;
use progred_display::widget::style::{face_style, highlight_outline};
use puri::draw::Canvas;
use puri::edit::{LineEditDescription, LineEditPresentation, LineEditState};
use puri::geometry::Placement;
use puri::handler::HasHandler;
use puri::interact::is_primary_contact;
use puri::text::{TextCtx, TextStyle};
use std::collections::HashSet;
use std::rc::Rc;
use ui_events::keyboard::{Key, NamedKey};

type SharedPath = Rc<[Step]>;

/// One ordered composition of partial value projections. The
/// structural fallback lives in this runtime and is always total.
pub struct Projection<World> {
    partial: progred_display::Partial<World, Hovered>,
    entry: Option<progred_display::Partial<World, Hovered>>,
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
            partial: progred_display::partial(|_| None),
            entry: None,
        }
    }
}

impl<World: 'static> Projection<World> {
    pub fn new(
        partials: impl IntoIterator<Item = progred_display::Partial<World, Hovered>>,
    ) -> Self {
        Self {
            partial: progred_display::compose_partials(partials),
            entry: None,
        }
    }

    /// Prepend a partial at entry, following cells to their definitions.
    /// Its children and computed results use the ordinary projection.
    pub fn with_entry(self, partial: progred_display::Partial<World, Hovered>) -> Self {
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
        input: &progred_display::ProjectionInput<'_, World, Hovered>,
    ) -> Option<progred_display::Layout<World, Hovered>> {
        self.entry
            .as_ref()
            .and_then(|entry| entry(input))
            .or_else(|| (self.partial)(input))
    }
}

/// Read-only projection context threaded through every view.
struct Cx<'a> {
    /// The reading context: the document read over its library.
    sources: Sources<'a>,
    /// Names and field order derive from this view bit. Value
    /// projections come from the editor's stack; `grap` is one of them.
    raw: bool,
    annotations: &'a Annotations,
    styles: &'a Styles,
    selection: Option<&'a Selection>,
    scrub_spelling: Option<(&'a [Step], &'a str)>,
    /// The selected cell-relative location whose other projections
    /// carry the secondary mark.
    secondary: Option<Secondary>,
    /// The selected structural source, normalized across projections
    /// for execution-linked output.
    selected_trace: Option<SourceTrace>,
    source: Source<'a>,
    fuel: std::cell::Cell<usize>,
}

#[derive(Clone, Default)]
struct Ancestry {
    /// Cells crossed by `Follow`, used to stop projection cycles.
    cells: HashSet<CellId>,
    /// The nearest followed definition and the start of its relative path.
    enclosing: Option<(CellId, gid::Resolution, usize)>,
}

#[derive(Clone, Copy)]
enum Source<'a> {
    Stored,
    Transient { owner: &'a [Step] },
}

impl Source<'_> {
    fn transient(self) -> bool {
        matches!(self, Self::Transient { .. })
    }
}

struct ProjectEnv<'a, 's> {
    cx: &'a Cx<'s>,
}

impl progred_display::Env for ProjectEnv<'_, '_> {
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

    fn resolve(&self, cell: CellId) -> Option<progred_display::ResolvedCell<'_>> {
        self.cx.sources.definition(cell)
    }
}

fn evaluate(cx: &Cx<'_>, expression: &Value, fuel: usize) -> grap::Evaluation {
    grap::evaluate(expression, &cx.sources, fuel)
}

/// Prepare a projection with its explicit source and widget capabilities.
/// Box composition itself belongs to the display measurement interpreter.
#[allow(clippy::too_many_arguments)]
fn prepare<C: 'static>(
    cx: &Cx,
    projection: &Projection<C>,
    tcx: &mut TextCtx,
    path: &[Step],
    ancestors: &Ancestry,
    hooks: &Hooks<C>,
    value: Option<&Value>,
    layout: progred_display::Layout<C, Hovered>,
    build: &mut ChoiceBuild<Placed<C>>,
) -> ChoiceLayout<Placed<C>> {
    let project = ProjectionScope {
        cx,
        projection,
        path,
        ancestors,
        value,
        hooks,
    };
    with_widget_context(cx, tcx, path, value, hooks, &project, |context| {
        layout.measure(context, build)
    })
}

struct ProjectionScope<'a, 's, C> {
    cx: &'a Cx<'s>,
    projection: &'a Projection<C>,
    path: &'a [Step],
    ancestors: &'a Ancestry,
    value: Option<&'a Value>,
    hooks: &'a Hooks<C>,
}

impl<C: 'static> progred_display::widget::project::Project<C, Hovered>
    for ProjectionScope<'_, '_, C>
{
    fn descend(
        &self,
        text: &mut TextCtx,
        build: &mut ChoiceBuild<Placed<C>>,
        step: Step,
        current: Option<progred_display::Partial<C, Hovered>>,
        default: Option<progred_display::Partial<C, Hovered>>,
    ) -> ChoiceLayout<Placed<C>> {
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
            self.hooks,
            build,
        )
    }
    fn at(
        &self,
        text: &mut TextCtx,
        build: &mut ChoiceBuild<Placed<C>>,
        steps: Vec<Step>,
        value: Value,
        current: Option<progred_display::Partial<C, Hovered>>,
        default: Option<progred_display::Partial<C, Hovered>>,
    ) -> ChoiceLayout<Placed<C>> {
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
            self.hooks,
            build,
        )
    }
    fn transient(
        &self,
        text: &mut TextCtx,
        build: &mut ChoiceBuild<Placed<C>>,
        value: Value,
        fuel: usize,
    ) -> ChoiceLayout<Placed<C>> {
        prepare_transient_root(
            self.cx,
            self.projection,
            text,
            self.path,
            value,
            fuel,
            self.hooks,
            build,
        )
    }
}

fn prepare_at<C: 'static>(
    cx: &Cx,
    projection: &Projection<C>,
    tcx: &mut TextCtx,
    path: &[Step],
    ancestors: &Ancestry,
    steps: Vec<Step>,
    nested: Value,
    current_projection: Option<progred_display::Partial<C, Hovered>>,
    default_projection: Option<progred_display::Partial<C, Hovered>>,
    hooks: &Hooks<C>,
    build: &mut ChoiceBuild<Placed<C>>,
) -> ChoiceLayout<Placed<C>> {
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
        hooks,
        build,
    )
}

/// The resolved hover's tree identity, for ink that lights its own
/// claim.
fn tree_hovered<'a>(ink: placed::Ink<'a>) -> Option<&'a Hover> {
    match ink.hovered {
        Some(Hovered::Tree(hover)) => Some(hover),
        _ => None,
    }
}

/// Host callbacks for library value offers, selection, editing,
/// and dispatch access to caller-owned state and platform services.
pub struct Hooks<C> {
    /// Library vocabulary used when the control does not specify its own.
    pub completions: Option<progred_display::CompletionProvider>,
    pub select: Rc<dyn Fn(&mut C, Path)>,
    /// Select a visible occurrence of a drawing's structural source.
    pub select_source: Rc<dyn Fn(&mut C, &[crate::navigate::Descend<C>], &SourceTrace)>,
    pub select_payload: Rc<dyn Fn(&mut C, Path, Value)>,
    /// Access a selected line's state, using its current description
    /// when no editing state has been stored yet.
    pub edit_line: Rc<
        dyn Fn(&mut C, &[Step], &progred_display::LineEdit, &puri::edit::EditOperation<'_>) -> bool,
    >,
    pub toggle: Rc<dyn Fn(&mut C, Path)>,
    /// Replace the annotation value at one projection site. The
    /// concrete view root remains host-owned and closed over here.
    pub update_state: Rc<dyn Fn(&mut C, Path, Value) -> bool>,
    /// Run an operation on the selected query, declining if it is gone.
    pub edit: Rc<dyn Fn(&mut C, &puri::edit::EditOperation<'_>) -> bool>,
    /// Commit a pointed-at value into the open pending (value or
    /// label stage); false when nothing is pending, so the click
    /// falls through to selection.
    pub pick: Rc<dyn Fn(&mut C, Value) -> bool>,
    /// Open a pending sibling after the element at `path` — the flat
    /// list separator's click.
    pub insert: Rc<dyn Fn(&mut C, Path)>,
    /// Delete the selected edge. Installed on the selected descend
    /// so Raw and library projections share one handler.
    pub delete: Rc<dyn Fn(&mut C, &[Descend<C>]) -> bool>,
    /// Apply a Grap event handler at `path` with the event as data and
    /// capabilities closed over that site.
    pub apply: Rc<dyn Fn(&mut C, Path, Value, Value) -> bool>,
    pub start_gesture:
        Rc<dyn Fn(&mut C, Path, Box<dyn progred_display::widget::gesture::Gesture<C>>, &[Point])>,
    pub value_edit: Rc<dyn Fn(Path) -> progred_display::widget::gesture::ValueEdit<C>>,
    /// Commit one of the exact offers shown by an engaged pending.
    pub commit_value: Rc<dyn Fn(&mut C, Value, Option<Value>)>,
    pub commit_label: Rc<dyn Fn(&mut C, CellId, Option<Value>, Option<Value>)>,
    /// Retain the completion offset and choice in the pending selection.
    pub set_completion_view: Rc<dyn Fn(&mut C, f64, usize, bool)>,
}

fn select_handler<C: 'static>(
    path: SharedPath,
    hooks: &Hooks<C>,
) -> progred_display::ActionHandler<C> {
    let select = hooks.select.clone();
    Rc::new(move |world| {
        select(world, path.to_vec());
        true
    })
}

fn navigation_select_handler<C: 'static>(
    path: SharedPath,
    hooks: &Hooks<C>,
) -> crate::navigate::Select<C> {
    let select = hooks.select.clone();
    Rc::new(move |world, _| {
        select(world, path.to_vec());
        true
    })
}

fn projection_target<C: 'static>(
    path: &[Step],
    hooks: &Hooks<C>,
    steps: Vec<Step>,
) -> progred_display::ProjectionTarget<C, Hovered> {
    let path: SharedPath = Rc::from(path.iter().cloned().chain(steps).collect::<Path>());
    let selected = path.clone();
    let selected_with = path.clone();
    let select = hooks.select.clone();
    let select_payload = hooks.select_payload.clone();
    progred_display::ProjectionTarget {
        select: Rc::new(move |world| {
            select(world, selected.to_vec());
            true
        }),
        select_with: Rc::new(move |world, payload| {
            select_payload(world, selected_with.to_vec(), payload);
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
    fn pending_edge_under(&self, path: &[Step]) -> Option<(&LineEditState, usize)> {
        let current = self.selection?;
        (current.path() == path && current.stage(&self.sources) == Stage::Label)
            .then(|| Some((current.edit()?, current.choice())))
            .flatten()
    }
}

fn edit_presentation(style: &TextStyle) -> LineEditPresentation {
    LineEditPresentation::new(style.size, style.brush.clone())
}

fn with_widget_context<C: 'static, Result>(
    cx: &Cx,
    text: &mut TextCtx,
    path: &[Step],
    value: Option<&Value>,
    hooks: &Hooks<C>,
    project: &dyn progred_display::widget::project::Project<C, Hovered>,
    widget: impl FnOnce(&mut progred_display::widget::Context<'_, '_, C, Hovered>) -> Result,
) -> Result {
    let initial_text = |spelling: &str| {
        cx.selection
            .filter(|selection| selection.path() == path)
            .map(|selection| selection.initial_line(spelling))
            .unwrap_or_else(|| LineEditState::new(spelling).with_cursor_at_end())
    };
    let site = || {
        let path: SharedPath = Rc::from(path);
        progred_display::widget::Site {
            target: Hovered::Tree(Hover::Value(path.clone())),
            value,
            select: select_handler(path, hooks),
        }
    };
    let line = || {
        let writable = !cx.source.transient() && writable_at(&cx.sources, path);
        let selected = cx.selection.filter(|selection| {
            writable && selection.path() == path && selection.stage(&cx.sources) == Stage::Edge
        });
        progred_display::widget::LineSite {
            spelling: cx
                .scrub_spelling
                .filter(|(site, _)| *site == path)
                .map(|(_, text)| text),
            input: writable.then(|| {
                let edit = hooks.edit_line.clone();
                let path: SharedPath = Rc::from(path);
                let edit_path = path.clone();
                progred_display::widget::LineInput {
                    selected: selected.is_some(),
                    editing: selected.and_then(Selection::edit),
                    initial_text: &initial_text,
                    target: Hovered::Tree(Hover::Value(path.clone())),
                    select: select_handler(path, hooks),
                    edit: Rc::new(move |world, description, operation| {
                        edit(world, &edit_path, description, operation)
                    }),
                }
            }),
        }
    };
    let event_interpreter = || {
        let apply = hooks.apply.clone();
        let path = path.to_vec();
        Rc::new(move |world: &mut C, function: &Value, event| {
            apply(world, path.clone(), function.clone(), event)
        }) as progred_display::widget::EventInterpreter<C>
    };
    let annotate = || {
        let update = hooks.update_state.clone();
        let path = path.to_vec();
        Rc::new(move |world: &mut C, state| update(world, path.clone(), state))
            as progred_display::widget::Annotate<C>
    };
    widget(&mut progred_display::widget::Context {
        project,
        completion: &|text, kind, provider| match kind {
            progred_display::CompletionKind::Value => {
                pending_view(cx, text, path.to_vec(), provider, hooks)
            }
            progred_display::CompletionKind::Field => cx
                .pending_edge_under(path)
                .map(|(query, _)| label_query(cx, text, path, query, provider, hooks))
                .unwrap_or_else(|| render::text(text, "…", &cx.styles.dim)),
        },
        drawing: &|extent, fuel, program| {
            drawing::program_leaf(
                cx,
                path,
                extent.width,
                extent.ascent,
                extent.descent,
                fuel,
                program,
                hooks.select_source.clone(),
            )
        },
        text,
        styles: cx.styles,
        site: &site,
        line: &line,
        event_interpreter: &event_interpreter,
        annotate: &annotate,
        start_gesture: &|| {
            let start = hooks.start_gesture.clone();
            let path = path.to_vec();
            Rc::new(move |world, gesture, samples| start(world, path.clone(), gesture, samples))
        },
        value_edit: &|| {
            (!cx.source.transient() && writable_at(&cx.sources, path)).then(|| {
                let edit = hooks.value_edit.clone();
                let path = path.to_vec();
                Rc::new(move || edit(path.clone()))
                    as progred_display::widget::gesture::BeginEdit<C>
            })
        },
        drag_threshold: crate::gesture::DRAG_THRESHOLD,
        command: crate::modifiers::command,
        pick: hooks.pick.clone(),
        picking: |event| crate::modifiers::pick(&event.state.modifiers),
        same_target: PartialEq::eq,
        primary_edit: |event| {
            is_primary_contact(event) && !crate::modifiers::pick(&event.state.modifiers)
        },
    })
}

fn placeholder_box<C: 'static>(tcx: &mut TextCtx, styles: &Styles) -> Measured<Placed<C>> {
    progred_display::widget::empty(tcx, styles)
}

/// The pointer over this settled rect names `key`, with the visible
/// ink as its footprint. Placement order is precedence: descendants
/// and overlays contribute later and answer first.
fn hover_claim<C: 'static>(p: &mut placed::Builder<'_, '_, C>, placement: Placement, key: Hover) {
    p.claim(placement, Hovered::Tree(key));
}

/// An occluder: takes the pointer and names nothing, so targets
/// beneath an overlay never light.
fn hover_block<C: 'static>(p: &mut placed::Builder<'_, '_, C>, placement: Placement) {
    p.occlude(placement);
}

/// The pointer's preview of a click's meaning, washed faint.
fn hover_highlight<P: Canvas + ?Sized>(p: &mut P, outline: RoundedRect) {
    p.fill(
        outline,
        progred_display::widget::style::hover_wash(),
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
pub struct ProjectDescription<'a, World> {
    pub sources: Sources<'a>,
    pub root: Option<&'a Value>,
    pub root_path: &'a [Step],
    /// Selection belonging to this editable view.
    pub selection: Option<&'a Selection>,
    pub scrub_spelling: Option<(&'a [Step], &'a str)>,
    /// Selection from any view, used only to link generated output
    /// back to its structural source.
    pub source_selection: Option<&'a Selection>,
    pub annotations: &'a Annotations,
    pub raw: bool,
    pub styles: &'a Styles,
    pub width: f64,
    pub projection: Option<&'a Projection<World>>,
}

pub(crate) fn project<C: 'static>(
    description: ProjectDescription<'_, C>,
    tcx: &mut TextCtx,
    hooks: Hooks<C>,
) -> Measured<Placed<C>> {
    let width = description.width;
    resolve_choices(
        prepare_project(description, tcx, hooks),
        width,
        std::env::var_os("PROGRED_LAYOUT_TRACE").is_some(),
    )
}

fn prepare_project<C: 'static>(
    description: ProjectDescription<'_, C>,
    tcx: &mut TextCtx,
    hooks: Hooks<C>,
) -> ChoiceGraph<Placed<C>> {
    let ProjectDescription {
        sources,
        root,
        root_path,
        selection,
        scrub_spelling,
        source_selection,
        annotations,
        raw,
        styles,
        width: _,
        projection,
    } = description;
    let projection = projection.cloned().unwrap_or_default();
    let cx = Cx {
        sources,
        raw,
        annotations,
        styles,
        selection,
        scrub_spelling,
        source: Source::Stored,
        fuel: std::cell::Cell::new(grap::DEFAULT_FUEL),
        // Other projections of the selected cell are secondary. The
        // HOVERED value's faint marks come from the render pass's Ink.
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
        &hooks,
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
fn descend_landmark_with<C: 'static>(
    transient: bool,
    selected: bool,
    scale: f64,
    path: SharedPath,
    secondary: Option<(Secondary, bool)>,
    select: crate::navigate::Select<C>,
    delete: Rc<dyn Fn(&mut C, &[Descend<C>]) -> bool>,
    child: Measured<Placed<C>>,
) -> Measured<Placed<C>> {
    let highlight_path = path.clone();
    let marked = decorate(child, move |p, rect| {
        let highlight_path = highlight_path.clone();
        let outline = highlight_outline(scale, rect);
        p.ink(move |cv, ink| {
            if selected && !transient {
                primary_highlight(scale, cv, outline);
            } else if matches!(&secondary, Some((_, true))) {
                secondary_highlight(scale, cv, outline, true);
            } else if !transient
                && matches!(
                    tree_hovered(ink),
                    Some(Hover::Value(hovered)) if hovered.as_ref() == highlight_path.as_ref()
                )
            {
                hover_highlight(cv, outline);
            } else if secondary
                .as_ref()
                .is_some_and(|(secondary, _)| ink.hovered_secondary == Some(secondary))
            {
                secondary_highlight(scale, cv, outline, false);
            }
        });
    });
    if transient {
        return marked;
    }
    let marked = progred_display::widget::navigation::landmark(marked, path, select);
    if selected {
        bind_delete_with(delete, marked)
    } else {
        marked
    }
}

fn bind_delete_with<C: 'static>(
    delete: Rc<dyn Fn(&mut C, &[Descend<C>]) -> bool>,
    child: Measured<Placed<C>>,
) -> Measured<Placed<C>> {
    before(child, move |p, _| {
        p.handler().on_key_with(move |ctx, event, input| {
            crate::modifiers::plain(&event.modifiers)
                && matches!(
                    &event.key,
                    Key::Named(NamedKey::Backspace | NamedKey::Delete)
                )
                && event.state.is_down()
                && delete(ctx, &input.descends)
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

fn ground_with<C: 'static>(
    scale: f64,
    color: Color,
    content: Measured<Placed<C>>,
) -> Measured<Placed<C>> {
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
fn prepare_transient_root<C: 'static>(
    cx: &Cx,
    projection: &Projection<C>,
    tcx: &mut TextCtx,
    path: &[Step],
    result: Value,
    fuel: usize,
    hooks: &Hooks<C>,
    build: &mut ChoiceBuild<Placed<C>>,
) -> ChoiceLayout<Placed<C>> {
    let ordinary_projection = projection.without_entry();
    let projection = &ordinary_projection;
    let origin = path.to_vec();
    let select = hooks.select.clone();
    let select_origin = origin.clone();
    let select_payload = hooks.select_payload.clone();
    let payload_origin = origin.clone();
    let result_hooks = Hooks {
        completions: hooks.completions.clone(),
        select: Rc::new(move |ctx, _| select(ctx, select_origin.clone())),
        select_source: hooks.select_source.clone(),
        select_payload: Rc::new(move |ctx, _, payload| {
            select_payload(ctx, payload_origin.clone(), payload)
        }),
        edit_line: Rc::new(|_, _, _, _| false),
        toggle: Rc::new(|_, _| {}),
        update_state: hooks.update_state.clone(),
        edit: Rc::new(|_, _| false),
        pick: hooks.pick.clone(),
        insert: Rc::new(|_, _| {}),
        delete: Rc::new(|_, _| false),
        apply: hooks.apply.clone(),
        start_gesture: hooks.start_gesture.clone(),
        value_edit: hooks.value_edit.clone(),
        commit_value: hooks.commit_value.clone(),
        commit_label: hooks.commit_label.clone(),
        set_completion_view: hooks.set_completion_view.clone(),
    };
    let result_cx = Cx {
        sources: cx.sources,
        raw: false,
        annotations: cx.annotations,
        styles: cx.styles,
        selection: None,
        scrub_spelling: None,
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
        &result_hooks,
        build,
    )
}

/// Adds one GID step to the active source and resolves that location.
/// Current and descendant projections are independent replacements.
/// Missing locations use the standard empty picker, without a value.
#[allow(clippy::too_many_arguments)]
fn prepare_descend<C: 'static>(
    cx: &Cx,
    projection: &Projection<C>,
    tcx: &mut TextCtx,
    parent_path: &[Step],
    ancestors: &Ancestry,
    parent: Option<&Value>,
    step: Step,
    current_projection: Option<progred_display::Partial<C, Hovered>>,
    default_projection: Option<progred_display::Partial<C, Hovered>>,
    hooks: &Hooks<C>,
    build: &mut ChoiceBuild<Placed<C>>,
) -> ChoiceLayout<Placed<C>> {
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
            hooks,
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
            hooks,
            build,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn prepare_location<C: 'static>(
    cx: &Cx,
    projection: &Projection<C>,
    tcx: &mut TextCtx,
    path: &[Step],
    ancestors: &Ancestry,
    location: Location<'_>,
    current_projection: Option<&progred_display::Partial<C, Hovered>>,
    default_projection: Option<&progred_display::Partial<C, Hovered>>,
    hooks: &Hooks<C>,
    build: &mut ChoiceBuild<Placed<C>>,
) -> ChoiceLayout<Placed<C>> {
    prepare_value(
        cx,
        projection,
        current_projection,
        default_projection,
        tcx,
        path,
        ancestors,
        location.value(|cell, resolution| cx.sources.value(cell, resolution)),
        hooks,
        build,
    )
}

#[allow(clippy::too_many_arguments)]
fn prepare_value<C: 'static>(
    cx: &Cx,
    projection: &Projection<C>,
    current_projection: Option<&progred_display::Partial<C, Hovered>>,
    default_projection: Option<&progred_display::Partial<C, Hovered>>,
    tcx: &mut TextCtx,
    path: &[Step],
    ancestors: &Ancestry,
    value: Option<&Value>,
    hooks: &Hooks<C>,
    build: &mut ChoiceBuild<Placed<C>>,
) -> ChoiceLayout<Placed<C>> {
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
        hooks,
    );
    match layout {
        None => ChoiceLayout::fixed(pending_view(cx, tcx, path.to_vec(), None, hooks)),
        Some(layout) => {
            let inner = prepare(
                cx,
                child_projection,
                tcx,
                path,
                ancestors,
                hooks,
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
            let select = navigation_select_handler(landmark_path.clone(), hooks);
            let delete = hooks.delete.clone();
            let ground = value.and_then(|value| ground_decoration(cx, path, value));
            let target =
                value.map(|value| (value.clone(), hooks.pick.clone(), hooks.select.clone()));
            ChoiceLayout::map(inner, 0.0, move |inner| {
                let placed = descend_landmark_with(
                    transient,
                    selected,
                    scale,
                    landmark_path.clone(),
                    secondary,
                    select,
                    delete,
                    inner,
                );
                let grounded = match ground {
                    Some((scale, color)) => ground_with(scale, color, placed),
                    None => placed,
                };
                match target {
                    Some((value, pick, select)) => {
                        pick_target_with(landmark_path, value, pick, select, grounded)
                    }
                    None => grounded,
                }
            })
        }
    }
}

fn value_layout<C: 'static>(
    cx: &Cx,
    projection: &Projection<C>,
    current_projection: Option<&progred_display::Partial<C, Hovered>>,
    default_projection: &progred_display::Partial<C, Hovered>,
    path: &[Step],
    ancestors: &Ancestry,
    value: Option<&Value>,
    hooks: &Hooks<C>,
) -> Option<progred_display::Layout<C, Hovered>> {
    #[cfg(all(test, feature = "layout-profile"))]
    let _profile = progred_display::profile::enter(progred_display::profile::Kind::Projection);
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
        && let Some(collapsed) = structure::collapsed_layout(cx, path, value, hooks)
    {
        return Some(collapsed);
    }
    let selection = cx
        .selection
        .filter(|current| current.path() == path)
        .map(Selection::payload);
    let pending = if cx.pending_edge_under(path).is_some() {
        Some(progred_display::Pending::Field)
    } else {
        cx.pending_child_of(path)
            .map(progred_display::Pending::Child)
    };
    let state = cx.annotations.at(path);
    let target = |steps| projection_target(path, hooks, steps);
    let insert = |position| {
        let target: SharedPath = Rc::from(
            path.iter()
                .cloned()
                .chain([Step::Element(position)])
                .collect::<Path>(),
        );
        let insert = hooks.insert.clone();
        let destination = target.clone();
        let action: progred_display::ActionHandler<C> = Rc::new(move |world| {
            insert(world, destination.to_vec());
            true
        });
        (Hovered::Tree(Hover::Insert(target)), action)
    };
    let input = progred_display::ProjectionInput {
        env: &ProjectEnv { cx },
        default_projection: default_projection.clone(),
        value,
        scale_factor: cx.styles.scale,
        writable: !cx.source.transient() && writable_at(&cx.sources, path),
        selection: selection.as_ref(),
        pending,
        state,
        targets: progred_display::ProjectionTargets::new(&target).with_insert_after(&insert),
    };
    match current_projection {
        Some(current) => current(&input),
        None => projection.apply(&input),
    }
    .or_else(|| value.map(|value| structure::of(cx, path, value, hooks, &input)))
}

/// Every projected value's Pick backstop: pick the value into an open
/// pending, or — nothing pending — select it like Activate, so a stray
/// modifier never deadens the gesture. Inner Pick registrations answer
/// first and this catches what they refused.
fn pick_target_with<C: 'static>(
    path: SharedPath,
    value: Value,
    pick: Rc<dyn Fn(&mut C, Value) -> bool>,
    select: Rc<dyn Fn(&mut C, Path)>,
    child: Measured<Placed<C>>,
) -> Measured<Placed<C>> {
    before(child, move |p, _| {
        let pick = pick.clone();
        let select = select.clone();
        let path = path.clone();
        let value = value.clone();
        p.pick(Hovered::Tree(Hover::Value(path.clone())), move |world| {
            if !pick(world, value.clone()) {
                select(world, path.to_vec());
            }
            true
        });
    })
}

/// An editable atom's content: the selection's focused editor when
/// this atom is being edited — with `placeholder` as its ghost while
/// empty — its static text otherwise.
fn atom_content<C: 'static>(
    editing: Option<&LineEditState>,
    fallback: Measured<Placed<C>>,
    presentation: LineEditPresentation,
    placeholder: Option<(&str, &TextStyle)>,
    tcx: &mut TextCtx,
    styles: &Styles,
    hooks: &Hooks<C>,
) -> Measured<Placed<C>> {
    match editing {
        Some(line) => {
            let edit_ctx = hooks.edit.clone();
            render::text_edit(
                LineEditDescription {
                    state: line,
                    focused: true,
                    presentation,
                    style: &styles.edit,
                    placeholder,
                },
                tcx,
                move |c, operation| edit_ctx(c, operation),
            )
        }
        None => fallback,
    }
}
