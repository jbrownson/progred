//! Fine-end-aligned range sliders. The caller supplies ordered occurrence keys
//! and retains selection intent. Choosing another group resets finer rows.
use crate::{
    range_slider::RangeSlider,
    slider::{Slider, TickLevel},
};
use std::collections::BTreeSet;
use std::ops::Range;

pub struct Tree<Key> {
    key: Key,
    leaves: usize,
    height: usize,
    children: Vec<Tree<Key>>,
}

impl<Key> Tree<Key> {
    pub fn leaf(key: Key) -> Self {
        Self {
            key,
            leaves: 1,
            height: 0,
            children: Vec::new(),
        }
    }

    /// Keys must sort in traversal order on every frontier (e.g. list paths).
    pub fn group(key: Key, children: impl IntoIterator<Item = Self>) -> Self {
        let children: Vec<_> = children.into_iter().filter(|c| c.leaves > 0).collect();
        Self {
            key,
            leaves: children.iter().map(|c| c.leaves).sum(),
            height: children.iter().map(|c| c.height + 1).max().unwrap_or(0),
            children,
        }
    }
}

fn children<Key>(tree: &Tree<Key>, offset: usize) -> impl Iterator<Item = (&Tree<Key>, usize)> {
    tree.children.iter().scan(offset, |offset, child| {
        let start = *offset;
        *offset += child.leaves;
        Some((child, start))
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum Intent<Key> {
    #[default]
    All,
    /// Both endpoint occurrences are included; missing endpoints remain bounds.
    Between { first: Key, last: Key },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Cursor<Key> {
    pub item: Key,
    pub fraction: f64,
}

impl<Key: Clone + Eq> Tree<Key> {
    pub fn cursor(&self, position: f64) -> Option<Cursor<Key>> {
        if self.leaves == 0 || !position.is_finite() {
            None
        } else if self.children.is_empty() {
            Some(Cursor {
                item: self.key.clone(),
                fraction: position.clamp(0.0, 1.0),
            })
        } else {
            let position = position.clamp(0.0, self.leaves as f64);
            let index = (position.floor() as usize).min(self.leaves - 1);
            children(self, 0).find_map(|(child, offset)| {
                (index < offset + child.leaves)
                    .then(|| child.cursor(position - offset as f64))
                    .flatten()
            })
        }
    }

    pub fn position(&self, cursor: &Cursor<Key>) -> Option<f64> {
        if !cursor.fraction.is_finite() || !(0.0..=1.0).contains(&cursor.fraction) {
            None
        } else if self.children.is_empty() {
            (self.leaves > 0 && self.key == cursor.item).then_some(cursor.fraction)
        } else {
            children(self, 0)
                .find_map(|(child, offset)| child.position(cursor).map(|p| p + offset as f64))
        }
    }
}

pub struct Row<Key> {
    /// Remaining depth: zero is the finest row, independent of hidden rows.
    pub level: usize,
    pub slider: RangeSlider,
    items: Vec<Item<Key>>,
}

impl<Key> Row<Key> {
    pub fn keys(&self) -> impl Iterator<Item = &Key> {
        self.items.iter().map(|item| &item.key)
    }
}

struct Item<Key> {
    key: Key,
    leaves: Range<usize>,
}

pub struct Selection<Key> {
    /// Coarsest first; paint in reverse below the continuous slider.
    pub rows: Vec<Row<Key>>,
    pub leaves: Range<usize>,
    /// Finest first. Entries for temporarily hidden rows are retained too.
    pub intent: Vec<Intent<Key>>,
}

impl<Key: Clone + Ord> Selection<Key> {
    pub fn new(tree: &Tree<Key>, stored: &[Intent<Key>]) -> Self {
        let mut intent = stored.to_vec();
        intent.resize_with(intent.len().max(tree.height), || Intent::All);
        let mut leaves = 0..tree.leaves;
        let mut rows = Vec::new();
        let mut level = tree.height.saturating_sub(1);
        let mut frontier: Vec<_> = if tree.children.is_empty() && tree.leaves > 0 {
            vec![(tree, 0)]
        } else {
            children(tree, 0).collect()
        };
        while let Some(all) = RangeSlider::new(frontier.len(), 0..frontier.len()) {
            let slider = match intent.get(level) {
                Some(Intent::Between { first, last }) => RangeSlider::new(
                    frontier.len(),
                    frontier.partition_point(|(node, _)| node.key < *first)
                        ..frontier.partition_point(|(node, _)| node.key <= *last),
                )
                .unwrap_or(all),
                _ => all,
            };
            let nodes = &frontier[slider.selected.clone()];
            if let (Some((_, first)), Some((last, offset))) = (nodes.first(), nodes.last()) {
                leaves = *first..offset + last.leaves;
            }
            let height = nodes.iter().map(|(node, _)| node.height).max().unwrap_or(0);
            rows.push(Row {
                level,
                slider,
                items: frontier
                    .iter()
                    .map(|(node, offset)| Item {
                        key: node.key.clone(),
                        leaves: *offset..offset + node.leaves,
                    })
                    .collect(),
            });
            if height == 0 {
                break;
            }
            let mut next = Vec::new();
            for &(node, offset) in nodes {
                if node.height < height {
                    next.push((node, offset));
                } else {
                    next.extend(children(node, offset));
                }
            }
            frontier = next;
            level = height - 1;
        }
        Self {
            rows,
            leaves,
            intent,
        }
    }

    /// Narrowing to the current item preserves finer selections. A wider range
    /// or another item starts fresh, as does selection without a current item.
    pub fn select(
        &self,
        tree: &Tree<Key>,
        level: usize,
        range: Range<usize>,
        current: Option<usize>,
    ) -> Self {
        let preserve_finer = range.len() == 1 && current == Some(range.start);
        let intent = self
            .rows
            .iter()
            .find(|row| row.level == level)
            .and_then(|row| {
                let items = row.items.get(range)?;
                Some(Intent::Between {
                    first: items.first()?.key.clone(),
                    last: items.last()?.key.clone(),
                })
            });
        match intent {
            Some(intent) => self.with_intent(tree, level, intent, preserve_finer),
            None => Self::new(tree, &self.intent),
        }
    }

    pub fn select_all(&self, tree: &Tree<Key>, level: usize) -> Self {
        self.with_intent(tree, level, Intent::All, false)
    }

    fn with_intent(
        &self,
        tree: &Tree<Key>,
        level: usize,
        value: Intent<Key>,
        preserve_finer: bool,
    ) -> Self {
        let mut intent = self.intent.clone();
        intent.resize_with(intent.len().max(level + 1), || Intent::All);
        if !preserve_finer {
            intent[..level].fill(Intent::All);
        }
        intent[level] = value;
        Self::new(tree, &intent)
    }

    pub fn position(&self, position: f64) -> f64 {
        if (self.leaves.start as f64..=self.leaves.end as f64).contains(&position) {
            position
        } else {
            self.leaves.start as f64
        }
    }

    pub fn cursor(&self, tree: &Tree<Key>, position: f64) -> Option<Cursor<Key>> {
        let position = self.position(position);
        if self.leaves.is_empty() {
            None
        } else if position == self.leaves.end as f64 {
            tree.cursor(position - 1.0).map(|cursor| Cursor {
                fraction: 1.0,
                ..cursor
            })
        } else {
            tree.cursor(position)
        }
    }

    pub fn slider(&self, position: f64) -> Option<Slider> {
        Slider::new(
            self.leaves.start as f64,
            self.leaves.end as f64,
            self.position(position),
        )
    }

    /// Boundary positions use the continuous slider's leaf units, not the
    /// equal-width notches. Keep levels intact for density decisions at placement.
    pub fn ticks(&self) -> Vec<TickLevel> {
        let coarsest = self.rows.first().map_or(0, |row| row.level);
        self.rows
            .iter()
            .rev()
            .map(|row| {
                let divisor = 1.5_f64.powf((coarsest - row.level) as f64);
                TickLevel {
                    values: row
                        .items
                        .iter()
                        .flat_map(|item| [item.leaves.start, item.leaves.end])
                        .filter(|boundary| (self.leaves.start..=self.leaves.end).contains(boundary))
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .map(|value| value as f64)
                        .collect(),
                    height: (24.0 / divisor).max(6.0),
                    width: (3.0 / divisor).max(1.0),
                }
            })
            .collect()
    }

    /// The visible item containing playback, including the final selected leaf
    /// when playback is exactly at the range's end.
    pub fn current_item(&self, level: usize, position: f64) -> Option<usize> {
        let leaf = (self.position(position).floor() as usize).min(self.leaves.clone().last()?);
        self.rows
            .iter()
            .find(|row| row.level == level)?
            .items
            .iter()
            .position(|item| item.leaves.contains(&leaf))
    }
}

#[cfg(test)]
mod tests;
