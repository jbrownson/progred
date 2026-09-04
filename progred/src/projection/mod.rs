//! The editor's tree-projection runtime: interpret display layouts,
//! retain source provenance, and fall back to total structural display.

use crate::annotations::Annotations;
use crate::completion::{Entry, EntryAction, HasCompletion, Offers, completion_entries_with};
#[cfg(test)]
use crate::completion::{completion_entries, resolve_entry, resolve_label};
use crate::filter;
use crate::frame::Hovered;
use crate::hover::{Hover, Secondary, SourceTrace};
#[cfg(test)]
use crate::identity::short_id;
use crate::navigate::{Descend, HasDescends};
#[cfg(test)]
use crate::navigate::{projected_name_owner, step_selection};
use crate::placed::{self, Placed, before, decorate, leaf, on_key};
use crate::render::{self, text};
#[cfg(test)]
use crate::sample::{sample_document, sample_vocabulary};
use crate::selection::{Selection, Stage, last_follow, writable_at};
#[cfg(test)]
use crate::selection::{
    break_edit_run, delete_edge, from_clipboard, from_structure, pending_edge, pending_follow,
    pending_insert, pending_into, pending_value, resolve_query, set_collapse, set_value,
    to_clipboard, toggle_collapse, write_through,
};
use crate::sources::Sources;
use crate::styles::Styles;
use measured::{Extent, Measured, centered_row, col, layers, min_width, pad, row};
use progred_libraries::{absent, f64 as f64_convention, layout as layout_data, presentation, text};
mod drawing;
#[cfg(test)]
mod iop_tree_native;
mod location;
pub(crate) use drawing::Memo as DrawingMemo;
use gid::{CellId, Path, Step, Value};
#[cfg(test)]
use gid::{Cells, Document, new_cell_id};
use kurbo::{Affine, Insets, Point, Rect, RoundedRect, Size, Stroke, Vec2};
use location::Location;
use peniko::{Brush, Color};
use puri::delim::{self, Delim, DelimStyle};
use puri::draw::Canvas;
use puri::edit::{
    EditCtx, LineEditDescription, LineEditPointerDown, LineEditPresentation, LineEditState,
};
use puri::geometry::Placement;
use puri::handler::{HasHandler, ImeEvent, ScrollOutcome};
use puri::interact::is_primary_contact;
use puri::text::{TextCtx, TextStyle};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use ui_events::ScrollDelta;
use ui_events::keyboard::KeyboardEvent;
use ui_events::keyboard::{Key, KeyState, NamedKey};
use ui_events::pointer::{
    PointerButton, PointerButtonEvent, PointerScrollEvent, PointerType, PointerUpdate,
};

type SharedPath = Rc<[Step]>;

/// One ordered composition of partial value projections. The
/// structural fallback lives in this runtime and is always total.
pub struct Projection<World> {
    partials: Box<[progred_display::Partial<World, Hover>]>,
}

impl<World> Clone for Projection<World> {
    fn clone(&self) -> Self {
        Self {
            partials: self.partials.clone(),
        }
    }
}

impl<World> Default for Projection<World> {
    fn default() -> Self {
        Self {
            partials: Box::new([]),
        }
    }
}

impl<World> Projection<World> {
    pub fn new(partials: impl IntoIterator<Item = progred_display::Partial<World, Hover>>) -> Self {
        Self {
            partials: partials.into_iter().collect(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn apply(
        &self,
        env: &dyn progred_display::Env,
        value: &Value,
        scale_factor: f64,
        writable: bool,
        selection: Option<&Value>,
        state: Option<&Value>,
        targets: progred_display::ProjectionTargets<'_, World, Hover>,
    ) -> Option<progred_display::Layout<World, Hover>> {
        let input = progred_display::ProjectionInput {
            env,
            value,
            scale_factor,
            writable,
            selection,
            state,
            targets,
        };
        self.partials.iter().find_map(|partial| partial(&input))
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
    drawing_memo: &'a DrawingMemo,
}

#[derive(Clone, Default)]
struct Traversal {
    /// Cells crossed by `Follow`, used to stop projection cycles.
    cells: HashSet<CellId>,
    /// The nearest followed cell and the start of its relative path.
    enclosing: Option<(CellId, usize)>,
    /// The nearest projection-supplied vocabulary. This is frame-local
    /// description data, not retained editor state.
    completions: Option<progred_display::CompletionProvider>,
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

    fn names(&self, cell: CellId) -> Vec<&str> {
        if self.cx.raw {
            Vec::new()
        } else {
            self.cx.sources.names(cell).collect()
        }
    }

    fn cell_definitions(&self, cell: CellId) -> Vec<progred_display::CellDefinition<'_>> {
        self.cx
            .sources
            .definitions(cell)
            .map(|definition| match definition.definition {
                progred_libraries::DefinitionRef::Value(value) => {
                    progred_display::CellDefinition::Value(definition.source, value)
                }
                progred_libraries::DefinitionRef::ForeignFunction(_) => {
                    progred_display::CellDefinition::Foreign
                }
            })
            .collect()
    }
}

fn resolved_definitions(cx: &Cx<'_>, cell: CellId) -> Vec<grap::Definition> {
    cx.sources.grap_definitions(cell)
}

fn evaluate(cx: &Cx<'_>, expression: &Value, fuel: usize) -> grap::Evaluation {
    grap::evaluate(expression, |cell| resolved_definitions(cx, cell), fuel)
}

#[derive(Clone, Copy)]
struct Widths {
    preferred: f64,
    minimum: f64,
    maximum: f64,
}

impl Widths {
    fn fixed(width: f64) -> Self {
        Self {
            preferred: width,
            minimum: width,
            maximum: width,
        }
    }

    fn plus(self, width: f64) -> Self {
        Self {
            preferred: self.preferred + width,
            minimum: self.minimum + width,
            maximum: self.maximum + width,
        }
    }
}

/// A measured layout whose responsive choices have not been settled.
/// Fixed leaves already own their placement continuations; selecting
/// forms therefore combines widths only and never remeasures text
/// or reruns a projection.
struct ChoiceLayout<Out> {
    widths: Widths,
    kind: ChoiceKind<Out>,
}

type MeasureMap<Out> = Box<dyn FnOnce(Measured<Out>) -> Measured<Out>>;

enum ChoiceKind<Out> {
    Fixed(Measured<Out>),
    Use(usize),
    Map {
        child: Box<ChoiceLayout<Out>>,
        map: MeasureMap<Out>,
        width_add: f64,
    },
    Row {
        alignment: progred_display::RowAlignment,
        gap: f64,
        children: Vec<ChoiceLayout<Out>>,
    },
    Col {
        baseline: usize,
        gap: f64,
        children: Vec<ChoiceLayout<Out>>,
    },
    Overlay {
        children: Vec<ChoiceLayout<Out>>,
    },
    Popover {
        trigger: Box<ChoiceLayout<Out>>,
        content: Box<ChoiceLayout<Out>>,
        map: Box<dyn FnOnce(Measured<Out>, Measured<Out>) -> Measured<Out>>,
    },
    Pad {
        insets: Insets,
        child: Box<ChoiceLayout<Out>>,
    },
    Alternatives {
        id: usize,
        options: Vec<ChoiceLayout<Out>>,
    },
}

struct ChoiceBuild<Out> {
    next_choice: usize,
    shared_ids: HashMap<usize, usize>,
    shared: Vec<Option<ChoiceLayout<Out>>>,
}

impl<Out> Default for ChoiceBuild<Out> {
    fn default() -> Self {
        Self {
            next_choice: 0,
            shared_ids: HashMap::new(),
            shared: Vec::new(),
        }
    }
}

struct ChoiceGraph<Out> {
    root: ChoiceLayout<Out>,
    shared: Vec<Option<ChoiceLayout<Out>>>,
    choice_count: usize,
}

#[derive(Default)]
struct LayoutTrace {
    nodes: usize,
    alternatives: usize,
    multiway_alternatives: usize,
    wider_backups: usize,
    maximum_depth: usize,
    selected_fallbacks: usize,
    deepest_selected_fallback: usize,
}

impl<Out: measured::Output + 'static> ChoiceLayout<Out> {
    fn fixed(measured: Measured<Out>) -> Self {
        Self {
            widths: Widths::fixed(measured.extent.width),
            kind: ChoiceKind::Fixed(measured),
        }
    }

    fn used(id: usize, widths: Widths) -> Self {
        Self {
            widths,
            kind: ChoiceKind::Use(id),
        }
    }

    fn map(
        child: Self,
        width_add: f64,
        map: impl FnOnce(Measured<Out>) -> Measured<Out> + 'static,
    ) -> Self {
        Self {
            widths: child.widths.plus(width_add),
            kind: ChoiceKind::Map {
                child: Box::new(child),
                map: Box::new(map),
                width_add,
            },
        }
    }

    fn aligned_row(
        alignment: progred_display::RowAlignment,
        gap: f64,
        children: Vec<Self>,
    ) -> Self {
        let gaps = gap * children.len().saturating_sub(1) as f64;
        let widths = Widths {
            preferred: children
                .iter()
                .map(|child| child.widths.preferred)
                .sum::<f64>()
                + gaps,
            minimum: children
                .iter()
                .map(|child| child.widths.minimum)
                .sum::<f64>()
                + gaps,
            maximum: children
                .iter()
                .map(|child| child.widths.maximum)
                .sum::<f64>()
                + gaps,
        };
        Self {
            widths,
            kind: ChoiceKind::Row {
                alignment,
                gap,
                children,
            },
        }
    }

    fn col(baseline: usize, gap: f64, children: Vec<Self>) -> Self {
        let widths = Widths {
            preferred: children
                .iter()
                .map(|child| child.widths.preferred)
                .fold(0.0_f64, f64::max),
            minimum: children
                .iter()
                .map(|child| child.widths.minimum)
                .fold(0.0_f64, f64::max),
            maximum: children
                .iter()
                .map(|child| child.widths.maximum)
                .fold(0.0_f64, f64::max),
        };
        Self {
            widths,
            kind: ChoiceKind::Col {
                baseline,
                gap,
                children,
            },
        }
    }

    fn overlay(children: Vec<Self>) -> Self {
        let widths = Widths {
            preferred: children
                .iter()
                .map(|child| child.widths.preferred)
                .fold(0.0_f64, f64::max),
            minimum: children
                .iter()
                .map(|child| child.widths.minimum)
                .fold(0.0_f64, f64::max),
            maximum: children
                .iter()
                .map(|child| child.widths.maximum)
                .fold(0.0_f64, f64::max),
        };
        Self {
            widths,
            kind: ChoiceKind::Overlay { children },
        }
    }

    fn popover(
        trigger: Self,
        content: Self,
        map: impl FnOnce(Measured<Out>, Measured<Out>) -> Measured<Out> + 'static,
    ) -> Self {
        Self {
            widths: trigger.widths,
            kind: ChoiceKind::Popover {
                trigger: Box::new(trigger),
                content: Box::new(content),
                map: Box::new(map),
            },
        }
    }

    fn pad(insets: Insets, child: Self) -> Self {
        let widths = child.widths.plus(insets.x0 + insets.x1);
        Self {
            widths,
            kind: ChoiceKind::Pad {
                insets,
                child: Box::new(child),
            },
        }
    }

    fn alternatives(id: usize, options: Vec<Self>) -> Self {
        let widths = match options.first() {
            Some(first) => Widths {
                preferred: first.widths.preferred,
                minimum: options
                    .iter()
                    .map(|option| option.widths.minimum)
                    .fold(f64::INFINITY, f64::min),
                maximum: options
                    .iter()
                    .map(|option| option.widths.maximum)
                    .fold(0.0_f64, f64::max),
            },
            None => Widths::fixed(0.0),
        };
        Self {
            widths,
            kind: ChoiceKind::Alternatives { id, options },
        }
    }

    fn select(&self, choices: &mut [usize], shared: &[Option<Self>], available: f64) -> f64 {
        match &self.kind {
            ChoiceKind::Fixed(measured) => measured.extent.width,
            ChoiceKind::Use(id) => shared[*id]
                .as_ref()
                .map_or(0.0, |child| child.select(choices, shared, available)),
            ChoiceKind::Map {
                child, width_add, ..
            } => child.select(choices, shared, (available - width_add).max(0.0)) + width_add,
            ChoiceKind::Row { gap, children, .. } => {
                let gaps = gap * children.len().saturating_sub(1) as f64;
                let mut slack = (available - self.widths.minimum).max(0.0);
                children
                    .iter()
                    .map(|child| {
                        let budget = child.widths.minimum + slack;
                        let width = child.select(choices, shared, budget);
                        slack = (budget - width).max(0.0);
                        width
                    })
                    .sum::<f64>()
                    + gaps
            }
            ChoiceKind::Col { children, .. } | ChoiceKind::Overlay { children } => children
                .iter()
                .map(|child| child.select(choices, shared, available))
                .fold(0.0_f64, f64::max),
            ChoiceKind::Popover {
                trigger, content, ..
            } => {
                content.select(choices, shared, available);
                trigger.select(choices, shared, available)
            }
            ChoiceKind::Pad { insets, child } => {
                let horizontal = insets.x0 + insets.x1;
                child.select(choices, shared, (available - horizontal).max(0.0)) + horizontal
            }
            ChoiceKind::Alternatives { id, options } => match options.split_last() {
                None => 0.0,
                Some((accommodating, preferred)) if available <= 0.0 => {
                    choices[*id] = preferred.len();
                    accommodating.select(choices, shared, available)
                }
                Some((accommodating, preferred)) => {
                    match preferred
                        .iter()
                        .enumerate()
                        .find(|(_, option)| option.widths.preferred <= available)
                    {
                        Some((index, option)) => {
                            choices[*id] = index;
                            option.widths.preferred
                        }
                        None => {
                            choices[*id] = preferred.len();
                            let accommodating_width =
                                accommodating.select(choices, shared, available);
                            preferred.iter().enumerate().rev().fold(
                                accommodating_width,
                                |best_width, (index, option)| {
                                    if option.widths.preferred <= best_width {
                                        choices[*id] = index;
                                        option.widths.preferred
                                    } else {
                                        best_width
                                    }
                                },
                            )
                        }
                    }
                }
            },
        }
    }

    fn inspect(&self, depth: usize, trace: &mut LayoutTrace) {
        trace.nodes += 1;
        trace.maximum_depth = trace.maximum_depth.max(depth);
        match &self.kind {
            ChoiceKind::Fixed(_) | ChoiceKind::Use(_) => {}
            ChoiceKind::Map { child, .. } | ChoiceKind::Pad { child, .. } => {
                child.inspect(depth + 1, trace)
            }
            ChoiceKind::Row { children, .. }
            | ChoiceKind::Col { children, .. }
            | ChoiceKind::Overlay { children } => {
                for child in children {
                    child.inspect(depth + 1, trace);
                }
            }
            ChoiceKind::Popover {
                trigger, content, ..
            } => {
                trigger.inspect(depth + 1, trace);
                content.inspect(depth + 1, trace);
            }
            ChoiceKind::Alternatives { options, .. } => {
                trace.alternatives += 1;
                trace.multiway_alternatives += usize::from(options.len() > 2);
                trace.wider_backups += options
                    .windows(2)
                    .filter(|pair| pair[1].widths.preferred > pair[0].widths.preferred)
                    .count();
                for option in options {
                    option.inspect(depth + 1, trace);
                }
            }
        }
    }

    fn inspect_selection(
        &self,
        choices: &[usize],
        shared: &[Option<Self>],
        depth: usize,
        trace: &mut LayoutTrace,
    ) {
        match &self.kind {
            ChoiceKind::Fixed(_) => {}
            ChoiceKind::Use(id) => {
                if let Some(child) = shared[*id].as_ref() {
                    child.inspect_selection(choices, shared, depth + 1, trace);
                }
            }
            ChoiceKind::Map { child, .. } | ChoiceKind::Pad { child, .. } => {
                child.inspect_selection(choices, shared, depth + 1, trace)
            }
            ChoiceKind::Row { children, .. }
            | ChoiceKind::Col { children, .. }
            | ChoiceKind::Overlay { children } => {
                for child in children {
                    child.inspect_selection(choices, shared, depth + 1, trace);
                }
            }
            ChoiceKind::Popover {
                trigger, content, ..
            } => {
                trigger.inspect_selection(choices, shared, depth + 1, trace);
                content.inspect_selection(choices, shared, depth + 1, trace);
            }
            ChoiceKind::Alternatives { id, options } => {
                let selected = choices[*id];
                if selected > 0 {
                    trace.selected_fallbacks += 1;
                    trace.deepest_selected_fallback = trace.deepest_selected_fallback.max(depth);
                }
                if let Some(option) = options.get(selected) {
                    option.inspect_selection(choices, shared, depth + 1, trace);
                }
            }
        }
    }

    fn settle(self, choices: &[usize], shared: &mut [Option<Self>]) -> Measured<Out> {
        match self.kind {
            ChoiceKind::Fixed(measured) => measured,
            ChoiceKind::Use(id) => shared[id]
                .take()
                .expect("a shared layout is consumed by only one selected form")
                .settle(choices, shared),
            ChoiceKind::Map { child, map, .. } => map(child.settle(choices, shared)),
            ChoiceKind::Row {
                alignment,
                gap,
                children,
            } => {
                let children = children
                    .into_iter()
                    .map(|child| child.settle(choices, shared))
                    .collect();
                match alignment {
                    progred_display::RowAlignment::Baseline => row(gap, children),
                    progred_display::RowAlignment::Center => centered_row(gap, children),
                }
            }
            ChoiceKind::Col {
                baseline,
                gap,
                children,
            } => col(
                baseline,
                gap,
                children
                    .into_iter()
                    .map(|child| child.settle(choices, shared))
                    .collect(),
            ),
            ChoiceKind::Overlay { children } => layers(
                children
                    .into_iter()
                    .map(|child| child.settle(choices, shared))
                    .collect(),
            ),
            ChoiceKind::Popover {
                trigger,
                content,
                map,
            } => map(
                trigger.settle(choices, shared),
                content.settle(choices, shared),
            ),
            ChoiceKind::Pad { insets, child } => pad(insets, child.settle(choices, shared)),
            ChoiceKind::Alternatives { id, options } => {
                options.into_iter().nth(choices[id]).map_or_else(
                    || row(0.0, Vec::new()),
                    |option| option.settle(choices, shared),
                )
            }
        }
    }
}

fn resolve_choices<Out: measured::Output + 'static>(
    graph: ChoiceGraph<Out>,
    available: f64,
) -> Measured<Out> {
    let ChoiceGraph {
        root: layout,
        mut shared,
        choice_count,
    } = graph;
    let tracing = std::env::var_os("PROGRED_LAYOUT_TRACE").is_some();
    let mut trace = LayoutTrace::default();
    if tracing {
        layout.inspect(0, &mut trace);
        for child in shared.iter().flatten() {
            child.inspect(0, &mut trace);
        }
    }
    let preferred = layout.widths.preferred;
    let minimum = layout.widths.minimum;
    let maximum = layout.widths.maximum;
    if tracing {
        eprintln!(
            "layout analysis: nodes={} alternatives={} wider_backups={} preferred={preferred:.1} minimum={minimum:.1} maximum={maximum:.1} available={available:.1}",
            trace.nodes, trace.alternatives, trace.wider_backups,
        );
    }
    let mut choices = vec![0; choice_count];
    let selected = layout.select(&mut choices, &shared, available);
    if tracing {
        layout.inspect_selection(&choices, &shared, 0, &mut trace);
        eprintln!(
            "layout: nodes={} alternatives={} multiway={} wider_backups={} preferred={preferred:.1} minimum={minimum:.1} selected={selected:.1} available={available:.1} fallbacks={} deepest_fallback={}",
            trace.nodes,
            trace.alternatives,
            trace.multiway_alternatives,
            trace.wider_backups,
            trace.selected_fallbacks,
            trace.deepest_selected_fallback,
        );
    }
    layout.settle(&choices, &mut shared)
}

#[cfg(test)]
mod choice_tests {
    use super::*;

    #[derive(Clone, Copy)]
    struct Output;

    impl measured::Output for Output {
        fn empty() -> Self {
            Self
        }

        fn over(self, _: Self) -> Self {
            self
        }
    }

    fn fixed(width: f64) -> ChoiceLayout<Output> {
        ChoiceLayout::fixed(measured::leaf(
            Extent {
                width,
                ascent: 1.0,
                descent: 0.0,
            },
            |_| Output,
        ))
    }

    fn select(layout: &ChoiceLayout<Output>, count: usize, available: f64) -> (f64, Vec<usize>) {
        let mut choices = vec![0; count];
        let width = layout.select(&mut choices, &[], available);
        (width, choices)
    }

    #[test]
    fn preferred_form_wins_when_its_natural_width_fits() {
        let layout = ChoiceLayout::alternatives(0, vec![fixed(100.0), fixed(70.0)]);

        let (width, choices) = select(&layout, 1, 100.0);

        assert_eq!(width, 100.0);
        assert_eq!(choices, vec![0]);
    }

    #[test]
    fn outer_accommodation_precedes_nested_accommodation() {
        let nested = ChoiceLayout::alternatives(1, vec![fixed(100.0), fixed(60.0)]);
        let layout = ChoiceLayout::alternatives(0, vec![nested, fixed(80.0)]);

        let (width, choices) = select(&layout, 2, 80.0);

        assert_eq!(width, 80.0);
        assert_eq!(choices, vec![1, 0]);
    }

    #[test]
    fn accommodating_form_receives_the_real_allocation() {
        let nested = ChoiceLayout::alternatives(1, vec![fixed(60.0), fixed(20.0)]);
        let accommodating = ChoiceLayout::aligned_row(
            progred_display::RowAlignment::Baseline,
            0.0,
            vec![nested, fixed(40.0)],
        );
        let layout = ChoiceLayout::alternatives(0, vec![fixed(120.0), accommodating]);

        let (width, choices) = select(&layout, 2, 70.0);

        assert_eq!(width, 60.0);
        assert_eq!(choices, vec![1, 1]);
    }

    #[test]
    fn a_row_reserves_its_siblings_minimum_widths() {
        let choice = ChoiceLayout::alternatives(0, vec![fixed(90.0), fixed(50.0)]);
        let layout = ChoiceLayout::aligned_row(
            progred_display::RowAlignment::Baseline,
            0.0,
            vec![choice, fixed(40.0)],
        );

        let (width, choices) = select(&layout, 1, 100.0);

        assert_eq!(width, 90.0);
        assert_eq!(choices, vec![1]);
    }

    #[test]
    fn a_wider_backup_does_not_hide_a_later_fit() {
        let layout = ChoiceLayout::alternatives(0, vec![fixed(100.0), fixed(120.0), fixed(70.0)]);

        let (width, choices) = select(&layout, 1, 80.0);

        assert_eq!(width, 70.0);
        assert_eq!(choices, vec![2]);
    }

    #[test]
    fn one_overwide_column_does_not_expand_its_siblings_budget() {
        let choice = ChoiceLayout::alternatives(0, vec![fixed(105.0), fixed(70.0)]);
        let layout = ChoiceLayout::col(0, 0.0, vec![fixed(110.0), choice]);

        let (width, choices) = select(&layout, 1, 100.0);

        assert_eq!(width, 110.0);
        assert_eq!(choices, vec![1]);
    }
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
        progred_display::Layout::DrawingProgram {
            width,
            ascent,
            descent,
            fuel,
            program,
        } => ChoiceLayout::fixed(drawing::program_leaf(
            cx, path, width, ascent, descent, fuel, program,
        )),
        progred_display::Layout::Query => {
            let engaged = cx.pending_edge_under(path).map(|(query, _)| query);
            ChoiceLayout::fixed(match engaged {
                Some(query) => label_query(cx, tcx, query, hooks),
                None => render::text(tcx, "…", &cx.styles.dim),
            })
        }
        progred_display::Layout::WithCompletions { child, provider } => {
            let mut scoped = ancestors.clone();
            scoped.completions = Some(provider);
            prepare(
                cx, projection, tcx, path, &scoped, hooks, value, *child, build,
            )
        }
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
            ChoiceLayout::map(inner, 0.0, move |inner| {
                if writable {
                    realize_scrub(path, target, handler, inner)
                } else {
                    inner
                }
            })
        }
        progred_display::Layout::OnStateDrag {
            child,
            target,
            handler,
        } => {
            let inner = prepare(
                cx, projection, tcx, path, ancestors, hooks, value, *child, build,
            );
            let path = path.to_vec();
            ChoiceLayout::map(inner, 0.0, move |inner| {
                realize_state_drag(path, target, handler, inner)
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
            let fill = Color::new([0.985, 0.985, 0.99, 1.0]);
            let stroke = cx.styles.dim.brush.clone();
            ChoiceLayout::popover(trigger, content, move |trigger, content| {
                let card = measured::pad(Insets::uniform(10.0 * scale), content);
                let card = before(card, move |p, placement| {
                    let shape = RoundedRect::from_rect(placement.rect, 6.0 * scale);
                    p.fill(shape, fill, Affine::IDENTITY);
                    p.stroke(shape, Stroke::new(scale), stroke, Affine::IDENTITY);
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

fn display_delim(delim: progred_display::Delim) -> Delim {
    match delim {
        progred_display::Delim::Paren => Delim::Paren,
        progred_display::Delim::Bracket => Delim::Bracket,
        progred_display::Delim::Brace => Delim::Brace,
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
        if matches!(step, Step::Follow(_)) {
            if let Some(cell) = cx.sources.resolve_path(&path).and_then(Value::as_cell) {
                follow_ancestors.cells.insert(cell);
                follow_ancestors.enclosing = Some((cell, path.len() + 1));
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

fn realize_click<C: 'static, Cv: Canvas + 'static>(
    handler: progred_display::ActionHandler<C>,
    inner: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    before(inner, move |p, placement| {
        p.handler().on_pointer_down(move |world, event| {
            is_primary_contact(event)
                && !crate::modifiers::pick(&event.state.modifiers)
                && placement.contains(Point::new(event.state.position.x, event.state.position.y))
                && handler(world)
        });
    })
}

fn realize_activate<C: 'static, Cv: Canvas + 'static>(
    target: Hover,
    handler: progred_display::ActionHandler<C>,
    inner: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    before(inner, move |p, _| {
        p.activate(Hovered::Tree(target), move |world| handler(world));
    })
}

fn realize_event_with<C: 'static, Cv: Canvas + 'static>(
    path: Path,
    function: Value,
    apply: Rc<dyn Fn(&mut C, Path, Value, Value) -> bool>,
    scale: f64,
    inner: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    before(inner, move |p, placement| {
        {
            let path = path.clone();
            let function = function.clone();
            let apply = apply.clone();
            p.handler().on_pointer_down(move |world, event| {
                placement.contains(Point::new(event.state.position.x, event.state.position.y))
                    && apply(
                        world,
                        path.clone(),
                        function.clone(),
                        pointer_button_value(
                            layout_data::vocabulary::POINTER_DOWN,
                            layout_data::vocabulary::TOUCH_START,
                            placement,
                            scale,
                            event,
                        ),
                    )
            });
        }
        {
            let path = path.clone();
            let function = function.clone();
            let apply = apply.clone();
            p.handler().on_pointer_cancel(move |world, event| {
                apply(
                    world,
                    path.clone(),
                    function.clone(),
                    pointer_cancel_value(event),
                )
            });
        }
        {
            let path = path.clone();
            let function = function.clone();
            let apply = apply.clone();
            p.handler().on_pointer_move(move |world, event| {
                apply(
                    world,
                    path.clone(),
                    function.clone(),
                    pointer_move_value(placement, scale, event),
                )
            });
        }
        {
            let path = path.clone();
            let function = function.clone();
            let apply = apply.clone();
            p.handler().on_pointer_up(move |world, event| {
                apply(
                    world,
                    path.clone(),
                    function.clone(),
                    pointer_button_value(
                        layout_data::vocabulary::POINTER_UP,
                        layout_data::vocabulary::TOUCH_END,
                        placement,
                        scale,
                        event,
                    ),
                )
            });
        }
        {
            let path = path.clone();
            let function = function.clone();
            let apply = apply.clone();
            p.handler().on_scroll(move |world, event| {
                if placement.contains(Point::new(event.state.position.x, event.state.position.y))
                    && apply(
                        world,
                        path.clone(),
                        function.clone(),
                        scroll_value(placement, scale, event),
                    )
                {
                    ScrollOutcome::consume(event)
                } else {
                    ScrollOutcome::pass(event)
                }
            });
        }
        {
            let path = path.clone();
            let function = function.clone();
            let apply = apply.clone();
            p.handler().on_key(move |world, event| {
                apply(world, path.clone(), function.clone(), key_value(event))
            });
        }
        {
            p.handler().on_ime(move |world, event| {
                apply(world, path.clone(), function.clone(), ime_value(event))
            });
        }
    })
}

fn realize_scrub<C: 'static, Cv: Canvas + 'static>(
    path: Path,
    target: Hover,
    handler: progred_display::ScrubHandler,
    inner: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    before(inner, move |p, _| {
        p.scrub(Hovered::Tree(target), path, handler);
    })
}

fn realize_state_drag<C: 'static, Cv: Canvas + 'static>(
    path: Path,
    target: Hover,
    handler: progred_display::StateDragHandler,
    inner: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    before(inner, move |p, _| {
        p.state_drag(Hovered::Tree(target), path, handler);
    })
}

fn realize_state_scroll<C: 'static, Cv: Canvas + 'static>(
    path: Path,
    handler: progred_display::StateScrollHandler,
    update_state: Rc<dyn Fn(&mut C, Path, Value) -> bool>,
    scale: f64,
    inner: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    before(inner, move |p, placement| {
        p.handler().on_scroll(move |world, event| {
            let point = Point::new(event.state.position.x, event.state.position.y);
            if !placement.contains(point) {
                return ScrollOutcome::pass(event);
            }
            let (delta_x, delta_y) = match event.delta {
                ScrollDelta::PageDelta(x, y) => (
                    f64::from(x) * placement.rect.width() / scale,
                    f64::from(y) * placement.rect.height() / scale,
                ),
                ScrollDelta::LineDelta(x, y) => (f64::from(x) * 40.0, f64::from(y) * 40.0),
                ScrollDelta::PixelDelta(delta) => (delta.x / scale, delta.y / scale),
            };
            match handler(progred_display::StateScrollEvent { delta_x, delta_y }) {
                Some(state) => {
                    if update_state(world, path.clone(), state) {
                        ScrollOutcome::consume(event)
                    } else {
                        ScrollOutcome::pass(event)
                    }
                }
                None => ScrollOutcome::pass(event),
            }
        });
    })
}

fn realize_point<C: 'static, Cv: Canvas + 'static>(
    path: Path,
    handler: progred_display::PointHandler,
    start: Rc<dyn Fn(&mut C, Path, Placement, progred_display::PointHandler, Point) -> bool>,
    inner: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    before(inner, move |p, placement| {
        p.handler().on_pointer_down(move |world, event| {
            let point = Point::new(event.state.position.x, event.state.position.y);
            is_primary_contact(event)
                && placement.contains(point)
                && start(world, path.clone(), placement, handler.clone(), point)
        });
    })
}

fn event_value(kind: CellId, fields: impl IntoIterator<Item = (CellId, Value)>) -> Value {
    Value::record(
        [(layout_data::vocabulary::EVENT_KIND, Value::Cell(kind))]
            .into_iter()
            .chain(fields),
    )
}

fn marker() -> Value {
    Value::record([])
}

fn pointer_fields(
    placement: Placement,
    scale: f64,
    state: &ui_events::pointer::PointerState,
) -> Vec<(CellId, Value)> {
    let mut fields = vec![
        (
            layout_data::vocabulary::X,
            f64_convention::value(state.position.x - placement.rect.x0),
        ),
        (
            layout_data::vocabulary::Y,
            f64_convention::value(state.position.y - placement.rect.y0),
        ),
        (
            layout_data::vocabulary::COUNT,
            f64_convention::value(f64::from(state.count)),
        ),
        (layout_data::vocabulary::SCALE, f64_convention::value(scale)),
    ];
    fields.push((
        layout_data::vocabulary::MODIFIERS,
        Value::list(
            [
                state
                    .modifiers
                    .shift()
                    .then_some(Value::Cell(layout_data::vocabulary::SHIFT)),
                crate::modifiers::command(&state.modifiers)
                    .then_some(Value::Cell(layout_data::vocabulary::COMMAND)),
            ]
            .into_iter()
            .flatten(),
        ),
    ));
    fields
}

fn pointer_button_value(
    pointer_kind: CellId,
    touch_kind: CellId,
    placement: Placement,
    scale: f64,
    event: &PointerButtonEvent,
) -> Value {
    let mut fields = pointer_fields(placement, scale, &event.state);
    let touch = event.pointer.pointer_type == PointerType::Touch;
    if !touch && event.button == Some(PointerButton::Primary) {
        fields.push((
            layout_data::vocabulary::BUTTON,
            Value::Cell(layout_data::vocabulary::PRIMARY),
        ));
    }
    event_value(if touch { touch_kind } else { pointer_kind }, fields)
}

fn pointer_move_value(placement: Placement, scale: f64, event: &PointerUpdate) -> Value {
    let mut fields = pointer_fields(placement, scale, &event.current);
    let touch = event.pointer.pointer_type == PointerType::Touch;
    if !touch && event.current.buttons.contains(PointerButton::Primary) {
        fields.push((
            layout_data::vocabulary::BUTTON,
            Value::Cell(layout_data::vocabulary::PRIMARY),
        ));
    }
    event_value(
        if touch {
            layout_data::vocabulary::TOUCH_MOVE
        } else {
            layout_data::vocabulary::POINTER_MOVE
        },
        fields,
    )
}

fn pointer_cancel_value(event: &ui_events::pointer::PointerInfo) -> Value {
    event_value(
        if event.pointer_type == PointerType::Touch {
            layout_data::vocabulary::TOUCH_CANCEL
        } else {
            layout_data::vocabulary::POINTER_CANCEL
        },
        [],
    )
}

fn scroll_value(placement: Placement, scale: f64, event: &PointerScrollEvent) -> Value {
    let mut fields = pointer_fields(placement, scale, &event.state);
    let (x, y) = match event.delta {
        ScrollDelta::PageDelta(x, y) | ScrollDelta::LineDelta(x, y) => (f64::from(x), f64::from(y)),
        ScrollDelta::PixelDelta(delta) => (delta.x, delta.y),
    };
    fields.extend([
        (layout_data::vocabulary::DELTA_X, f64_convention::value(x)),
        (layout_data::vocabulary::DELTA_Y, f64_convention::value(y)),
    ]);
    event_value(layout_data::vocabulary::SCROLL, fields)
}

fn key_value(event: &KeyboardEvent) -> Value {
    let mut fields = vec![
        (
            layout_data::vocabulary::EVENT_STATE,
            Value::Cell(match event.state {
                KeyState::Down => layout_data::vocabulary::DOWN,
                KeyState::Up => layout_data::vocabulary::UP,
            }),
        ),
        (
            layout_data::vocabulary::CONTENT,
            text::value(event.key.to_string()),
        ),
    ];
    if event.repeat {
        fields.push((layout_data::vocabulary::REPEAT, marker()));
    }
    fields.push((
        layout_data::vocabulary::MODIFIERS,
        Value::list(
            [
                event
                    .modifiers
                    .shift()
                    .then_some(Value::Cell(layout_data::vocabulary::SHIFT)),
                crate::modifiers::command(&event.modifiers)
                    .then_some(Value::Cell(layout_data::vocabulary::COMMAND)),
            ]
            .into_iter()
            .flatten(),
        ),
    ));
    event_value(layout_data::vocabulary::KEY, fields)
}

fn ime_value(event: &ImeEvent) -> Value {
    let mut fields = Vec::new();
    let state = match event {
        ImeEvent::Enabled => layout_data::vocabulary::IME_ENABLED,
        ImeEvent::Disabled => layout_data::vocabulary::IME_DISABLED,
        ImeEvent::Preedit(content, cursor) => {
            fields.push((layout_data::vocabulary::CONTENT, text::value(content)));
            if let Some((start, end)) = cursor {
                fields.extend([
                    (
                        layout_data::vocabulary::START,
                        f64_convention::value(*start as f64),
                    ),
                    (
                        layout_data::vocabulary::END,
                        f64_convention::value(*end as f64),
                    ),
                ]);
            }
            layout_data::vocabulary::IME_PREEDIT
        }
        ImeEvent::Commit(content) => {
            fields.push((layout_data::vocabulary::CONTENT, text::value(content)));
            layout_data::vocabulary::IME_COMMIT
        }
    };
    fields.push((layout_data::vocabulary::EVENT_STATE, Value::Cell(state)));
    event_value(layout_data::vocabulary::IME, fields)
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
        puri::Leaf::Text { text, paint } => {
            let style = match paint {
                progred_display::Paint::Face(face) => face_style(styles, face).clone(),
                progred_display::Paint::Brush(brush) => TextStyle {
                    brush,
                    ..styles.name.clone()
                },
            };
            render::text(tcx, &text, &style)
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
    let scale = styles.scale;
    let extent = Extent {
        width: drawing.width * scale,
        ascent: drawing.ascent * scale,
        descent: drawing.descent * scale,
    };
    let drawing = drawing.map_paint(|paint| match paint {
        progred_display::Paint::Face(face) => face_style(styles, face).brush.clone(),
        progred_display::Paint::Brush(brush) => brush,
    });
    leaf(extent, move |p, placement| {
        let transform =
            Affine::translate((placement.rect.x0, placement.rect.y0)) * Affine::scale(scale);
        puri::draw::draw(drawing, p, transform, Clone::clone);
    })
}

/// Dispatch-time callbacks the shell injects: what selecting a path
/// does, what toggling a collapse does, and how a dispatch reaches
/// the remaining host-owned editor state and measurement caches.
pub struct Hooks<C> {
    pub select: Rc<dyn Fn(&mut C, Path)>,
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
    pub delete: Rc<dyn Fn(&mut C) -> bool>,
    /// Apply a Grap event handler at `path` with the event as data and
    /// capabilities closed over that site.
    pub apply: Rc<dyn Fn(&mut C, Path, Value, Value) -> bool>,
    /// Begin a continuous point control at its settled placement.
    pub point: Rc<dyn Fn(&mut C, Path, Placement, progred_display::PointHandler, Point) -> bool>,
    /// Commit one of the exact offers shown by an engaged pending.
    pub commit_offer: Rc<dyn Fn(&mut C, &EntryAction)>,
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
    fn names(&self, cell: CellId) -> Option<String> {
        (!self.raw)
            .then(|| self.sources.display_names(cell))
            .flatten()
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

/// The delimiter metrics that marry the drawn family to the text:
/// the system font's own glyphs span -0.704..+0.171 em around the
/// baseline while its line box spans -0.929..+0.249, so a stretched
/// delimiter trims the difference at each end — it meets the glyph
/// span on its first and last lines, and a one-line span IS the
/// glyph's. Measured by `puri`'s delimiter_bench example.
const GLYPH_ASC_EM: f64 = 0.704;
const GLYPH_DESC_EM: f64 = 0.171;
const TOP_TRIM_EM: f64 = 0.929 - GLYPH_ASC_EM;
const BOTTOM_TRIM_EM: f64 = 0.249 - GLYPH_DESC_EM;
const SIDE_BEARING_EM: f64 = 0.05;

fn delim_style(scale: f64) -> DelimStyle {
    DelimStyle::for_text_size(14.0 * scale)
}

/// The delimiter width reserved while responsive choices are being
/// selected. The child's final height is not known until its choice
/// settles, so reserve the capped grown width; the final leaf below
/// takes only its actual height-derived width.
fn delim_advance(scale: f64, delim: Delim) -> f64 {
    delim_style(scale).bow(delim) * delim::MAX_GROWTH + 2.0 * SIDE_BEARING_EM * 14.0 * scale
}

fn side_advance(scale: f64, ink: &progred_display::Ink) -> f64 {
    match ink {
        progred_display::Ink::Delim { delim, .. } => delim_advance(scale, display_delim(*delim)),
    }
}

/// A drawn delimiter leaf whose box grows with and contains its ink.
/// `extent` supplies the vertical span while `ink_top..ink_bottom`
/// determines the common height-sensitive width of every delimiter
/// family. Side bearings are part of the box and therefore of its
/// honest hover target.
fn delim_leaf<C: 'static, Cv: Canvas + 'static>(
    scale: f64,
    delim: Delim,
    open: bool,
    extent: Extent,
    ink_top: f64,
    ink_bottom: f64,
    brush: Brush,
) -> Measured<Placed<C, Cv>> {
    let style = delim_style(scale);
    let bearing = SIDE_BEARING_EM * 14.0 * scale;
    let bow = style.bow_for(delim, extent.ascent + extent.descent);
    let path = if open {
        delim::open_with_width(delim, &style, ink_top, ink_bottom, bow)
    } else {
        delim::close_with_width(delim, &style, ink_top, ink_bottom, bow)
    };
    let ink_x = bearing;
    leaf(
        Extent {
            width: bow + 2.0 * bearing,
            ..extent
        },
        move |p, placement| {
            let at = Point::new(placement.rect.x0, placement.rect.y0 + extent.ascent);
            p.fill(
                path.clone(),
                brush.clone(),
                Affine::translate((at.x + ink_x, at.y)),
            );
        },
    )
}

/// A delimiter stretched over `content`'s extent, ink trimmed to meet
/// the glyph span on the first and last lines. The charged span never
/// shrinks below the glyph's own, so an empty pair still stands a
/// glyph tall — and one-line content gets exactly the flat form.
fn tall_delim<C: 'static, Cv: Canvas + 'static>(
    scale: f64,
    delim: Delim,
    open: bool,
    content: Extent,
    brush: Brush,
) -> Measured<Placed<C, Cv>> {
    let em = 14.0 * scale;
    let content = Extent {
        width: content.width,
        ascent: content.ascent.max(GLYPH_ASC_EM * em),
        descent: content.descent.max(GLYPH_DESC_EM * em),
    };
    let (ink_top, ink_bottom) = match delim {
        // Square caps visibly define the enclosure, so they meet the
        // full measured box rather than the glyph ink within its
        // first and last line boxes.
        Delim::Bracket => (-content.ascent, content.descent),
        Delim::Paren | Delim::Brace => (
            -(content.ascent - TOP_TRIM_EM * em).max(GLYPH_ASC_EM * em),
            (content.descent - BOTTOM_TRIM_EM * em).max(GLYPH_DESC_EM * em),
        ),
    };
    delim_leaf(scale, delim, open, content, ink_top, ink_bottom, brush)
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

fn glyph_extent(scale: f64) -> Extent {
    let em = 14.0 * scale;
    Extent {
        width: 0.0,
        ascent: GLYPH_ASC_EM * em,
        descent: GLYPH_DESC_EM * em,
    }
}

fn ink_leaf<C: 'static, Cv: Canvas + 'static>(
    scale: f64,
    brush: Brush,
    ink: progred_display::Ink,
    stretch: Option<Extent>,
) -> Measured<Placed<C, Cv>> {
    match ink {
        progred_display::Ink::Delim { delim, side } => tall_delim(
            scale,
            display_delim(delim),
            matches!(side, progred_display::Side::Open),
            stretch.unwrap_or_else(|| glyph_extent(scale)),
            brush,
        ),
    }
}

fn frame_leaf<C: 'static, Cv: Canvas + 'static>(
    scale: f64,
    brush: Brush,
    extent: Extent,
) -> Measured<Placed<C, Cv>> {
    leaf(extent, move |p, placement| {
        p.stroke(
            highlight_rect(scale, placement.rect),
            Stroke::new(scale),
            brush,
            Affine::IDENTITY,
        );
    })
}

fn bordered<C: 'static, Cv: Canvas + 'static>(
    scale: f64,
    brush: Brush,
    child: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    measured::around(child, move |placement, inner| {
        let mut placed = inner.place();
        if !placement.clipped_out() {
            let rect = placement.rect.inset(-0.5 * scale);
            placed.renders.push(Box::new(move |cv, _| {
                cv.stroke(rect, Stroke::new(scale), brush, Affine::IDENTITY)
            }));
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
                    ink_leaf(scale, brush.clone(), left, Some(extent)),
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
                    ink_leaf(scale, brush, right, Some(extent)),
                ),
            ),
        ],
    )
}

/// The one width every slot state shares: the cold box IS this wide,
/// and the engaged query's frame never lets the field get narrower —
/// the parity that keeps engagement from moving anything sideways.
fn slot_width(styles: &Styles) -> f64 {
    1.5 * 14.0 * styles.scale
}

/// The cold slot's ink: an empty rounded outline, the box marking
/// absence apart from projectional syntax (`…` is elision) — blank
/// on purpose, no ghost words. It is [`highlight_rect`] itself in
/// the dim brush — THE box, drawn the one way every box is drawn —
/// so engaging (the ring, blue over the same frame) and committing
/// (the ring over the same glyphs) redraw the same shape and only
/// the paint changes. The charge is exactly the text frame: the
/// empty line SHAPED, the same runtime metrics the engaged editor's
/// frame takes — no measured constants, one source.
fn placeholder_box<C: 'static, Cv: Canvas + 'static>(
    tcx: &mut TextCtx,
    styles: &Styles,
) -> Measured<Placed<C, Cv>> {
    let extent = Extent {
        width: slot_width(styles),
        ..text::<C, Cv>(tcx, "", &styles.name).extent
    };
    frame_leaf(styles.scale, styles.dim.brush.clone(), extent)
}

/// THE box: the one geometry every box around content takes — the
/// content rect plus breathing room, rounded. The selection ring
/// draws it in blue, the cold placeholder in dim; sharing the shape
/// is what keeps slot → pending → committed value from ever
/// changing the box. Sized so the QUIET wearer fits: the cold box
/// stands beside delimiters permanently, and this outset keeps its
/// hairline clear of a paren's ink where the old ring-sized box
/// overlapped.
fn highlight_rect(scale: f64, rect: Rect) -> RoundedRect {
    RoundedRect::from_rect(rect.inset(2.0 * scale), 4.0 * scale)
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
        highlight_rect(scale, rect),
        Color::new([0.0, 0.48, 1.0, 0.08]),
        Affine::IDENTITY,
    );
}

/// The pane-local primary: translucent system blue, like the Swift
/// version's selection, ringed at full strength — the strongest mark
/// in the shared vocabulary.
fn primary_highlight<P: Canvas>(scale: f64, p: &mut P, rect: Rect) {
    let bg = highlight_rect(scale, rect);
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
            Secondary::from_path(sources, path.clone(), sources.resolve_path(path.as_ref())?)
        }
        _ => None,
    }
}

/// The explicit-state boundary: everything a projection pass reads.
/// `width` is the space the projection may fill. `root` and
/// `root_path` let an editor pane begin at a stable cell occurrence
/// while retaining ordinary document-relative interaction paths.
pub struct ProjectDescription<'a, World> {
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
    /// An ordinary Grap callable placed in front of the normal
    /// projection for this root. It receives the root under the
    /// conventional `value` argument; an absent result falls through.
    pub root_projection: Option<&'a Value>,
    pub projection: Option<&'a Projection<World>>,
    /// Suggestions contributed for an empty document root.
    pub root_completions: Option<&'a progred_display::CompletionProvider>,
}

fn projection_is_absent(value: &Value) -> bool {
    absent::is_absent(value)
}

#[cfg(test)]
pub fn project<C: 'static, Cv: Canvas + 'static>(
    description: ProjectDescription<'_, C>,
    tcx: &mut TextCtx,
    hooks: Hooks<C>,
) -> Measured<Placed<C, Cv>> {
    project_with_drawing_memo(description, tcx, hooks, &DrawingMemo::default(), None)
}

pub(crate) fn project_with_drawing_memo<C: 'static, Cv: Canvas + 'static>(
    description: ProjectDescription<'_, C>,
    tcx: &mut TextCtx,
    hooks: Hooks<C>,
    drawing_memo: &DrawingMemo,
    scrub_spelling: Option<(&[Step], &str)>,
) -> Measured<Placed<C, Cv>> {
    drawing_memo.begin();
    let ProjectDescription {
        sources,
        root,
        root_path,
        selection,
        source_selection,
        annotations,
        raw,
        styles,
        width,
        root_projection,
        projection,
        root_completions,
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
        drawing_memo,
        // Other projections of the selected cell are secondary. The
        // HOVERED value's faint marks come from the render pass's Ink.
        secondary: secondary_of(&sources, selection),
        selected_trace: source_selection
            .map(|selection| SourceTrace::from_path(&sources, Rc::from(selection.path()))),
    };
    // An empty document is a selectable placeholder at the root path.
    let mut build = ChoiceBuild::default();
    let mut traversal = Traversal::default();
    if matches!(root_path.last(), Some(Step::Follow(_)))
        && let Some(cell) = sources
            .resolve_path(&root_path[..root_path.len() - 1])
            .and_then(Value::as_cell)
    {
        traversal.cells.insert(cell);
        traversal.enclosing = Some((cell, root_path.len()));
    }
    let projected = root_projection.zip(root).map(|(function, root)| {
        let arguments = [(presentation::vocabulary::VALUE, root.clone())];
        grap::apply(
            function,
            arguments,
            |cell| resolved_definitions(&cx, cell),
            grap::DEFAULT_FUEL,
        )
    });
    let layout =
        match projected.filter(|evaluation| !projection_is_absent(&evaluation.result)) {
            Some(evaluation) => prepare_transient_root(
                &cx,
                projection,
                tcx,
                root_path,
                evaluation.result,
                evaluation.remaining_fuel,
                &hooks,
                &mut build,
            ),
            None if root.is_none() && root_completions.is_some() => ChoiceLayout::fixed(
                pending_view(&cx, tcx, root_path.to_vec(), root_completions, &hooks),
            ),
            None => prepare_location(
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
            ),
        };
    let resolved = resolve_choices(
        ChoiceGraph {
            root: layout,
            shared: build.shared,
            choice_count: build.next_choice,
        },
        width,
    );
    drawing_memo.finish();
    resolved
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
    delete: Rc<dyn Fn(&mut C) -> bool>,
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
    delete: Rc<dyn Fn(&mut C) -> bool>,
    child: Measured<Placed<C, Cv>>,
) -> Measured<Placed<C, Cv>> {
    on_key(child, move |ctx, event| {
        crate::modifiers::plain(&event.modifiers)
            && matches!(
                &event.key,
                Key::Named(NamedKey::Backspace | NamedKey::Delete)
            )
            && event.state.is_down()
            && delete(ctx)
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
    let origin = path.to_vec();
    let select = hooks.select.clone();
    let select_origin = origin.clone();
    let select_payload = hooks.select_payload.clone();
    let payload_origin = origin.clone();
    let result_hooks = Hooks {
        select: Rc::new(move |ctx, _| select(ctx, select_origin.clone())),
        select_payload: Rc::new(move |ctx, _, payload| {
            select_payload(ctx, payload_origin.clone(), payload)
        }),
        start_edit: Rc::new(|_, _, _| {}),
        toggle: Rc::new(|_, _| {}),
        update_state: hooks.update_state.clone(),
        edit: Rc::new(|_| None),
        pick: hooks.pick.clone(),
        insert: Rc::new(|_, _| {}),
        delete: Rc::new(|_| false),
        apply: hooks.apply.clone(),
        point: hooks.point.clone(),
        commit_offer: hooks.commit_offer.clone(),
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
        drawing_memo: cx.drawing_memo,
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
    if matches!(step, Step::Follow(_))
        && let Some(parent) = parent
    {
        let mut ancestors = ancestors.clone();
        if let Some(cell) = parent.as_cell() {
            ancestors.cells.insert(cell);
            ancestors.enclosing = Some((cell, path.len()));
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
            None => ChoiceLayout::fixed(pending_view(
                cx,
                tcx,
                path.to_vec(),
                ancestors.completions.as_ref(),
                hooks,
            )),
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
    let inner = prepare(
        cx,
        projection,
        tcx,
        path,
        ancestors,
        hooks,
        Some(value),
        layout,
        build,
    );
    let landmark_path: SharedPath = Rc::from(path);
    let secondary = Secondary::from_context(landmark_path.clone(), value, ancestors.enclosing);
    // Other projections of the selected location carry the secondary
    // mark; the selected one has the primary highlight.
    let inner = if cx.selected(path) {
        inner
    } else if let Some(secondary) = secondary {
        let strong = cx.secondary.as_ref() == Some(&secondary);
        let scale = cx.styles.scale;
        ChoiceLayout::map(inner, 0.0, move |inner| {
            secondary_mark_with(strong, scale, secondary, inner)
        })
    } else {
        inner
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
    let project_layout = || {
        projection
            .and_then(|projection| {
                // Editor state arrives positionally: the payload only at
                // the selected path, the annotations only at this one.
                let selection = cx
                    .selection
                    .filter(|current| current.path() == path)
                    .map(Selection::payload);
                let state = cx.annotations.at(path);
                let target = |steps| projection_target(path, hooks, steps);
                projection.apply(
                    &ProjectEnv { cx },
                    value,
                    cx.styles.scale,
                    !cx.source.transient() && writable_at(&cx.sources, path),
                    selection,
                    state,
                    progred_display::ProjectionTargets::new(&target),
                )
            })
            .unwrap_or_else(|| structure::of(cx, path, value, hooks))
    };
    project_layout()
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

/// An EMPTY SLOT at `path`: the [`placeholder`] widget wired to this
/// projection — engagement derived from the selection, wrapped as an
/// ordinary descend so it highlights, clicks, and navigates like the
/// value it may become. Engaged, its placement emits the completion
/// floating completion card.
fn pending_view<C: 'static, Cv: Canvas + 'static>(
    cx: &Cx,
    tcx: &mut TextCtx,
    path: Path,
    completions: Option<&progred_display::CompletionProvider>,
    hooks: &Hooks<C>,
) -> Measured<Placed<C, Cv>> {
    let engaged = cx
        .selection
        .filter(|current| current.stage() == Stage::Pending && current.path() == path.as_slice())
        .and_then(Selection::edit);
    let content = placeholder(cx, tcx, engaged, false, completions, hooks);
    // Engaged, the generic ring IS the slot's chrome: it draws
    // [`highlight_rect`] over the same frame the cold box strokes,
    // and the same ring survives the commit around the same glyphs —
    // the box never changes, only its paint.
    source_target(cx, path, None, hooks, content)
}

/// The slot widget, in the Puri idiom: its one state input is the
/// engaged pending's query, and None IS the inactive
/// pending — the cold [`placeholder_box`], whose width the engaged
/// query's frame holds as its minimum, so the two forms are one
/// widget in two states and the transition between them is pure
/// chrome. The caller owns identity (descend, highlight, clicks);
/// `labels` picks the slot's role.
fn placeholder<C: 'static, Cv: Canvas + 'static>(
    cx: &Cx,
    tcx: &mut TextCtx,
    engaged: Option<&LineEditState>,
    labels: bool,
    completions: Option<&progred_display::CompletionProvider>,
    hooks: &Hooks<C>,
) -> Measured<Placed<C, Cv>> {
    match engaged {
        Some(query) => query_content(cx, tcx, query, labels, completions, hooks),
        None => placeholder_box(tcx, cx.styles),
    }
}

/// A focused completion query: the editor plus an ordinary floating
/// card. Serves both pending stages — a value and a new field's label
/// (`labels` narrows the offers there).
fn query_content<C: 'static, Cv: Canvas + 'static>(
    cx: &Cx,
    tcx: &mut TextCtx,
    query: &LineEditState,
    labels: bool,
    completions: Option<&progred_display::CompletionProvider>,
    hooks: &Hooks<C>,
) -> Measured<Placed<C, Cv>> {
    // The card and keyboard commit must answer from one list.
    let can_show_everything = !labels && completions.is_some();
    let everything =
        !can_show_everything || cx.selection.is_some_and(Selection::completion_everything);
    let entries = completion_entries_with(
        &cx.sources,
        cx.raw,
        labels,
        query.text(),
        completions,
        everything,
    );
    let fallback = text(tcx, "…", &cx.styles.dim);
    let presentation = edit_presentation(&cx.styles.label);
    let content = atom_content(
        Some(query),
        fallback,
        presentation.clone(),
        None,
        tcx,
        cx.styles,
        hooks,
    );
    // The FRAME holds the slot's width as a minimum — the text field
    // stays content-sized (a blank query is a bare caret), and the
    // frame around it is what never shrinks to a sliver. Framed
    // before the decorate so the floater anchor and the caret clicks
    // span it; the air around it is the caller's [`slot_insets`].
    let content = min_width(slot_width(cx.styles), content);
    let edit = hooks.edit.clone();
    let offers = Offers {
        entries: entries.clone(),
    };
    let scale = cx.styles.scale;
    let trigger = before(content, move |p, placement| {
        let rect = placement.rect;
        *p.completion() = Some(offers);
        // Clicks in the query place the caret, straight through the
        // edit hook — the selection transition is never involved, so
        // clicking what you are typing can't discard it.
        hover_block(p, placement);
        let edit = edit.clone();
        p.handler().on_pointer_down(move |ctx, event| {
            is_primary_contact(event)
                && placement.contains(Point::new(event.state.position.x, event.state.position.y))
                && edit(ctx).is_some_and(|edit| {
                    edit.state.pointer_down(
                        &presentation,
                        edit.fonts,
                        edit.layouts,
                        scale as f32,
                        LineEditPointerDown {
                            point: Point::new(
                                event.state.position.x - rect.x0,
                                event.state.position.y - rect.y0,
                            ),
                            shift: event.state.modifiers.shift(),
                            count: event.state.count.max(1),
                        },
                    );
                    true
                })
        });
    });
    let commit_offer = hooks.commit_offer.clone();
    let choice = cx.selection.map(Selection::choice).unwrap_or(0);
    let scroll = cx
        .selection
        .map(Selection::completion_scroll)
        .unwrap_or(0.0);
    let set_completion_view = hooks.set_completion_view.clone();
    let card = completion_card(
        tcx,
        cx.styles,
        &entries,
        choice,
        scroll,
        everything,
        move |world, action| commit_offer(world, action),
        move |world, scroll, choice, everything| {
            set_completion_view(world, scroll, choice, everything)
        },
    );
    placed::popover(trigger, card, 4.0 * scale)
}

/// The drawn completion card: entry rows under the pending anchor,
/// the chosen one highlighted, styled by what each entry commits.
/// Its floater is raised after the body, so its handlers win: clicking
/// a row commits it, and the card swallows every other click so
/// nothing lands on content underneath.
pub fn completion_card<C: 'static, Cv: Canvas + 'static>(
    tcx: &mut TextCtx,
    styles: &Styles,
    entries: &[Entry],
    choice: usize,
    scroll: f64,
    everything: bool,
    commit: impl Fn(&mut C, &EntryAction) + Clone + 'static,
    set_view: impl Fn(&mut C, f64, usize, bool) + 'static,
) -> Measured<Placed<C, Cv>> {
    let scale = styles.scale;
    let choice = choice.min(entries.len().saturating_sub(1));
    // Cells first, so rows can pad out to the widest and the chosen
    // highlight spans the card, not just its own content.
    let cells: Vec<(Measured<Placed<C, Cv>>, Option<Measured<Placed<C, Cv>>>)> = entries
        .iter()
        .map(|entry| {
            let style = match &entry.action {
                EntryAction::Value(value) if text::read(value).is_some() => &styles.string,
                EntryAction::Value(value) if value.as_blob().is_some() => &styles.id,
                EntryAction::Value(_) if entry.id => &styles.id,
                EntryAction::Value(_) => &styles.label,
                EntryAction::NewLabel(_)
                | EntryAction::NewCell
                | EntryAction::NewList
                | EntryAction::NewRecord => &styles.dim,
            };
            let display = highlighted(tcx, &entry.display, &entry.matches, style);
            let detail = entry
                .detail
                .as_ref()
                .map(|detail| text(tcx, detail, &styles.id));
            (display, detail)
        })
        .collect();
    let widths: Vec<f64> = cells
        .iter()
        .map(|(display, detail)| {
            display.extent.width
                + detail
                    .as_ref()
                    .map_or(0.0, |detail| 8.0 * scale + detail.extent.width)
        })
        .collect();
    let more_label = (!everything).then(|| text::<C, Cv>(tcx, "…", &styles.dim));
    let max_width = widths
        .iter()
        .copied()
        .chain(more_label.iter().map(|label| label.extent.width))
        .fold(0.0, f64::max);
    let rows: Vec<Measured<Placed<C, Cv>>> = cells
        .into_iter()
        .zip(widths)
        .enumerate()
        .map(|(index, ((display, detail), width))| {
            let mut cells: Vec<Measured<Placed<C, Cv>>> = vec![display];
            if let Some(detail) = detail {
                cells.push(detail);
            }
            let content = pad(
                Insets::new(
                    8.0 * scale,
                    2.0 * scale,
                    8.0 * scale + (max_width - width),
                    2.0 * scale,
                ),
                row(8.0 * scale, cells),
            );
            let chosen = index == choice;
            let action = entries[index].action.clone();
            let commit = commit.clone();
            before(content, move |p, placement| {
                let rect = placement.rect;
                p.ink(move |cv, ink| {
                    let lit = !chosen
                        && matches!(tree_hovered(ink), Some(Hover::Entry(i)) if *i == index);
                    if chosen {
                        cv.fill(
                            RoundedRect::from_rect(rect, 4.0 * scale),
                            Color::new([0.0, 0.48, 1.0, 0.14]),
                            Affine::IDENTITY,
                        );
                    } else if lit {
                        cv.fill(
                            RoundedRect::from_rect(rect, 4.0 * scale),
                            Color::new([0.0, 0.48, 1.0, 0.08]),
                            Affine::IDENTITY,
                        );
                    }
                });
                hover_claim(p, placement, Hover::Entry(index));
                let target = Hovered::Tree(Hover::Entry(index));
                let pick_commit = commit.clone();
                let pick_action = action.clone();
                p.activate(target.clone(), move |ctx| {
                    commit(ctx, &action);
                    true
                });
                p.pick(target, move |ctx| {
                    pick_commit(ctx, &pick_action);
                    true
                });
            })
        })
        .collect();
    let gap = 2.0 * scale;
    let row_spans = completion_row_spans(&rows, gap, scale);
    let content = col(0, gap, rows);
    let viewport_height = completion_viewport_height(&row_spans);
    let maximum = (content.extent.height() / scale - viewport_height).max(0.0);
    let scroll = scroll.clamp(0.0, maximum);
    let viewport_extent = Extent {
        width: content.extent.width,
        ascent: content.extent.ascent.min(viewport_height * scale),
        descent: (viewport_height * scale - content.extent.ascent).max(0.0),
    };
    let set_view = Rc::new(set_view);
    let scroll_view = set_view.clone();
    let scrolled = placed::scrolled_at(
        content,
        Vec2::new(0.0, scroll * scale),
        None,
        move |world, event| {
            let (next, outcome) = crate::frame::scroll_offset(
                Vec2::new(0.0, scroll),
                event,
                scale,
                Size::new(viewport_extent.width, viewport_extent.height()),
                Vec2::new(0.0, maximum),
            );
            if next.y != scroll {
                scroll_view(world, next.y, choice, everything);
            }
            outcome
        },
    );
    let viewport = measured::overlay(
        leaf(viewport_extent, |_, _| {}),
        scrolled,
        move |placement, _, _| Some(placement),
    );
    let more = more_label.map(|label| {
        let width = label.extent.width;
        let more = pad(
            Insets::new(
                8.0 * scale,
                3.0 * scale,
                8.0 * scale + (max_width - width),
                3.0 * scale,
            ),
            label,
        );
        let set_view = set_view.clone();
        before(more, move |p, placement| {
            let target = Hovered::Tree(Hover::MoreCompletions);
            let mine = Hover::MoreCompletions;
            p.ink(move |cv, ink| {
                let hovered = tree_hovered(ink) == Some(&mine);
                if hovered {
                    cv.fill(
                        RoundedRect::from_rect(placement.rect, 4.0 * scale),
                        Color::new([0.0, 0.48, 1.0, 0.08]),
                        Affine::IDENTITY,
                    );
                }
            });
            hover_claim(p, placement, Hover::MoreCompletions);
            let activate = set_view.clone();
            p.activate(target.clone(), move |world| {
                activate(world, 0.0, 0, true);
                true
            });
            p.pick(target, move |world| {
                set_view(world, 0.0, 0, true);
                true
            });
        })
    });
    let card = col(
        0,
        2.0 * scale,
        std::iter::once(viewport).chain(more).collect(),
    );
    let card = pad(Insets::uniform(4.0 * scale), card);
    let count = entries.len();
    let card = on_key(card, move |world, event| {
        if event.state.is_down() && !crate::modifiers::command(&event.modifiers) {
            match event.key {
                Key::Named(direction @ (NamedKey::ArrowUp | NamedKey::ArrowDown)) => {
                    let next = match direction {
                        NamedKey::ArrowUp => choice.saturating_sub(1),
                        _ => choice.saturating_add(1).min(count.saturating_sub(1)),
                    };
                    set_view(
                        world,
                        reveal_completion(scroll, next, &row_spans, viewport_height)
                            .clamp(0.0, maximum),
                        next,
                        everything,
                    );
                    true
                }
                Key::Named(NamedKey::Tab) if !everything => {
                    set_view(world, 0.0, 0, true);
                    true
                }
                _ => false,
            }
        } else {
            false
        }
    });
    before(card, move |p, placement| {
        let rect = placement.rect;
        let shape = RoundedRect::from_rect(rect, 6.0 * scale);
        p.fill(shape, Color::new([1.0, 1.0, 1.0, 1.0]), Affine::IDENTITY);
        p.stroke(
            shape,
            Stroke::new(1.0 * scale),
            Color::new([0.75, 0.77, 0.81, 1.0]),
            Affine::IDENTITY,
        );
        hover_block(p, placement);
    })
}

fn completion_row_spans<C, Cv>(
    rows: &[Measured<Placed<C, Cv>>],
    gap: f64,
    scale: f64,
) -> Vec<(f64, f64)> {
    rows.iter()
        .scan(0.0, |top, row| {
            let span = (*top, *top + row.extent.height() / scale);
            *top = span.1 + gap / scale;
            Some(span)
        })
        .collect()
}

fn completion_viewport_height(spans: &[(f64, f64)]) -> f64 {
    const VISIBLE_ROWS: usize = 8;
    spans
        .get(
            VISIBLE_ROWS
                .saturating_sub(1)
                .min(spans.len().saturating_sub(1)),
        )
        .map_or(0.0, |(_, bottom)| *bottom)
}

fn reveal_completion(
    scroll: f64,
    choice: usize,
    spans: &[(f64, f64)],
    viewport_height: f64,
) -> f64 {
    match spans.get(choice) {
        Some((top, _)) if *top < scroll => *top,
        Some((_, bottom)) if *bottom > scroll + viewport_height => *bottom - viewport_height,
        _ => scroll,
    }
}

/// Entry text with the query's matched spans in bold — the fuzzy
/// filter's byte offsets drawn, not recomputed.
fn highlighted<C: 'static, Cv: Canvas + 'static>(
    tcx: &mut TextCtx,
    s: &str,
    matches: &[filter::Match],
    style: &TextStyle,
) -> Measured<Placed<C, Cv>> {
    if matches.is_empty() {
        return text(tcx, s, style);
    }
    let bold = TextStyle {
        weight: Some(700.0),
        ..style.clone()
    };
    let mut segments: Vec<Measured<Placed<C, Cv>>> = Vec::new();
    let mut at = 0;
    for span in matches {
        if span.start > at {
            segments.push(text(tcx, &s[at..span.start], style));
        }
        segments.push(text(tcx, &s[span.start..span.start + span.len], &bold));
        at = span.start + span.len;
    }
    if at < s.len() {
        segments.push(text(tcx, &s[at..], style));
    }
    row(0.0, segments)
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

/// The new-field label stage engaged, its query wearing the primary
/// ring explicitly: a pending edge has no path of its own for
/// [`descend`] to mark, and the ring spans the QUERY frame alone, the
/// way a value pending's does. Clicks inside belong to the query's
/// own caret target; clicks beside fall through like any pending's.
fn label_query<C: 'static, Cv: Canvas + 'static>(
    cx: &Cx,
    tcx: &mut TextCtx,
    query: &LineEditState,
    hooks: &Hooks<C>,
) -> Measured<Placed<C, Cv>> {
    let scale = cx.styles.scale;
    let content = placeholder(cx, tcx, Some(query), true, None, hooks);
    let ringed = decorate(content, move |p, rect| {
        primary_highlight(scale, p, rect);
    });
    // The ring's outset rides inside the node, so glued neighbors —
    // the colon, a flat comma — clear its ink.
    pad(Insets::new(4.0 * scale, 0.0, 4.0 * scale, 0.0), ringed)
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

mod structure;

#[cfg(test)]
mod svg_bench;
#[cfg(test)]
mod tests;
