//! Settle ordered layout alternatives over already measured leaves.

use kurbo::Insets;
use measured::{Measured, centered_row, col, layers, pad, row};
use std::collections::HashMap;

#[derive(Clone, Copy)]
pub(super) struct Widths {
    preferred: f64,
    minimum: f64,
}

impl Widths {
    fn fixed(width: f64) -> Self {
        Self {
            preferred: width,
            minimum: width,
        }
    }

    fn plus(self, width: f64) -> Self {
        Self {
            preferred: self.preferred + width,
            minimum: self.minimum + width,
        }
    }
}

/// A measured layout whose responsive choices have not been settled.
/// Fixed leaves already own their placement continuations; selecting
/// forms therefore combines widths only and never remeasures text
/// or reruns a projection.
pub(super) struct ChoiceLayout<Out> {
    pub(super) widths: Widths,
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

pub(super) struct ChoiceBuild<Out> {
    pub(super) next_choice: usize,
    pub(super) shared_ids: HashMap<usize, usize>,
    pub(super) shared: Vec<Option<ChoiceLayout<Out>>>,
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

pub(super) struct ChoiceGraph<Out> {
    pub(super) root: ChoiceLayout<Out>,
    pub(super) shared: Vec<Option<ChoiceLayout<Out>>>,
    pub(super) choice_count: usize,
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
    pub(super) fn fixed(measured: Measured<Out>) -> Self {
        Self {
            widths: Widths::fixed(measured.extent.width),
            kind: ChoiceKind::Fixed(measured),
        }
    }

    pub(super) fn used(id: usize, widths: Widths) -> Self {
        Self {
            widths,
            kind: ChoiceKind::Use(id),
        }
    }

    pub(super) fn map(
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

    pub(super) fn aligned_row(
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

    pub(super) fn col(baseline: usize, gap: f64, children: Vec<Self>) -> Self {
        let widths = Widths {
            preferred: children
                .iter()
                .map(|child| child.widths.preferred)
                .fold(0.0_f64, f64::max),
            minimum: children
                .iter()
                .map(|child| child.widths.minimum)
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

    pub(super) fn overlay(children: Vec<Self>) -> Self {
        let widths = Widths {
            preferred: children
                .iter()
                .map(|child| child.widths.preferred)
                .fold(0.0_f64, f64::max),
            minimum: children
                .iter()
                .map(|child| child.widths.minimum)
                .fold(0.0_f64, f64::max),
        };
        Self {
            widths,
            kind: ChoiceKind::Overlay { children },
        }
    }

    pub(super) fn popover(
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

    pub(super) fn pad(insets: Insets, child: Self) -> Self {
        let widths = child.widths.plus(insets.x0 + insets.x1);
        Self {
            widths,
            kind: ChoiceKind::Pad {
                insets,
                child: Box::new(child),
            },
        }
    }

    pub(super) fn alternatives(id: usize, options: Vec<Self>) -> Self {
        let widths = match options.first() {
            Some(first) => Widths {
                preferred: first.widths.preferred,
                minimum: options
                    .iter()
                    .map(|option| option.widths.minimum)
                    .fold(f64::INFINITY, f64::min),
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

pub(super) fn resolve_choices<Out: measured::Output + 'static>(
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
    if tracing {
        eprintln!(
            "layout analysis: nodes={} alternatives={} wider_backups={} preferred={preferred:.1} minimum={minimum:.1} available={available:.1}",
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
    use measured::Extent;

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
