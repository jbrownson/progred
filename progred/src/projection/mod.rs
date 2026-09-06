//! The editor's tree-projection runtime: interpret display layouts,
//! retain source provenance, and fall back to total structural display.

mod choices;
mod completion;
mod drawing;
mod events;
mod location;
mod structure;
#[cfg(test)]
mod tests;

use crate::annotations::Annotations;
use crate::frame::Hovered;
use crate::hover::{Hover, Secondary, SourceTrace};
use crate::navigate::Descend;
use crate::placed::{self, Placed, before, decorate, leaf};
use crate::render;
use crate::selection::{Selection, Stage, last_follow, writable_at};
use crate::sources::Sources;
use crate::styles::Styles;
use choices::{ChoiceBuild, ChoiceGraph, ChoiceLayout, resolve_choices};
use completion::{label_query, pending_view};
use events::{
    realize_activate, realize_click, realize_event_with, realize_point, realize_scrub,
    realize_state_drag, realize_state_scroll,
};
use gid::{CellId, Path, Step, Value};
use kurbo::{Affine, Insets, Point, Rect, RoundedRect, Stroke};
use location::Location;
use measured::{Extent, Measured, pad, row};
use peniko::{Brush, Color};
use puri::delim;
use puri::draw::Canvas;
use puri::edit::{
    EditCtx, LineEditDescription, LineEditPointerDown, LineEditPresentation, LineEditState,
};
use puri::geometry::Placement;
use puri::handler::HasHandler;
use puri::interact::is_primary_contact;
use puri::text::{TextCtx, TextStyle};
use puri_widgets::panel::Panel;
use puri_widgets::text_frame;
use std::collections::HashSet;
use std::rc::Rc;
use ui_events::keyboard::{Key, NamedKey};

type SharedPath = Rc<[Step]>;

/// One ordered composition of partial value projections. The
/// structural fallback lives in this runtime and is always total.
pub struct Projection<World> {
    partials: Rc<[progred_display::Partial<World, Hover>]>,
    entry: Option<progred_display::Partial<World, Hover>>,
}

impl<World> Clone for Projection<World> {
    fn clone(&self) -> Self {
        Self {
            partials: self.partials.clone(),
            entry: self.entry.clone(),
        }
    }
}

impl<World> Default for Projection<World> {
    fn default() -> Self {
        Self {
            partials: Rc::new([]),
            entry: None,
        }
    }
}

impl<World> Projection<World> {
    pub fn new(partials: impl IntoIterator<Item = progred_display::Partial<World, Hover>>) -> Self {
        Self {
            partials: partials.into_iter().collect(),
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
            partials: self.partials.clone(),
            entry: None,
        }
    }

    fn apply(
        &self,
        input: &progred_display::ProjectionInput<'_, World, Hover>,
    ) -> Option<progred_display::Layout<World, Hover>> {
        self.entry
            .iter()
            .chain(self.partials.iter())
            .find_map(|partial| partial(input))
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

    fn cell_definition(&self, cell: CellId) -> Option<(gid::Resolution, &Value)> {
        self.cx
            .sources
            .resolve(cell)
            .map(|value| (value.source, value.value))
    }

    fn foreign_source(&self, cell: CellId) -> Option<gid::Resolution> {
        match grap::Host::resolve(&self.cx.sources, cell) {
            Some((source, grap::Definition::Foreign(_))) => Some(source),
            _ => None,
        }
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
    projection: Option<&Projection<C>>,
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
        progred_display::Layout::EmptySlot => ChoiceLayout::fixed(placeholder_box(tcx, cx.styles)),
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
        progred_display::Layout::LineEdit(mut line) => {
            if let Some((scrub_path, spelling)) = cx.scrub_spelling
                && scrub_path == path
            {
                line.text = spelling.to_owned();
            }
            ChoiceLayout::fixed(line_edit_view(cx, tcx, path, line, hooks))
        }
        progred_display::Layout::OnClick { child, handler } => {
            let inner = prepare(
                cx, projection, tcx, path, ancestors, hooks, value, *child, build,
            );
            ChoiceLayout::map(inner, 0.0, move |inner| realize_click(handler, inner))
        }
        progred_display::Layout::OnActivate {
            child,
            target,
            handler,
        } => {
            let inner = prepare(
                cx, projection, tcx, path, ancestors, hooks, value, *child, build,
            );
            ChoiceLayout::map(inner, 0.0, move |inner| {
                realize_activate(target, handler, inner)
            })
        }
        progred_display::Layout::OnPick {
            child,
            target,
            value: picked,
        } => {
            let inner = prepare(
                cx, projection, tcx, path, ancestors, hooks, value, *child, build,
            );
            let pick = hooks.pick.clone();
            ChoiceLayout::map(inner, 0.0, move |inner| {
                realize_pick_with(target, picked, pick, inner)
            })
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
        progred_display::Layout::OnHover { child, hover } => {
            let inner = prepare(
                cx, projection, tcx, path, ancestors, hooks, value, *child, build,
            );
            ChoiceLayout::map(inner, 0.0, move |inner| realize_hover(scale, hover, inner))
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
            ChoiceLayout::popover(trigger, content, move |trigger, content| {
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
            let gap = 2.0 * scale;
            let reserved = side_advance(scale, &left) + side_advance(scale, &right) + 2.0 * gap;
            let inner = prepare(
                cx, projection, tcx, path, ancestors, hooks, value, *child, build,
            );
            let path = path.to_vec();
            let target = value.cloned();
            let dim = cx.styles.dim.brush.clone();
            let select = hooks.select.clone();
            let pick = hooks.pick.clone();
            ChoiceLayout::map(inner, reserved, move |inner| {
                surround_sides(scale, dim, path, target, select, pick, left, inner, right)
            })
        }
        progred_display::Layout::Descend {
            step,
            projection: contextual_partials,
            missing,
        } => prepare_descend(
            cx,
            projection,
            tcx,
            path,
            ancestors,
            value,
            step,
            contextual_partials,
            missing.map(|missing| *missing),
            hooks,
            build,
        ),
        progred_display::Layout::At {
            steps,
            value: nested,
            projection: override_partials,
        } => prepare_at(
            cx,
            projection,
            tcx,
            path,
            ancestors,
            steps,
            nested,
            override_partials,
            hooks,
            build,
        ),
        progred_display::Layout::Transient {
            value: computed,
            fuel,
        } => prepare_transient_root(cx, projection, tcx, path, computed, fuel, hooks, build),
        progred_display::Layout::Shared { id: key, child } => {
            if let Some(id) = build.shared_ids.get(&key).copied() {
                let widths = build.shared[id]
                    .as_ref()
                    .expect("a shared layout is prepared before reuse")
                    .widths;
                ChoiceLayout::used(id, widths)
            } else {
                let prepared = prepare(
                    cx,
                    projection,
                    tcx,
                    path,
                    ancestors,
                    hooks,
                    value,
                    child.as_ref().clone(),
                    build,
                );
                let id = build.shared.len();
                let widths = prepared.widths;
                build.shared.push(Some(prepared));
                build.shared_ids.insert(key, id);
                ChoiceLayout::used(id, widths)
            }
        }
        progred_display::Layout::Alternatives(options) => {
            let id = build.next_choice;
            build.next_choice += 1;
            ChoiceLayout::alternatives(
                id,
                options
                    .into_iter()
                    .map(|option| {
                        prepare(
                            cx, projection, tcx, path, ancestors, hooks, value, option, build,
                        )
                    })
                    .collect(),
            )
        }
    }
}

fn prepare_at<C: 'static, Cv: Canvas + 'static>(
    cx: &Cx,
    projection: Option<&Projection<C>>,
    tcx: &mut TextCtx,
    path: &[Step],
    ancestors: &Traversal,
    steps: Vec<Step>,
    nested: Value,
    override_partials: Option<Vec<progred_display::Partial<C, Hover>>>,
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
    let override_projection = contextual_projection(projection, override_partials);
    prepare_present_value(
        cx,
        override_projection.as_ref().or(projection),
        tcx,
        &path,
        &follow_ancestors,
        &nested,
        hooks,
        build,
    )
}

fn contextual_projection<C>(
    ambient: Option<&Projection<C>>,
    contextual: Option<Vec<progred_display::Partial<C, Hover>>>,
) -> Option<Projection<C>> {
    contextual.map(|partials| {
        Projection::new(
            partials.into_iter().chain(
                ambient
                    .into_iter()
                    .flat_map(|projection| projection.partials.iter().cloned()),
            ),
        )
    })
}

fn realize_pick_with<C: 'static, Cv: Canvas + 'static>(
    target: Hover,
    picked: Value,
    pick: Rc<dyn Fn(&mut C, Value) -> bool>,
    inner: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    before(inner, move |p, _| {
        p.pick(Hovered::Tree(target), move |world| {
            pick(world, picked.clone())
        });
    })
}

fn realize_hover<C: 'static, Cv: Canvas + 'static>(
    scale: f64,
    hover: Option<Hover>,
    inner: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    let highlight = matches!(hover.as_ref(), Some(Hover::Toggle(_) | Hover::Insert(_)));
    before(inner, move |p, placement| match hover {
        Some(hover) => {
            if highlight {
                light_hover(p, placement, hover, scale);
            } else {
                hover_claim(p, placement, hover);
            }
        }
        None => hover_block(p, placement),
    })
}

/// Claim `hover` and, when it is the resolved hover, wash the box.
fn light_hover<C: 'static, Cv: Canvas + 'static>(
    p: &mut placed::Builder<'_, C, Cv>,
    placement: Placement,
    hover: Hover,
    scale: f64,
) {
    let mine = hover.clone();
    p.ink(move |cv, ink| {
        if tree_hovered(ink) == Some(&mine) {
            hover_highlight(scale, cv, placement.rect);
        }
    });
    hover_claim(p, placement, hover);
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

/// Lower the stock Rust line control. An inactive line owns the raw
/// click that mounts its editor and places the caret; once active,
/// Puri's line editor owns pointer, keyboard, and IME dispatch. The
/// projection still supplies the Grap update rule used at write-back.
fn line_edit_view<C: 'static, Cv: Canvas + 'static>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: &[Step],
    line: progred_display::LineEdit,
    hooks: &Hooks<C>,
) -> Measured<Placed<C, Cv>> {
    let writable = !cx.source.transient() && writable_at(&cx.sources, path);
    let editing = if writable {
        cx.selection
            .filter(|selection| selection.path() == path)
            .and_then(Selection::edit)
    } else {
        None
    };
    let active = editing.is_some();
    let edit = hooks.edit.clone();
    let content = render::line_edit(tcx, cx.styles, &line, editing, move |ctx| edit(ctx));

    if !writable {
        return content;
    }

    let path: SharedPath = Rc::from(path);
    let select_path = path.clone();
    let select_line = line.clone();
    let start_edit = hooks.start_edit.clone();
    let select: progred_display::ActionHandler<C> = Rc::new(move |ctx| {
        start_edit(ctx, select_path.to_vec(), select_line.clone());
        true
    });
    let content = before(content, move |p, _| p.select_landmark(select));
    let presentation = cx.styles.line_presentation(&line);
    let scale = cx.styles.scale;
    let start_edit = hooks.start_edit.clone();
    let edit = hooks.edit.clone();
    before(content, move |p, placement| {
        hover_claim(p, placement, Hover::Value(path.clone()));
        let path = path.clone();
        let line = line.clone();
        let presentation = presentation.clone();
        let start_edit = start_edit.clone();
        let edit = edit.clone();
        p.handler().on_pointer_down(move |ctx, event| {
            is_primary_contact(event)
                && !crate::modifiers::pick(&event.state.modifiers)
                && placement.contains(Point::new(event.state.position.x, event.state.position.y))
                && {
                    if !active {
                        start_edit(ctx, path.to_vec(), line.clone());
                    }
                    if let Some(edit) = edit(ctx) {
                        edit.state.pointer_down(
                            &presentation,
                            edit.fonts,
                            edit.layouts,
                            scale as f32,
                            LineEditPointerDown {
                                point: Point::new(
                                    event.state.position.x - placement.rect.x0,
                                    event.state.position.y - placement.rect.y0,
                                ),
                                shift: event.state.modifiers.shift(),
                                count: event.state.count.max(1),
                            },
                        );
                    }
                    true
                }
        });
    })
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
    /// Mount the stock editor described by a Rust projection. Its
    /// first pointer event then uses `edit` below for caret placement.
    pub start_edit: Rc<dyn Fn(&mut C, Path, progred_display::LineEdit)>,
    pub toggle: Rc<dyn Fn(&mut C, Path)>,
    /// Replace the annotation value at one projection site. The
    /// concrete view root remains host-owned and closed over here.
    pub update_state: Rc<dyn Fn(&mut C, Path, Value) -> bool>,
    /// None when the editor is already gone — retained-frame dispatch
    /// may fire a frame late, and absent state declines.
    pub edit: Rc<dyn for<'a> Fn(&'a mut C) -> Option<EditCtx<'a>>>,
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
        self.selection
            .is_some_and(|current| current.stage() != Stage::Label && current.path() == path)
    }

    /// The pending child step under `path`, when the selection is
    /// authoring one there.
    fn pending_child_of(&self, path: &[Step]) -> Option<Step> {
        let current = self.selection?;
        (current.stage() == Stage::Pending
            && current
                .path()
                .split_last()
                .is_some_and(|(_, parent)| parent == path))
        .then(|| current.path().last().cloned())
        .flatten()
    }

    /// The label query of a new field being authored on the record at
    /// `path`.
    fn pending_edge_under(&self, path: &[Step]) -> Option<(&LineEditState, usize)> {
        let current = self.selection?;
        (current.stage() == Stage::Label && current.path() == path)
            .then(|| Some((current.edit()?, current.choice())))
            .flatten()
    }
}

fn edit_presentation(style: &TextStyle) -> LineEditPresentation {
    LineEditPresentation::new(style.size, style.brush.clone())
}

fn side_advance(scale: f64, ink: &progred_display::Ink) -> f64 {
    match ink {
        progred_display::Ink::Delim { delim, .. } => delim::maximum_advance(*delim, 14.0 * scale),
    }
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

fn ink_leaf<C: 'static, Cv: Canvas + 'static>(
    scale: f64,
    brush: Brush,
    ink: progred_display::Ink,
    content: Extent,
) -> Measured<Placed<C, Cv>> {
    match ink {
        progred_display::Ink::Delim { delim, side } => render::drawing(
            delim::stretched(
                delim,
                side,
                14.0 * scale,
                content.ascent,
                content.descent,
                brush,
            ),
            1.0,
        ),
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

/// Place `left` and `right` in the side columns of `content`: same
/// height as the child, with their width grown from that height. The
/// display nodes paint; this only allocates and keeps the sides as
/// handles.
fn surround_sides<C: 'static, Cv: Canvas + 'static>(
    scale: f64,
    brush: Brush,
    path: Path,
    target: Option<Value>,
    select: Rc<dyn Fn(&mut C, Path)>,
    pick: Rc<dyn Fn(&mut C, Value) -> bool>,
    left: progred_display::Ink,
    content: Measured<Placed<C, Cv>>,
    right: progred_display::Ink,
) -> Measured<Placed<C, Cv>> {
    let extent = content.extent;
    let gap = 2.0 * scale;
    let path: SharedPath = Rc::from(path);
    row(
        0.0,
        vec![
            select_target_with(
                path.clone(),
                target.clone(),
                select.clone(),
                pick.clone(),
                pad(
                    Insets::new(0.0, 0.0, gap, 0.0),
                    ink_leaf(scale, brush.clone(), left, extent),
                ),
            ),
            content,
            select_target_with(
                path,
                target,
                select,
                pick,
                pad(
                    Insets::new(gap, 0.0, 0.0, 0.0),
                    ink_leaf(scale, brush, right, extent),
                ),
            ),
        ],
    )
}

fn placeholder_box<C: 'static, Cv: Canvas + 'static>(
    tcx: &mut TextCtx,
    styles: &Styles,
) -> Measured<Placed<C, Cv>> {
    let frame = text_frame::empty(tcx, &styles.label, styles.dim.brush.clone());
    leaf(
        placed::metrics_extent(frame.metrics()),
        move |p, placement| frame.place(p, placement),
    )
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

/// The pointer's preview of a click's meaning: the same box the
/// primary would ring, washed faint — hover never outranks selection.
fn hover_highlight<P: Canvas>(scale: f64, p: &mut P, rect: Rect) {
    p.fill(
        text_frame::outline(scale, rect),
        Color::new([0.0, 0.48, 1.0, 0.08]),
        Affine::IDENTITY,
    );
}

/// The pane-local primary: translucent system blue, like the Swift
/// version's selection, ringed at full strength — the strongest mark
/// in the shared vocabulary.
fn primary_highlight<P: Canvas>(scale: f64, p: &mut P, rect: Rect) {
    let bg = text_frame::outline(scale, rect);
    p.fill(bg, Color::new([0.0, 0.48, 1.0, 0.22]), Affine::IDENTITY);
    p.stroke(
        bg,
        Stroke::new(2.5 * scale),
        Color::new([0.0, 0.48, 1.0, 1.0]),
        Affine::IDENTITY,
    );
}

/// Marks CONTENT-SHAPED `child` as the projection of the value at
/// `path` — its bounding box is all ink (a pending's query, an
/// engaged name, the empty-document placeholder), so the whole box
/// is an honest hover target. On placement it draws the highlight,
/// registers Activate and Pick actions addressed to that hover, and
/// records itself for keyboard navigation. Views whose boxes span
/// structural whitespace use [`descend_landmark`] plus explicit
/// content claims instead.
fn source_target<C: 'static, Cv: Canvas + 'static>(
    cx: &Cx,
    path: Path,
    value: Option<Value>,
    hooks: &Hooks<C>,
    child: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    let (path, transient): (SharedPath, bool) = match cx.source {
        Source::Transient { owner } if owner != path.as_slice() => return child,
        Source::Transient { owner } => (Rc::from(owner), true),
        Source::Stored => (Rc::from(path), false),
    };
    let scale = cx.styles.scale;
    let selected = cx.selected(path.as_ref());
    let select = hooks.select.clone();
    let pick = hooks.pick.clone();
    before(child, move |p, placement| {
        let rect = placement.rect;
        let highlight_path = path.clone();
        p.ink(move |cv, ink| {
            if selected {
                primary_highlight(scale, cv, rect);
            } else if matches!(
                tree_hovered(ink),
                Some(Hover::Value(hovered)) if hovered.as_ref() == highlight_path.as_ref()
            ) {
                hover_highlight(scale, cv, rect);
            }
        });
        if !transient {
            hover_claim(p, placement, Hover::Value(path.clone()));
        }
        let select = select.clone();
        let pick = pick.clone();
        let target = path.clone();
        let value = value.clone();
        let action_target = Hovered::Tree(Hover::Value(target.clone()));
        let activate_select = select.clone();
        p.activate(action_target.clone(), move |ctx| {
            activate_select(ctx, target.to_vec());
            true
        });
        if let Some(value) = value {
            p.pick(action_target, move |ctx| pick(ctx, value.clone()));
        }
        if !transient {
            let target = path.clone();
            let select = select.clone();
            p.descends().push(Descend {
                root: None,
                path,
                rect,
                select: Rc::new(move |ctx| {
                    select(ctx, target.to_vec());
                    true
                }),
            });
        }
    })
}

/// The selected location shared by repeated projections of a cell.
fn secondary_of(sources: &Sources, selection: Option<&Selection>) -> Option<Secondary> {
    match selection? {
        current if current.stage() == Stage::Edge => {
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
        projection,
        tcx,
        root_path,
        &traversal,
        Location::Root(root),
        projection,
        None,
        &hooks,
        &mut build,
    );
    resolve_choices(
        ChoiceGraph {
            root: layout,
            shared: build.shared,
            choice_count: build.next_choice,
        },
        width,
    )
}

/// Marks `child` as the projection of `path` WITHOUT claiming any
/// clicks: the highlight, reveal rect, and keyboard reach of
/// [`descend`] over the full bounds, while pointer selection belongs
/// to the content targets the view registers — heads, delimiters,
/// rows — so clicks on structural whitespace (gutters, inter-row
/// gaps, the dead space inside a bounding box) fall through to the
/// background's deselect.
fn descend_landmark_with<C: 'static, Cv: Canvas + 'static>(
    transient: bool,
    selected: bool,
    scale: f64,
    path: SharedPath,
    select: progred_display::ActionHandler<C>,
    delete: Rc<dyn Fn(&mut C, &[Descend<C>]) -> bool>,
    child: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    if transient {
        return child;
    }
    let highlight_path = path.clone();
    let marked = decorate(child, move |p, rect| {
        let highlight_path = highlight_path.clone();
        p.ink(move |cv, ink| {
            if selected {
                primary_highlight(scale, cv, rect);
            } else if matches!(
                tree_hovered(ink),
                Some(Hover::Value(hovered)) if hovered.as_ref() == highlight_path.as_ref()
            ) {
                hover_highlight(scale, cv, rect);
            }
        });
    });
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

/// The secondary selection's mark: a subtle wash over another whole
/// projection of the selected value — an expanded block, a collapsed
/// handle, or a label. The primary selection's geometry at lower
/// strength, so the two read as one family.
fn secondary_mark_with<C: 'static, Cv: Canvas + 'static>(
    strong: bool,
    scale: f64,
    secondary: Secondary,
    content: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    decorate(content, move |p, rect| {
        p.ink(move |cv, ink| {
            // The hover variant is the same mark at half voice.
            let faint = !strong && ink.hovered_secondary == Some(&secondary);
            if !strong && !faint {
                return;
            }
            let bg = RoundedRect::from_rect(rect.inset(3.0 * scale), 5.0 * scale);
            let (fill, line) = if strong { (0.10, 0.55) } else { (0.05, 0.25) };
            cv.fill(bg, Color::new([0.0, 0.48, 1.0, fill]), Affine::IDENTITY);
            cv.stroke(
                bg,
                Stroke::new(1.5 * scale),
                Color::new([0.0, 0.48, 1.0, line]),
                Affine::IDENTITY,
            );
        });
    })
}

/// Starts the ordinary projection at a value with no document source.
/// Interaction attributes the transient tree to `owner`, while its
/// children remain read-only and have no document paths of their own.
fn prepare_transient_root<C: 'static, Cv: Canvas + 'static>(
    cx: &Cx,
    projection: Option<&Projection<C>>,
    tcx: &mut TextCtx,
    path: &[Step],
    result: Value,
    fuel: usize,
    hooks: &Hooks<C>,
    build: &mut ChoiceBuild<Placed<C, Cv>>,
) -> ChoiceLayout<Placed<C, Cv>> {
    let ordinary_projection = projection.map(Projection::without_entry);
    let projection = ordinary_projection.as_ref();
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
        start_edit: Rc::new(|_, _, _| {}),
        toggle: Rc::new(|_, _| {}),
        update_state: hooks.update_state.clone(),
        edit: Rc::new(|_| None),
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
        projection,
        None,
        &result_hooks,
        build,
    )
}

/// Adds one GID step to the active source and resolves that location.
/// Contextual partials precede the ambient projection for a present
/// child. A concrete missing layout replaces the ordinary pending
/// view without inventing a value at that location.
#[allow(clippy::too_many_arguments)]
fn prepare_descend<C: 'static, Cv: Canvas + 'static>(
    cx: &Cx,
    projection: Option<&Projection<C>>,
    tcx: &mut TextCtx,
    parent_path: &[Step],
    ancestors: &Traversal,
    parent: Option<&Value>,
    step: Step,
    contextual_partials: Option<Vec<progred_display::Partial<C, Hover>>>,
    missing: Option<progred_display::Layout<C, Hover>>,
    hooks: &Hooks<C>,
    build: &mut ChoiceBuild<Placed<C, Cv>>,
) -> ChoiceLayout<Placed<C, Cv>> {
    let mut path = parent_path.to_vec();
    path.push(step.clone());
    let contextual_projection = contextual_projection(projection, contextual_partials);
    let child_projection = contextual_projection.as_ref().or(projection);
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
            child_projection,
            tcx,
            &path,
            &ancestors,
            Location::Child { parent, step },
            projection,
            missing,
            hooks,
            build,
        )
    } else {
        prepare_location(
            cx,
            child_projection,
            tcx,
            &path,
            ancestors,
            match parent {
                Some(parent) => Location::Child { parent, step },
                None => Location::Root(None),
            },
            projection,
            missing,
            hooks,
            build,
        )
    }
}

#[allow(clippy::too_many_arguments)]
fn prepare_location<C: 'static, Cv: Canvas + 'static>(
    cx: &Cx,
    present_projection: Option<&Projection<C>>,
    tcx: &mut TextCtx,
    path: &[Step],
    ancestors: &Traversal,
    location: Location<'_>,
    missing_projection: Option<&Projection<C>>,
    missing: Option<progred_display::Layout<C, Hover>>,
    hooks: &Hooks<C>,
    build: &mut ChoiceBuild<Placed<C, Cv>>,
) -> ChoiceLayout<Placed<C, Cv>> {
    match location.value(|cell, resolution| cx.sources.value(cell, resolution)) {
        Some(value) => prepare_present_value(
            cx,
            present_projection,
            tcx,
            path,
            ancestors,
            value,
            hooks,
            build,
        ),
        None => match missing {
            Some(layout) => prepare_missing_layout(
                cx,
                missing_projection,
                tcx,
                path,
                ancestors,
                hooks,
                layout,
                build,
            ),
            None => prepare(
                cx,
                present_projection,
                tcx,
                path,
                ancestors,
                hooks,
                None,
                progred_display::completion(progred_display::CompletionKind::Value, None),
                build,
            ),
        },
    }
}

#[allow(clippy::too_many_arguments)]
fn prepare_missing_layout<C: 'static, Cv: Canvas + 'static>(
    cx: &Cx,
    projection: Option<&Projection<C>>,
    tcx: &mut TextCtx,
    path: &[Step],
    ancestors: &Traversal,
    hooks: &Hooks<C>,
    layout: progred_display::Layout<C, Hover>,
    build: &mut ChoiceBuild<Placed<C, Cv>>,
) -> ChoiceLayout<Placed<C, Cv>> {
    let inner = prepare(
        cx, projection, tcx, path, ancestors, hooks, None, layout, build,
    );
    let landmark: SharedPath = Rc::from(path);
    let transient = cx.source.transient();
    let selected = cx.selected(path);
    let scale = cx.styles.scale;
    let select = select_handler(landmark.clone(), hooks);
    let delete = hooks.delete.clone();
    ChoiceLayout::map(inner, 0.0, move |inner| {
        descend_landmark_with(transient, selected, scale, landmark, select, delete, inner)
    })
}

#[allow(clippy::too_many_arguments)]
fn prepare_present_value<C: 'static, Cv: Canvas + 'static>(
    cx: &Cx,
    projection: Option<&Projection<C>>,
    tcx: &mut TextCtx,
    path: &[Step],
    ancestors: &Traversal,
    value: &Value,
    hooks: &Hooks<C>,
    build: &mut ChoiceBuild<Placed<C, Cv>>,
) -> ChoiceLayout<Placed<C, Cv>> {
    let layout = present_layout(cx, projection, path, ancestors, value, hooks);
    let ordinary_projection = projection
        .filter(|projection| projection.entry.is_some() && !matches!(value, Value::Cell(_)))
        .map(Projection::without_entry);
    let inner = prepare(
        cx,
        ordinary_projection.as_ref().or(projection),
        tcx,
        path,
        ancestors,
        hooks,
        Some(value),
        layout,
        build,
    );
    let landmark_path: SharedPath = Rc::from(path);
    // Other projections of the selected location carry the secondary
    // mark; the selected one has the primary highlight.
    let inner = if cx.selected(path) {
        inner
    } else {
        let secondary = Secondary::from_context(landmark_path.clone(), value, ancestors.enclosing);
        let strong = cx.secondary.as_ref() == Some(&secondary);
        let scale = cx.styles.scale;
        ChoiceLayout::map(inner, 0.0, move |inner| {
            secondary_mark_with(strong, scale, secondary, inner)
        })
    };
    // A landmark, not a target: highlight and keyboard reach span
    // the full bounds, while clicks belong to the content each arm
    // claimed above — structural whitespace deselects.
    let transient = cx.source.transient();
    let selected = cx.selected(path);
    let scale = cx.styles.scale;
    let select = select_handler(landmark_path.clone(), hooks);
    let delete = hooks.delete.clone();
    let landmark = landmark_path.clone();
    let placed = ChoiceLayout::map(inner, 0.0, move |inner| {
        descend_landmark_with(transient, selected, scale, landmark, select, delete, inner)
    });
    let grounded = match ground_decoration(cx, path, value) {
        Some((scale, color)) => {
            ChoiceLayout::map(placed, 0.0, move |placed| ground_with(scale, color, placed))
        }
        None => placed,
    };
    let target_path = landmark_path.clone();
    let target_value = value.clone();
    let pick = hooks.pick.clone();
    let select = hooks.select.clone();
    ChoiceLayout::map(grounded, 0.0, move |grounded| {
        pick_target_with(target_path, target_value, pick, select, grounded)
    })
}

fn present_layout<C: 'static>(
    cx: &Cx,
    projection: Option<&Projection<C>>,
    path: &[Step],
    ancestors: &Traversal,
    value: &Value,
    hooks: &Hooks<C>,
) -> progred_display::Layout<C, Hover> {
    // Traversal has already accumulated the cells crossed by Follow
    // edges. A repeated cell is the graph cycle; re-resolving this
    // path and all its prefixes here made every frame walk from the
    // root once per projected value.
    let in_cycle = value
        .as_cell()
        .is_some_and(|cell| ancestors.cells.contains(&cell));
    if crate::selection::collapse_default_for_value(&cx.sources, value, in_cycle)
        .is_some_and(|default| crate::annotations::collapsed(cx.annotations, path, default))
        && let Some(collapsed) = structure::collapsed_layout(cx, path, value, hooks)
    {
        return collapsed;
    }
    projection
        .and_then(|projection| {
            // Editor state arrives positionally: the payload only at
            // the selected path, the annotations only at this one.
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
            projection.apply(&progred_display::ProjectionInput {
                env: &ProjectEnv { cx },
                value,
                scale_factor: cx.styles.scale,
                writable: !cx.source.transient() && writable_at(&cx.sources, path),
                selection: selection.as_ref(),
                pending,
                state,
                targets: progred_display::ProjectionTargets::new(&target),
            })
        })
        .unwrap_or_else(|| structure::of(cx, path, value, hooks))
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
                move |c| edit_ctx(c),
            )
        }
        None => fallback,
    }
}

/// An Activate-to-select target for `path` — for parts like labels
/// and the cell star. When present, `value` is also offered to an open
/// pending by Pick.
fn select_target_with<C: 'static, Cv: Canvas + 'static>(
    path: SharedPath,
    value: Option<Value>,
    select: Rc<dyn Fn(&mut C, Path)>,
    pick: Rc<dyn Fn(&mut C, Value) -> bool>,
    content: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    let claimed = hover_target(path.clone(), content);
    quiet_select_target_with(path, value, select, pick, claimed)
}

/// Name the value at `path` for the pointer over this ink, adding no
/// action of its own — the hover half of [`select_target`], and the
/// flat literal's delimiter dress.
fn hover_target<C: 'static, Cv: Canvas + 'static>(
    path: SharedPath,
    content: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    before(content, move |p, placement| {
        hover_claim(p, placement, Hover::Value(path.clone()));
    })
}

/// [`select_target`] minus the pointer claim — for a container's
/// one-line literal, whose interior air belongs to the landmark's
/// hold and whose delimiter ink names the container through
/// [`hover_target`].
fn quiet_select_target_with<C: 'static, Cv: Canvas + 'static>(
    path: SharedPath,
    value: Option<Value>,
    select: Rc<dyn Fn(&mut C, Path)>,
    pick: Rc<dyn Fn(&mut C, Value) -> bool>,
    content: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    before(content, move |p, _| {
        let select = select.clone();
        let pick = pick.clone();
        let target = path.clone();
        let action_target = Hovered::Tree(Hover::Value(target.clone()));
        p.activate(action_target.clone(), move |ctx| {
            select(ctx, target.to_vec());
            true
        });
        if let Some(value) = value.clone() {
            p.pick(action_target, move |ctx| pick(ctx, value.clone()));
        }
    })
}
