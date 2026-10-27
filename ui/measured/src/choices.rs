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

/// A measured layout whose responsive choices have not been settled.
/// Fixed leaves already own their placement continuations; selecting
/// forms therefore combines widths only and never remeasures text
/// or reruns a projection.
pub struct ChoiceLayout<Out> {
    widths: Widths,
    extent: Extent,
    kind: ChoiceKind<Out>,
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
    Alternatives {
        id: usize,
        options: Vec<ChoiceLayout<Out>>,
    },
}

pub struct ChoiceBuild<Out> {
    pending: Vec<Option<ChoiceLayout<Out>>>,
    next_choice: usize,
    shared_ids: HashMap<usize, usize>,
    shared: Vec<Option<ChoiceLayout<Out>>>,
}

impl<Out> Default for ChoiceBuild<Out> {
    fn default() -> Self {
        Self {
            pending: Vec::new(),
            next_choice: 0,
            shared_ids: HashMap::new(),
            shared: Vec::new(),
        }
    }
}

pub struct ChoiceGraph<Out> {
    root: ChoiceLayout<Out>,
    shared: Vec<Option<ChoiceLayout<Out>>>,
    choice_count: usize,
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
        ChoiceLayout::alternatives(id, options)
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
        }
    }
}

impl<Out: 'static> ChoiceLayout<Out> {
    pub fn fixed(measured: Measured<Out>) -> Self {
        Self {
            widths: Widths::fixed(measured.extent.width),
            extent: Extent::default(),
            kind: ChoiceKind::Fixed(measured),
        }
    }

    fn used(id: usize, widths: Widths) -> Self {
        Self {
            widths,
            extent: Extent::default(),
            kind: ChoiceKind::Use(id),
        }
    }

    pub fn map(child: Self, map: impl FnOnce(Measured<Out>) -> Measured<Out> + 'static) -> Self {
        Self {
            widths: child.widths,
            extent: Extent::default(),
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
            extent: Extent::default(),
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
            extent: Extent::default(),
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
            extent: Extent::default(),
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
            extent: Extent::default(),
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
            extent: Extent::default(),
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
            },
            None => Widths::fixed(0.0),
        };
        Self {
            widths,
            extent: Extent::default(),
            kind: ChoiceKind::Alternatives { id, options },
        }
    }

    fn select(&self, choices: &mut [usize], shared: &[Option<Self>], available: f64) -> f64 {
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
                            let width = preferred.iter().enumerate().rev().fold(
                                accommodating_width,
                                |best_width, (index, option)| {
                                    if option.widths.preferred <= best_width {
                                        choices[*id] = index;
                                        option.widths.preferred
                                    } else {
                                        best_width
                                    }
                                },
                            );
                            // Exploring the accommodating form may have
                            // narrowed children it shares with the winner.
                            if let Some(option) = preferred.get(choices[*id]) {
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
    fn prefer(&self, choices: &mut [usize], shared: &[Option<Self>]) {
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
            ChoiceKind::Attached {
                trigger, content, ..
            } => {
                trigger.prefer(choices, shared);
                content.prefer(choices, shared);
            }
            ChoiceKind::Alternatives { id, options } => {
                choices[*id] = 0;
                if let Some(first) = options.first() {
                    first.prefer(choices, shared);
                }
            }
        }
    }

    fn settle(self, choices: &[usize], shared: &mut [Option<Self>]) -> Self
    where
        Out: crate::ObserveLayout,
    {
        let widths = self.widths;
        let settled = |kind| Self {
            widths,
            extent: settled_extent(&kind),
            kind,
        };
        match self.kind {
            ChoiceKind::Use(id) => shared[id]
                .take()
                .expect("a shared layout is consumed by only one selected form")
                .settle(choices, shared),
            ChoiceKind::Alternatives { id, options } => options
                .into_iter()
                .nth(choices[id])
                .map(|option| option.settle(choices, shared))
                .unwrap_or_else(|| Self::fixed(crate::leaf_into(Extent::default(), |_, _| {}))),
            ChoiceKind::Map { child, map } => settled(ChoiceKind::Fixed(map(child
                .settle(choices, shared)
                .into_measured()))),
            ChoiceKind::Attached {
                trigger,
                content,
                map,
            } => settled(ChoiceKind::Fixed(map(
                trigger.settle(choices, shared).into_measured(),
                content.settle(choices, shared).into_measured(),
            ))),
            ChoiceKind::Row {
                alignment,
                gap,
                children,
            } => settled(ChoiceKind::Row {
                alignment,
                gap,
                children: settle_all(children, choices, shared),
            }),
            ChoiceKind::Col {
                baseline,
                gap,
                children,
            } => settled(ChoiceKind::Col {
                baseline,
                gap,
                children: settle_all(children, choices, shared),
            }),
            ChoiceKind::Layers { children } => settled(ChoiceKind::Layers {
                children: settle_all(children, choices, shared),
            }),
            ChoiceKind::Pad { insets, child } => settled(ChoiceKind::Pad {
                insets,
                child: Box::new(child.settle(choices, shared)),
            }),
            fixed @ ChoiceKind::Fixed(_) => settled(fixed),
        }
    }

    fn into_measured(self) -> Measured<Out>
    where
        Out: crate::ObserveLayout,
    {
        match self.kind {
            ChoiceKind::Fixed(measured) => measured,
            _ => crate::leaf_into(self.extent, move |placement, out| {
                self.place(placement, out)
            }),
        }
    }

    fn place(self, placement: uig::Placement, out: &mut Out)
    where
        Out: crate::ObserveLayout,
    {
        match self.kind {
            ChoiceKind::Fixed(measured) => crate::place_into(measured, placement, out),
            ChoiceKind::Row {
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
            ChoiceKind::Col {
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
            ChoiceKind::Layers { children } => out.layout(crate::Composition::Layers, |out| {
                crate::place_layers(
                    self.extent,
                    placement,
                    children,
                    |child| child.extent,
                    |child, placement| out.layout_child(|out| child.place(placement, out)),
                )
            }),
            ChoiceKind::Pad { insets, child } => {
                let placement = crate::padded_placement(placement, insets, child.extent);
                child.place(placement, out);
            }
            _ => unreachable!("only the settled graph can place"),
        }
    }
}

/// The extent of a node `settle` made: a fixed leaf or a container of settled children.
fn settled_extent<Out>(kind: &ChoiceKind<Out>) -> Extent {
    match kind {
        ChoiceKind::Fixed(measured) => measured.extent,
        ChoiceKind::Row {
            alignment,
            gap,
            children,
        } => crate::row_extent(*gap, *alignment, children.iter().map(|child| child.extent)),
        ChoiceKind::Col {
            baseline,
            gap,
            children,
        } => crate::col_extent(*baseline, *gap, children.iter().map(|child| child.extent)),
        ChoiceKind::Layers { children } => {
            crate::layers_extent(children.iter().map(|child| child.extent))
        }
        ChoiceKind::Pad { insets, child } => crate::padded_extent(*insets, child.extent),
        _ => unreachable!("choices and measurement wrappers have been settled"),
    }
}

fn settle_all<Out: crate::ObserveLayout + 'static>(
    children: Vec<ChoiceLayout<Out>>,
    choices: &[usize],
    shared: &mut [Option<ChoiceLayout<Out>>],
) -> Vec<ChoiceLayout<Out>> {
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
    } = graph;
    let mut choices = vec![0; choice_count];
    root.select(&mut choices, &shared, available);
    root.settle(&choices, &mut shared).into_measured()
}

#[cfg(test)]
#[path = "tests/choices/choice_tests.rs"]
mod choice_tests;
