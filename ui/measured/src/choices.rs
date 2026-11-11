//! Settle ordered layout alternatives over already measured leaves.

use crate::{Extent, Measured};
use kurbo::Insets;
use std::collections::HashMap;

#[derive(Clone, Copy)]
struct Widths {
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

/// What settling decided: the option each alternatives node takes, by its
/// id, and how many children each line of a flow holds, by its id. A flow
/// with no lines recorded is on one.
struct Chosen {
    options: Vec<usize>,
    lines: Vec<Vec<usize>>,
}

/// A measured layout whose responsive choices have not been settled.
/// Fixed leaves already own their placement continuations; selecting
/// forms therefore combines widths only and never remeasures text
/// or reruns a projection.
pub struct ChoiceLayout<Out> {
    widths: Widths,
    kind: ChoiceKind<Out>,
}

/// A layout whose choices are made: `ChoiceLayout::settle` leaves only fixed
/// leaves and containers of settled children, each with its extent.
struct Settled<Out> {
    extent: Extent,
    kind: SettledKind<Out>,
}

enum SettledKind<Out> {
    Fixed(Measured<Out>),
    Row {
        alignment: crate::RowAlignment,
        gap: f64,
        children: Vec<Settled<Out>>,
    },
    Col {
        baseline: usize,
        gap: f64,
        children: Vec<Settled<Out>>,
    },
    Layers {
        children: Vec<Settled<Out>>,
    },
    Pad {
        insets: Insets,
        child: Box<Settled<Out>>,
    },
}

type MeasureMap<Out> = Box<dyn FnOnce(Measured<Out>) -> Measured<Out>>;

enum ChoiceKind<Out> {
    Fixed(Measured<Out>),
    Use(usize),
    Map {
        child: Box<ChoiceLayout<Out>>,
        map: MeasureMap<Out>,
    },
    Row {
        alignment: crate::RowAlignment,
        gap: f64,
        children: Vec<ChoiceLayout<Out>>,
    },
    Col {
        baseline: usize,
        gap: f64,
        children: Vec<ChoiceLayout<Out>>,
    },
    Layers {
        children: Vec<ChoiceLayout<Out>>,
    },
    Attached {
        trigger: Box<ChoiceLayout<Out>>,
        content: Box<ChoiceLayout<Out>>,
        map: Box<dyn FnOnce(Measured<Out>, Measured<Out>) -> Measured<Out>>,
    },
    Pad {
        insets: Insets,
        child: Box<ChoiceLayout<Out>>,
    },
    /// Children side by side, taking another line where one is full, as
    /// words do in a paragraph.
    Flow {
        id: usize,
        gap: f64,
        line_gap: f64,
        children: Vec<ChoiceLayout<Out>>,
        /// How wide a line may be however much room there is.
        ribbon: f64,
    },
    Alternatives {
        id: usize,
        options: Vec<ChoiceLayout<Out>>,
        /// How wide an option but the last may be and still be chosen,
        /// however much room there is: its own width, from where it starts,
        /// so indentation doesn't count against it.
        ribbon: f64,
    },
}

pub struct ChoiceBuild<Out> {
    /// The ribbon of the alternatives built here.
    ribbon: f64,
    pending: Vec<Option<ChoiceLayout<Out>>>,
    next_choice: usize,
    next_flow: usize,
    shared_ids: HashMap<usize, usize>,
    shared: Vec<Option<ChoiceLayout<Out>>>,
}

impl<Out> Default for ChoiceBuild<Out> {
    fn default() -> Self {
        Self {
            ribbon: f64::INFINITY,
            pending: Vec::new(),
            next_choice: 0,
            next_flow: 0,
            shared_ids: HashMap::new(),
            shared: Vec::new(),
        }
    }
}

impl<Out> ChoiceBuild<Out> {
    /// A build whose alternatives prefer an option only while it's no wider
    /// than `ribbon`: text is read across its own width, not the window's,
    /// so a line that fits the window can still be too long. With no ribbon,
    /// the default, room is the only limit.
    pub fn with_ribbon(ribbon: f64) -> Self {
        Self {
            ribbon,
            ..Self::default()
        }
    }
}

pub struct ChoiceGraph<Out> {
    root: ChoiceLayout<Out>,
    shared: Vec<Option<ChoiceLayout<Out>>>,
    choice_count: usize,
    flow_count: usize,
}

impl<Out: 'static> ChoiceBuild<Out> {
    /// Temporary results of builder calls, consumed by their parent operation.
    pub fn push(&mut self, layout: ChoiceLayout<Out>) -> usize {
        let id = self.pending.len();
        self.pending.push(Some(layout));
        id
    }

    pub fn take(&mut self, id: usize) -> ChoiceLayout<Out> {
        let result = self.pending[id]
            .take()
            .expect("a builder result is consumed once");
        while matches!(self.pending.last(), Some(None)) {
            self.pending.pop();
        }
        result
    }

    /// Reuse one measured child across mutually exclusive forms. The
    /// caller's key identifies its shared description, not a layout slot.
    /// A settled form must use each shared child at most once.
    pub fn shared(
        &mut self,
        key: usize,
        prepare: impl FnOnce(&mut Self) -> ChoiceLayout<Out>,
    ) -> ChoiceLayout<Out> {
        match self.shared_ids.get(&key).copied() {
            Some(id) => ChoiceLayout::used(
                id,
                self.shared[id]
                    .as_ref()
                    .expect("a shared layout is prepared before reuse")
                    .widths,
            ),
            None => {
                let child = prepare(self);
                let id = self.shared.len();
                let widths = child.widths;
                self.shared.push(Some(child));
                self.shared_ids.insert(key, id);
                ChoiceLayout::used(id, widths)
            }
        }
    }

    pub fn alternatives(&mut self, options: Vec<ChoiceLayout<Out>>) -> ChoiceLayout<Out> {
        let id = self.next_choice;
        self.next_choice += 1;
        ChoiceLayout::alternatives(id, options, self.ribbon)
    }

    /// `children` side by side, `gap` apart, taking another line `line_gap`
    /// below where one is full: as wide as the room it has, or its ribbon.
    pub fn flow(
        &mut self,
        gap: f64,
        line_gap: f64,
        children: Vec<ChoiceLayout<Out>>,
    ) -> ChoiceLayout<Out> {
        let id = self.next_flow;
        self.next_flow += 1;
        let gaps = gap * children.len().saturating_sub(1) as f64;
        ChoiceLayout {
            // On one line where there's room; no narrower than its widest
            // child alone.
            widths: Widths {
                preferred: children
                    .iter()
                    .map(|child| child.widths.preferred)
                    .sum::<f64>()
                    + gaps,
                minimum: children
                    .iter()
                    .map(|child| child.widths.minimum)
                    .fold(0.0_f64, f64::max),
            },
            kind: ChoiceKind::Flow {
                id,
                gap,
                line_gap,
                children,
                ribbon: self.ribbon,
            },
        }
    }

    pub fn finish(self, root: ChoiceLayout<Out>) -> ChoiceGraph<Out> {
        assert!(
            self.pending.is_empty(),
            "all builder results must belong to the root"
        );
        ChoiceGraph {
            root,
            shared: self.shared,
            choice_count: self.next_choice,
            flow_count: self.next_flow,
        }
    }
}

impl<Out: 'static> ChoiceLayout<Out> {
    pub fn fixed(measured: Measured<Out>) -> Self {
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

    pub fn map(child: Self, map: impl FnOnce(Measured<Out>) -> Measured<Out> + 'static) -> Self {
        Self {
            widths: child.widths,
            kind: ChoiceKind::Map {
                child: Box::new(child),
                map: Box::new(map),
            },
        }
    }

    pub fn aligned_row(alignment: crate::RowAlignment, gap: f64, children: Vec<Self>) -> Self {
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

    pub fn col(baseline: usize, gap: f64, children: Vec<Self>) -> Self {
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

    pub fn layers(children: Vec<Self>) -> Self {
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
            kind: ChoiceKind::Layers { children },
        }
    }

    /// Settle an out-of-flow child without adding its width to the base.
    /// The consumer combines their placement continuations.
    pub fn attach(
        trigger: Self,
        content: Self,
        map: impl FnOnce(Measured<Out>, Measured<Out>) -> Measured<Out> + 'static,
    ) -> Self {
        Self {
            widths: trigger.widths,
            kind: ChoiceKind::Attached {
                trigger: Box::new(trigger),
                content: Box::new(content),
                map: Box::new(map),
            },
        }
    }

    pub fn pad(insets: Insets, child: Self) -> Self {
        let widths = child.widths.plus(insets.x0 + insets.x1);
        Self {
            widths,
            kind: ChoiceKind::Pad {
                insets,
                child: Box::new(child),
            },
        }
    }

    fn alternatives(id: usize, options: Vec<Self>, ribbon: f64) -> Self {
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
            kind: ChoiceKind::Alternatives {
                id,
                options,
                ribbon,
            },
        }
    }

    fn select(&self, choices: &mut Chosen, shared: &[Option<Self>], available: f64) -> f64 {
        match &self.kind {
            ChoiceKind::Fixed(measured) => measured.extent.width,
            ChoiceKind::Use(id) => shared[*id]
                .as_ref()
                .map_or(0.0, |child| child.select(choices, shared, available)),
            ChoiceKind::Map { child, .. } => child.select(choices, shared, available),
            ChoiceKind::Row { gap, children, .. } => {
                let gaps = gap * children.len().saturating_sub(1) as f64;
                // Each child may use its minimum plus what its elder
                // siblings left unused.
                children
                    .iter()
                    .scan(
                        (available - self.widths.minimum).max(0.0),
                        |slack, child| {
                            let budget = child.widths.minimum + *slack;
                            let width = child.select(choices, shared, budget);
                            *slack = (budget - width).max(0.0);
                            Some(width)
                        },
                    )
                    .sum::<f64>()
                    + gaps
            }
            ChoiceKind::Col { children, .. } | ChoiceKind::Layers { children } => children
                .iter()
                .map(|child| child.select(choices, shared, available))
                .fold(0.0_f64, f64::max),
            ChoiceKind::Flow {
                id,
                gap,
                children,
                ribbon,
                ..
            } => {
                let across = available.min(*ribbon);
                let mut lines = Vec::new();
                let (mut widest, mut line, mut count) = (0.0_f64, 0.0_f64, 0);
                for child in children {
                    let width = child.select(choices, shared, available);
                    if count > 0 && line + gap + width > across {
                        lines.push(count);
                        widest = widest.max(line);
                        (line, count) = (width, 1);
                    } else {
                        line += if count > 0 { *gap } else { 0.0 } + width;
                        count += 1;
                    }
                }
                lines.push(count);
                choices.lines[*id] = lines;
                widest.max(line)
            }
            ChoiceKind::Attached {
                trigger, content, ..
            } => {
                content.select(choices, shared, available);
                trigger.select(choices, shared, available)
            }
            ChoiceKind::Pad { insets, child } => {
                let horizontal = insets.x0 + insets.x1;
                child.select(choices, shared, (available - horizontal).max(0.0)) + horizontal
            }
            ChoiceKind::Alternatives {
                id,
                options,
                ribbon,
            } => match options.split_last() {
                None => 0.0,
                Some((accommodating, preferred)) if available <= 0.0 => {
                    choices.options[*id] = preferred.len();
                    accommodating.select(choices, shared, available)
                }
                Some((accommodating, preferred)) => {
                    match preferred
                        .iter()
                        .enumerate()
                        .find(|(_, option)| option.widths.preferred <= available.min(*ribbon))
                    {
                        Some((index, option)) => {
                            choices.options[*id] = index;
                            option.widths.preferred
                        }
                        None => {
                            choices.options[*id] = preferred.len();
                            let accommodating_width =
                                accommodating.select(choices, shared, available);
                            let width = preferred.iter().enumerate().rev().fold(
                                accommodating_width,
                                |best_width, (index, option)| {
                                    if option.widths.preferred <= best_width {
                                        choices.options[*id] = index;
                                        option.widths.preferred
                                    } else {
                                        best_width
                                    }
                                },
                            );
                            // Exploring the accommodating form may have
                            // narrowed children it shares with the winner.
                            if let Some(option) = preferred.get(choices.options[*id]) {
                                option.prefer(choices, shared);
                            }
                            width
                        }
                    }
                }
            },
        }
    }

    /// Settle this form at its preferred width: every nested alternative
    /// takes its first form.
    fn prefer(&self, choices: &mut Chosen, shared: &[Option<Self>]) {
        match &self.kind {
            ChoiceKind::Fixed(_) => {}
            ChoiceKind::Use(id) => {
                if let Some(child) = &shared[*id] {
                    child.prefer(choices, shared);
                }
            }
            ChoiceKind::Map { child, .. } | ChoiceKind::Pad { child, .. } => {
                child.prefer(choices, shared)
            }
            ChoiceKind::Row { children, .. }
            | ChoiceKind::Col { children, .. }
            | ChoiceKind::Layers { children } => {
                for child in children {
                    child.prefer(choices, shared);
                }
            }
            ChoiceKind::Flow { id, children, .. } => {
                choices.lines[*id].clear();
                for child in children {
                    child.prefer(choices, shared);
                }
            }
            ChoiceKind::Attached {
                trigger, content, ..
            } => {
                trigger.prefer(choices, shared);
                content.prefer(choices, shared);
            }
            ChoiceKind::Alternatives { id, options, .. } => {
                choices.options[*id] = 0;
                if let Some(first) = options.first() {
                    first.prefer(choices, shared);
                }
            }
        }
    }

    fn settle(self, choices: &Chosen, shared: &mut [Option<Self>]) -> Settled<Out>
    where
        Out: crate::ObserveLayout,
    {
        match self.kind {
            ChoiceKind::Use(id) => shared[id]
                .take()
                .expect("a shared layout is consumed by only one selected form")
                .settle(choices, shared),
            ChoiceKind::Alternatives { id, options, .. } => options
                .into_iter()
                .nth(choices.options[id])
                .map(|option| option.settle(choices, shared))
                .unwrap_or_else(|| {
                    Settled::new(SettledKind::Fixed(crate::leaf_into(
                        Extent::default(),
                        |_, _| {},
                    )))
                }),
            ChoiceKind::Map { child, map } => Settled::new(SettledKind::Fixed(map(child
                .settle(choices, shared)
                .into_measured()))),
            ChoiceKind::Attached {
                trigger,
                content,
                map,
            } => Settled::new(SettledKind::Fixed(map(
                trigger.settle(choices, shared).into_measured(),
                content.settle(choices, shared).into_measured(),
            ))),
            ChoiceKind::Row {
                alignment,
                gap,
                children,
            } => Settled::new(SettledKind::Row {
                alignment,
                gap,
                children: settle_all(children, choices, shared),
            }),
            ChoiceKind::Col {
                baseline,
                gap,
                children,
            } => Settled::new(SettledKind::Col {
                baseline,
                gap,
                children: settle_all(children, choices, shared),
            }),
            ChoiceKind::Layers { children } => Settled::new(SettledKind::Layers {
                children: settle_all(children, choices, shared),
            }),
            ChoiceKind::Flow {
                id,
                gap,
                line_gap,
                children,
                ..
            } => {
                let mut children = settle_all(children, choices, shared).into_iter();
                let line = |children: Vec<Settled<Out>>| {
                    Settled::new(SettledKind::Row {
                        alignment: crate::RowAlignment::Baseline,
                        gap,
                        children,
                    })
                };
                match choices.lines[id].as_slice() {
                    [] | [_] => line(children.collect()),
                    lines => Settled::new(SettledKind::Col {
                        baseline: 0,
                        gap: line_gap,
                        children: lines
                            .iter()
                            .map(|count| line(children.by_ref().take(*count).collect()))
                            .collect(),
                    }),
                }
            }
            ChoiceKind::Pad { insets, child } => Settled::new(SettledKind::Pad {
                insets,
                child: Box::new(child.settle(choices, shared)),
            }),
            ChoiceKind::Fixed(measured) => Settled::new(SettledKind::Fixed(measured)),
        }
    }
}

impl<Out: crate::ObserveLayout + 'static> Settled<Out> {
    fn new(kind: SettledKind<Out>) -> Self {
        Self {
            extent: match &kind {
                SettledKind::Fixed(measured) => measured.extent,
                SettledKind::Row {
                    alignment,
                    gap,
                    children,
                } => crate::row_extent(*gap, *alignment, children.iter().map(|child| child.extent)),
                SettledKind::Col {
                    baseline,
                    gap,
                    children,
                } => crate::col_extent(*baseline, *gap, children.iter().map(|child| child.extent)),
                SettledKind::Layers { children } => {
                    crate::layers_extent(children.iter().map(|child| child.extent))
                }
                SettledKind::Pad { insets, child } => crate::padded_extent(*insets, child.extent),
            },
            kind,
        }
    }

    fn into_measured(self) -> Measured<Out> {
        match self.kind {
            SettledKind::Fixed(measured) => measured,
            _ => crate::leaf_into(self.extent, move |placement, out| {
                self.place(placement, out)
            }),
        }
    }

    fn place(self, placement: uig::Placement, out: &mut Out) {
        match self.kind {
            SettledKind::Fixed(measured) => crate::place_into(measured, placement, out),
            SettledKind::Row {
                alignment,
                gap,
                children,
            } => out.layout(crate::Composition::Row(alignment), |out| {
                crate::place_row(
                    self.extent,
                    placement,
                    gap,
                    alignment,
                    children,
                    |child| child.extent,
                    |child, placement| out.layout_child(|out| child.place(placement, out)),
                )
            }),
            SettledKind::Col {
                baseline,
                gap,
                children,
            } => out.layout(crate::Composition::Column { baseline }, |out| {
                crate::place_col(
                    placement,
                    gap,
                    children,
                    |child| child.extent,
                    |child, placement| out.layout_child(|out| child.place(placement, out)),
                )
            }),
            SettledKind::Layers { children } => out.layout(crate::Composition::Layers, |out| {
                crate::place_layers(
                    self.extent,
                    placement,
                    children,
                    |child| child.extent,
                    |child, placement| out.layout_child(|out| child.place(placement, out)),
                )
            }),
            SettledKind::Pad { insets, child } => {
                let placement = crate::padded_placement(placement, insets, child.extent);
                child.place(placement, out);
            }
        }
    }
}

fn settle_all<Out: crate::ObserveLayout + 'static>(
    children: Vec<ChoiceLayout<Out>>,
    choices: &Chosen,
    shared: &mut [Option<ChoiceLayout<Out>>],
) -> Vec<Settled<Out>> {
    children
        .into_iter()
        .map(|child| child.settle(choices, shared))
        .collect()
}

pub fn resolve_choices<Out: crate::ObserveLayout + 'static>(
    graph: ChoiceGraph<Out>,
    available: f64,
) -> Measured<Out> {
    let ChoiceGraph {
        root,
        mut shared,
        choice_count,
        flow_count,
    } = graph;
    let mut choices = Chosen {
        options: vec![0; choice_count],
        lines: vec![Vec::new(); flow_count],
    };
    root.select(&mut choices, &shared, available);
    root.settle(&choices, &mut shared).into_measured()
}

#[cfg(test)]
#[path = "tests/choices/choice_tests.rs"]
mod choice_tests;
