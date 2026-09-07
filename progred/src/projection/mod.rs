//! The editor's tree-projection runtime: interpret display layouts,
//! retain source provenance, and fall back to total structural display.

mod completion;
mod drawing;
mod events;
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
use events::{
    realize_event_with, realize_point, realize_scrub, realize_state_drag, realize_state_scroll,
};
use gid::{CellId, Path, Step, Value};
use kurbo::{Affine, Insets, Point, RoundedRect, Stroke};
use location::Location;
use measured::choices::{ChoiceBuild, ChoiceLayout, resolve_choices};
use measured::{Measured, row};
use peniko::{Brush, Color};
use progred_display::widget::style::highlight_outline;
use puri::draw::Canvas;
use puri::edit::{LineEditDescription, LineEditPresentation, LineEditState};
use puri::geometry::Placement;
use puri::handler::HasHandler;
use puri::interact::is_primary_contact;
use puri::text::{TextCtx, TextStyle};
use puri_widgets::panel::Panel;
use std::collections::HashSet;
use std::rc::Rc;
use ui_events::keyboard::{Key, NamedKey};

type SharedPath = Rc<[Step]>;

/// One ordered composition of partial value projections. The
/// structural fallback lives in this runtime and is always total.
pub struct Projection<World> {
    partial: progred_display::Partial<World, Hover>,
    entry: Option<progred_display::Partial<World, Hover>>,
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
    pub fn new(partials: impl IntoIterator<Item = progred_display::Partial<World, Hover>>) -> Self {
        Self {
            partial: progred_display::compose_partials(partials),
            entry: None,
        }
    }

    /// Prepend a partial at entry, following cells to their definitions.
    /// Its children and computed results use the ordinary projection.
    pub fn with_entry(self, partial: progred_display::Partial<World, Hover>) -> Self {
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
        input: &progred_display::ProjectionInput<'_, World, Hover>,
    ) -> Option<progred_display::Layout<World, Hover>> {
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
struct Traversal {
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
    fn apply(&self, function: &Value, arguments: &[(CellId, Value)]) -> (Value, usize) {
        let fuel = if self.cx.source.transient() {
            self.cx.fuel.get()
        } else {
            grap::DEFAULT_FUEL
        };
        let evaluation = grap::apply(function, arguments.iter().cloned(), &self.cx.sources, fuel);
        self.cx.fuel.set(evaluation.remaining_fuel);
        (evaluation.result, evaluation.remaining_fuel)
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

/// Lower a projection layout to measured boxes. Puri leaves
/// become place-continuations; interaction nodes become Puri handlers.
#[allow(clippy::too_many_arguments)]
fn prepare<C: 'static, Cv: Canvas + 'static>(
    cx: &Cx,
    projection: &Projection<C>,
    tcx: &mut TextCtx,
    path: &[Step],
    ancestors: &Traversal,
    hooks: &Hooks<C>,
    value: Option<&Value>,
    layout: progred_display::Layout<C, Hover>,
    build: &mut ChoiceBuild<Placed<C, Cv>>,
) -> ChoiceLayout<Placed<C, Cv>> {
    let scale = cx.styles.scale;
    match layout {
        progred_display::Layout::Leaf(content) => {
            ChoiceLayout::fixed(leaf_display(cx.styles, tcx, content))
        }
        progred_display::Layout::DrawingProgram {
            width,
            ascent,
            descent,
            fuel,
            program,
        } => ChoiceLayout::fixed(drawing::program_leaf(
            cx,
            path,
            width,
            ascent,
            descent,
            fuel,
            program,
            hooks.select_source.clone(),
        )),
        progred_display::Layout::Completion { kind, provider } => ChoiceLayout::fixed(match kind {
            progred_display::CompletionKind::Value => {
                pending_view(cx, tcx, path.to_vec(), provider.as_ref(), hooks)
            }
            progred_display::CompletionKind::Field => cx
                .pending_edge_under(path)
                .map(|(query, _)| label_query(cx, tcx, path, query, provider.as_ref(), hooks))
                .unwrap_or_else(|| render::text(tcx, "…", &cx.styles.dim)),
        }),
        progred_display::Layout::Widget(widget) => ChoiceLayout::fixed(with_widget_context(
            cx,
            tcx,
            path,
            value,
            hooks,
            |context| native_fragment(widget(context)),
        )),
        progred_display::Layout::Before { child, before } => {
            let before =
                with_widget_context(cx, tcx, path, value, hooks, |context| before(context));
            let inner = prepare(
                cx, projection, tcx, path, ancestors, hooks, value, *child, build,
            );
            ChoiceLayout::map(inner, 0.0, move |inner| native_before(inner, before))
        }
        progred_display::Layout::OnEvent { child, handler } => {
            let inner = prepare(
                cx, projection, tcx, path, ancestors, hooks, value, *child, build,
            );
            let apply = hooks.apply.clone();
            let path = path.to_vec();
            ChoiceLayout::map(inner, 0.0, move |inner| {
                realize_event_with(path, handler, apply, scale, inner)
            })
        }
        progred_display::Layout::OnScrub {
            child,
            target,
            handler,
        } => {
            let inner = prepare(
                cx, projection, tcx, path, ancestors, hooks, value, *child, build,
            );
            let path = path.to_vec();
            let writable = !cx.source.transient() && writable_at(&cx.sources, &path);
            let start = hooks.scrub.clone();
            ChoiceLayout::map(inner, 0.0, move |inner| {
                if writable {
                    realize_scrub(path, target, handler, start, scale, inner)
                } else {
                    inner
                }
            })
        }
        progred_display::Layout::OnStateDrag {
            child,
            target,
            on_press,
            handler,
        } => {
            let inner = prepare(
                cx, projection, tcx, path, ancestors, hooks, value, *child, build,
            );
            let path = path.to_vec();
            let start = hooks.state_drag.clone();
            ChoiceLayout::map(inner, 0.0, move |inner| {
                realize_state_drag(path, target, on_press, handler, start, scale, inner)
            })
        }
        progred_display::Layout::OnStateScroll { child, handler } => {
            let inner = prepare(
                cx, projection, tcx, path, ancestors, hooks, value, *child, build,
            );
            let path = path.to_vec();
            let update_state = hooks.update_state.clone();
            ChoiceLayout::map(inner, 0.0, move |inner| {
                realize_state_scroll(path, handler, update_state, scale, inner)
            })
        }
        progred_display::Layout::OnPoint { child, handler } => {
            let inner = prepare(
                cx, projection, tcx, path, ancestors, hooks, value, *child, build,
            );
            let path = path.to_vec();
            let point = hooks.point.clone();
            let writable = !cx.source.transient() && writable_at(&cx.sources, &path);
            ChoiceLayout::map(inner, 0.0, move |inner| {
                if writable {
                    realize_point(path, handler, point, inner)
                } else {
                    inner
                }
            })
        }
        progred_display::Layout::Row {
            alignment,
            gap,
            children,
        } => {
            let children = children
                .into_iter()
                .map(|child| {
                    prepare(
                        cx, projection, tcx, path, ancestors, hooks, value, child, build,
                    )
                })
                .collect();
            ChoiceLayout::aligned_row(alignment, gap * scale, children)
        }
        progred_display::Layout::Col {
            baseline,
            gap,
            children,
        } => {
            let children = children
                .into_iter()
                .map(|child| {
                    prepare(
                        cx, projection, tcx, path, ancestors, hooks, value, child, build,
                    )
                })
                .collect();
            ChoiceLayout::col(baseline, gap * scale, children)
        }
        progred_display::Layout::Overlay { children } => ChoiceLayout::overlay(
            children
                .into_iter()
                .map(|child| {
                    prepare(
                        cx, projection, tcx, path, ancestors, hooks, value, child, build,
                    )
                })
                .collect(),
        ),
        progred_display::Layout::Popover { trigger, content } => {
            let trigger = prepare(
                cx, projection, tcx, path, ancestors, hooks, value, *trigger, build,
            );
            let content = prepare(
                cx, projection, tcx, path, ancestors, hooks, value, *content, build,
            );
            let panel = Panel {
                fill: Some(Color::new([0.985, 0.985, 0.99, 1.0]).into()),
                border: Some((Stroke::new(scale), cx.styles.dim.brush.clone())),
                radius: 6.0 * scale,
            };
            ChoiceLayout::attach(trigger, content, move |trigger, content| {
                let card = measured::pad(Insets::uniform(10.0 * scale), content);
                let card = before(card, move |p, placement| {
                    panel.place(p, placement);
                    p.occlude(placement);
                });
                placed::popover(trigger, card, 4.0 * scale)
            })
        }
        progred_display::Layout::Pad {
            left,
            top,
            right,
            bottom,
            child,
        } => {
            let insets = Insets::new(left * scale, top * scale, right * scale, bottom * scale);
            ChoiceLayout::pad(
                insets,
                prepare(
                    cx, projection, tcx, path, ancestors, hooks, value, *child, build,
                ),
            )
        }
        progred_display::Layout::Border { child } => {
            let inner = prepare(
                cx, projection, tcx, path, ancestors, hooks, value, *child, build,
            );
            let scale = cx.styles.scale;
            let brush = cx.styles.dim.brush.clone();
            ChoiceLayout::map(inner, 0.0, move |inner| bordered(scale, brush, inner))
        }
        progred_display::Layout::Surround { left, child, right } => {
            let (left, right) = with_widget_context(cx, tcx, path, value, hooks, |context| {
                (left(context), right(context))
            });
            let inner = prepare(
                cx, projection, tcx, path, ancestors, hooks, value, *child, build,
            );
            ChoiceLayout::map(
                inner,
                left.maximum_width + right.maximum_width,
                move |inner| {
                    let span = inner.extent;
                    row(
                        0.0,
                        vec![
                            native_fragment((left.measure)(span)),
                            inner,
                            native_fragment((right.measure)(span)),
                        ],
                    )
                },
            )
        }
        progred_display::Layout::Descend {
            step,
            projection: current_projection,
            default_projection,
        } => prepare_descend(
            cx,
            projection,
            tcx,
            path,
            ancestors,
            value,
            step,
            current_projection,
            default_projection,
            hooks,
            build,
        ),
        progred_display::Layout::At {
            steps,
            value: nested,
            projection: current_projection,
            default_projection,
        } => prepare_at(
            cx,
            projection,
            tcx,
            path,
            ancestors,
            steps,
            nested,
            current_projection,
            default_projection,
            hooks,
            build,
        ),
        progred_display::Layout::Transient {
            value: computed,
            fuel,
        } => prepare_transient_root(cx, projection, tcx, path, computed, fuel, hooks, build),
        progred_display::Layout::Shared { id: key, child } => build.shared(key, |build| {
            prepare(
                cx,
                projection,
                tcx,
                path,
                ancestors,
                hooks,
                value,
                child.as_ref().clone(),
                build,
            )
        }),
        progred_display::Layout::Alternatives(options) => {
            let options = options
                .into_iter()
                .map(|option| {
                    prepare(
                        cx, projection, tcx, path, ancestors, hooks, value, option, build,
                    )
                })
                .collect();
            build.alternatives(options)
        }
    }
}

fn prepare_at<C: 'static, Cv: Canvas + 'static>(
    cx: &Cx,
    projection: &Projection<C>,
    tcx: &mut TextCtx,
    path: &[Step],
    ancestors: &Traversal,
    steps: Vec<Step>,
    nested: Value,
    current_projection: Option<progred_display::Partial<C, Hover>>,
    default_projection: Option<progred_display::Partial<C, Hover>>,
    hooks: &Hooks<C>,
    build: &mut ChoiceBuild<Placed<C, Cv>>,
) -> ChoiceLayout<Placed<C, Cv>> {
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

fn leaf_display<C: 'static, Cv: Canvas + 'static>(
    styles: &Styles,
    tcx: &mut TextCtx,
    content: puri::Leaf<progred_display::Paint>,
) -> Measured<Placed<C, Cv>> {
    match content {
        puri::Leaf::Text {
            text,
            paint,
            script,
        } => {
            let style = match paint {
                progred_display::Paint::Face(face) => face_style(styles, face).clone(),
                progred_display::Paint::Brush(brush) => TextStyle {
                    brush,
                    ..styles.name.clone()
                },
            };
            render::shaped_text(puri::text::scripted_text(tcx, &text, &style, script))
        }
        puri::Leaf::Drawing(drawing) => drawing_leaf(styles, drawing),
    }
}

fn drawing_leaf<C: 'static, Cv: Canvas + 'static>(
    styles: &Styles,
    drawing: puri::Drawing<progred_display::Paint>,
) -> Measured<Placed<C, Cv>> {
    render::drawing(
        drawing.map_paint(|paint| match paint {
            progred_display::Paint::Face(face) => face_style(styles, face).brush.clone(),
            progred_display::Paint::Brush(brush) => brush,
        }),
        styles.scale,
    )
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
    /// Begin a continuous point control at its settled placement.
    pub point: Rc<dyn Fn(&mut C, Path, Placement, progred_display::PointHandler, Point) -> bool>,
    /// Begin a projection state drag in this hook's owning view.
    pub state_drag: Rc<dyn Fn(&mut C, Path, progred_display::StateDragHandler, Point, f64)>,
    /// Select and begin a value scrub, declining while a pending is active.
    pub scrub: Rc<dyn Fn(&mut C, Path, progred_display::ScrubHandler, Point, f64) -> bool>,
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
) -> progred_display::ProjectionTarget<C, Hover> {
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
        hover: Hover::Value(path),
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
    widget: impl FnOnce(&mut progred_display::widget::Context<'_, '_, C, Hover>) -> Result,
) -> Result {
    let initial_text = |spelling: &str| {
        cx.selection
            .filter(|selection| selection.path() == path)
            .map(|selection| selection.initial_line(spelling))
            .unwrap_or_else(|| LineEditState::new(spelling).with_cursor_at_end())
    };
    let site = || {
        let writable = !cx.source.transient() && writable_at(&cx.sources, path);
        let selected = cx.selection.filter(|selection| {
            writable && selection.path() == path && selection.stage(&cx.sources) == Stage::Edge
        });
        let edit = hooks.edit_line.clone();
        let path: SharedPath = Rc::from(path);
        let edit_path = path.clone();
        progred_display::widget::Site {
            writable,
            selected: selected.is_some(),
            editing: selected.and_then(Selection::edit),
            initial_text: &initial_text,
            spelling: cx
                .scrub_spelling
                .filter(|(site, _)| *site == path.as_ref())
                .map(|(_, text)| text),
            target: Hover::Value(path.clone()),
            select: select_handler(path, hooks),
            value,
            edit: Rc::new(move |world, description, operation| {
                edit(world, &edit_path, description, operation)
            }),
        }
    };
    widget(&mut progred_display::widget::Context {
        text,
        styles: cx.styles,
        site: &site,
        pick: hooks.pick.clone(),
        picking: |event| crate::modifiers::pick(&event.state.modifiers),
        same_target: PartialEq::eq,
        primary_edit: |event| {
            is_primary_contact(event) && !crate::modifiers::pick(&event.state.modifiers)
        },
    })
}

fn native_fragment<C: 'static, Cv: Canvas + 'static>(
    measured: Measured<progred_display::widget::Fragment<C, Hover>>,
) -> Measured<Placed<C, Cv>> {
    placed::leaf(measured.extent, move |output, placement| {
        output.fragment(measured::place(measured, placement));
    })
}

fn native_before<C: 'static, Cv: Canvas + 'static>(
    child: Measured<Placed<C, Cv>>,
    place: progred_display::widget::Place<C, Hover>,
) -> Measured<Placed<C, Cv>> {
    before(child, move |output, placement| {
        let mut fragment =
            <progred_display::widget::Fragment<C, Hover> as measured::Output>::empty();
        place(&mut fragment, placement);
        output.fragment(fragment);
    })
}

fn face_style(styles: &Styles, face: progred_display::Face) -> &TextStyle {
    match face {
        progred_display::Face::Name => &styles.name,
        progred_display::Face::String => &styles.string,
        progred_display::Face::Dim => &styles.dim,
        progred_display::Face::Label => &styles.label,
        progred_display::Face::Id => &styles.id,
        progred_display::Face::AccentWash => &styles.accent_wash,
        progred_display::Face::Ink => &styles.ink,
    }
}

fn bordered<C: 'static, Cv: Canvas + 'static>(
    scale: f64,
    brush: Brush,
    child: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    let panel = Panel {
        fill: None,
        border: Some((Stroke::new(scale), brush)),
        radius: 0.0,
    };
    measured::around(child, move |placement, inner| {
        let mut placed = inner.place();
        if !placement.clipped_out() {
            let placement = Placement::new(placement.rect.inset(-0.5 * scale), placement.clip_rect);
            placed
                .renders
                .push(Box::new(move |cv, _| panel.place(cv, placement)));
        }
        placed
    })
}

fn placeholder_box<C: 'static, Cv: Canvas + 'static>(
    tcx: &mut TextCtx,
    styles: &Styles,
) -> Measured<Placed<C, Cv>> {
    native_fragment(progred_display::widget::empty(tcx, styles))
}

/// The pointer over this settled rect names `key`, with the visible
/// ink as its footprint. Placement order is precedence: descendants
/// and overlays contribute later and answer first.
fn hover_claim<C: 'static, Cv: 'static>(
    p: &mut placed::Builder<'_, C, Cv>,
    placement: Placement,
    key: Hover,
) {
    p.claim(placement, Hovered::Tree(key));
}

/// An occluder: takes the pointer and names nothing, so targets
/// beneath an overlay never light.
fn hover_block<C: 'static, Cv: 'static>(p: &mut placed::Builder<'_, C, Cv>, placement: Placement) {
    p.occlude(placement);
}

/// The pointer's preview of a click's meaning, washed faint.
fn hover_highlight<P: Canvas>(p: &mut P, outline: RoundedRect) {
    p.fill(
        outline,
        progred_display::widget::style::hover_wash(),
        Affine::IDENTITY,
    );
}

/// The pane-local primary: translucent system blue, like the Swift
/// version's selection, ringed at full strength — the strongest mark
/// in the shared vocabulary.
fn primary_highlight<P: Canvas>(scale: f64, p: &mut P, outline: RoundedRect) {
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

pub(crate) fn project<C: 'static, Cv: Canvas + 'static>(
    description: ProjectDescription<'_, C>,
    tcx: &mut TextCtx,
    hooks: Hooks<C>,
) -> Measured<Placed<C, Cv>> {
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
        width,
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
    let mut traversal = Traversal::default();
    if let Some(Step::Follow(source)) = root_path.last()
        && let Some(cell) = sources
            .resolve_path(&root_path[..root_path.len() - 1])
            .and_then(Value::as_cell)
    {
        traversal.cells.insert(cell);
        traversal.enclosing = Some((cell, *source, root_path.len()));
    }
    let layout = prepare_location(
        &cx,
        &projection,
        tcx,
        root_path,
        &traversal,
        Location::Root(root),
        None,
        None,
        &hooks,
        &mut build,
    );
    resolve_choices(
        build.finish(layout),
        width,
        std::env::var_os("PROGRED_LAYOUT_TRACE").is_some(),
    )
}

/// Marks `child` as the projection of `path` WITHOUT claiming any
/// clicks: the highlight, reveal rect, and keyboard reach of
/// [`descend`] over the full bounds, while pointer selection belongs
/// to the content targets the view registers — heads, delimiters,
/// rows — so clicks on structural whitespace (gutters, inter-row
/// gaps, the dead space inside a bounding box) fall through to the
/// background's deselect.
#[allow(clippy::too_many_arguments)]
fn descend_landmark_with<C: 'static, Cv: Canvas + 'static>(
    transient: bool,
    selected: bool,
    scale: f64,
    path: SharedPath,
    secondary: Option<(Secondary, bool)>,
    select: crate::navigate::Select<C>,
    delete: Rc<dyn Fn(&mut C, &[Descend<C>]) -> bool>,
    child: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
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
    let marked = measured::around_into(
        marked,
        move |placement, inner, placed: &mut Placed<C, Cv>| {
            let outer_select = placed.landmark_select.take();
            inner.place_into(placed);
            let select = placed.landmark_select.take().unwrap_or(select);
            placed.landmark_select = outer_select;
            placed.descends.push(Descend {
                root: None,
                path,
                rect: placement.rect,
                select,
            });
        },
    );
    if selected {
        bind_delete_with(delete, marked)
    } else {
        marked
    }
}

fn bind_delete_with<C: 'static, Cv: Canvas + 'static>(
    delete: Rc<dyn Fn(&mut C, &[Descend<C>]) -> bool>,
    child: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
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

fn ground_with<C: 'static, Cv: Canvas + 'static>(
    scale: f64,
    color: Color,
    content: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    decorate(content, move |p, rect| {
        let bg = RoundedRect::from_rect(rect.inset(3.0 * scale), 5.0 * scale);
        p.fill(bg, color, Affine::IDENTITY);
    })
}

fn secondary_highlight<P: Canvas>(scale: f64, p: &mut P, outline: RoundedRect, strong: bool) {
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
fn prepare_transient_root<C: 'static, Cv: Canvas + 'static>(
    cx: &Cx,
    projection: &Projection<C>,
    tcx: &mut TextCtx,
    path: &[Step],
    result: Value,
    fuel: usize,
    hooks: &Hooks<C>,
    build: &mut ChoiceBuild<Placed<C, Cv>>,
) -> ChoiceLayout<Placed<C, Cv>> {
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
        point: hooks.point.clone(),
        state_drag: hooks.state_drag.clone(),
        scrub: hooks.scrub.clone(),
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
        &Traversal::default(),
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
fn prepare_descend<C: 'static, Cv: Canvas + 'static>(
    cx: &Cx,
    projection: &Projection<C>,
    tcx: &mut TextCtx,
    parent_path: &[Step],
    ancestors: &Traversal,
    parent: Option<&Value>,
    step: Step,
    current_projection: Option<progred_display::Partial<C, Hover>>,
    default_projection: Option<progred_display::Partial<C, Hover>>,
    hooks: &Hooks<C>,
    build: &mut ChoiceBuild<Placed<C, Cv>>,
) -> ChoiceLayout<Placed<C, Cv>> {
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
fn prepare_location<C: 'static, Cv: Canvas + 'static>(
    cx: &Cx,
    projection: &Projection<C>,
    tcx: &mut TextCtx,
    path: &[Step],
    ancestors: &Traversal,
    location: Location<'_>,
    current_projection: Option<&progred_display::Partial<C, Hover>>,
    default_projection: Option<&progred_display::Partial<C, Hover>>,
    hooks: &Hooks<C>,
    build: &mut ChoiceBuild<Placed<C, Cv>>,
) -> ChoiceLayout<Placed<C, Cv>> {
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
fn prepare_value<C: 'static, Cv: Canvas + 'static>(
    cx: &Cx,
    projection: &Projection<C>,
    current_projection: Option<&progred_display::Partial<C, Hover>>,
    default_projection: Option<&progred_display::Partial<C, Hover>>,
    tcx: &mut TextCtx,
    path: &[Step],
    ancestors: &Traversal,
    value: Option<&Value>,
    hooks: &Hooks<C>,
    build: &mut ChoiceBuild<Placed<C, Cv>>,
) -> ChoiceLayout<Placed<C, Cv>> {
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
            let landmark = landmark_path.clone();
            let placed = ChoiceLayout::map(inner, 0.0, move |inner| {
                descend_landmark_with(
                    transient, selected, scale, landmark, secondary, select, delete, inner,
                )
            });
            let grounded = match value.and_then(|value| ground_decoration(cx, path, value)) {
                Some((scale, color)) => {
                    ChoiceLayout::map(placed, 0.0, move |placed| ground_with(scale, color, placed))
                }
                None => placed,
            };
            match value {
                Some(value) => {
                    let target_path = landmark_path;
                    let target_value = value.clone();
                    let pick = hooks.pick.clone();
                    let select = hooks.select.clone();
                    ChoiceLayout::map(grounded, 0.0, move |grounded| {
                        pick_target_with(target_path, target_value, pick, select, grounded)
                    })
                }
                None => grounded,
            }
        }
    }
}

fn value_layout<C: 'static>(
    cx: &Cx,
    projection: &Projection<C>,
    current_projection: Option<&progred_display::Partial<C, Hover>>,
    default_projection: &progred_display::Partial<C, Hover>,
    path: &[Step],
    ancestors: &Traversal,
    value: Option<&Value>,
    hooks: &Hooks<C>,
) -> Option<progred_display::Layout<C, Hover>> {
    // Traversal has already accumulated the cells crossed by Follow
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
        (Hover::Insert(target), action)
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
fn pick_target_with<C: 'static, Cv: Canvas + 'static>(
    path: SharedPath,
    value: Value,
    pick: Rc<dyn Fn(&mut C, Value) -> bool>,
    select: Rc<dyn Fn(&mut C, Path)>,
    child: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
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
fn atom_content<C: 'static, Cv: Canvas + 'static>(
    editing: Option<&LineEditState>,
    fallback: Measured<Placed<C, Cv>>,
    presentation: LineEditPresentation,
    placeholder: Option<(&str, &TextStyle)>,
    tcx: &mut TextCtx,
    styles: &Styles,
    hooks: &Hooks<C>,
) -> Measured<Placed<C, Cv>> {
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
