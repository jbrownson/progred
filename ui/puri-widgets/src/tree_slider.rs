//! Fine-end-aligned range sliders. The caller supplies ordered occurrence keys
//! and retains selection intent. Adjusting a row resets all finer rows.
use crate::{range_slider::RangeSlider, slider::Slider};
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
        while !frontier.is_empty() {
            let selected = match intent.get(level) {
                Some(Intent::Between { first, last }) => {
                    let start = frontier.partition_point(|(node, _)| node.key < *first);
                    let end = frontier.partition_point(|(node, _)| node.key <= *last);
                    if start < end {
                        start..end
                    } else {
                        0..frontier.len()
                    }
                }
                _ => 0..frontier.len(),
            };
            let nodes = &frontier[selected.clone()];
            if let (Some((_, first)), Some((last, offset))) = (nodes.first(), nodes.last()) {
                leaves = *first..offset + last.leaves;
            }
            let height = nodes.iter().map(|(node, _)| node.height).max().unwrap_or(0);
            rows.push(Row {
                level,
                slider: RangeSlider::new(frontier.len(), selected).unwrap(),
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

    pub fn select(&self, tree: &Tree<Key>, level: usize, range: Range<usize>) -> Self {
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
            Some(intent) => self.with_intent(tree, level, intent),
            None => Self::new(tree, &self.intent),
        }
    }

    pub fn select_all(&self, tree: &Tree<Key>, level: usize) -> Self {
        self.with_intent(tree, level, Intent::All)
    }

    fn with_intent(&self, tree: &Tree<Key>, level: usize, value: Intent<Key>) -> Self {
        let mut intent = self.intent.clone();
        intent.resize_with(intent.len().max(level + 1), || Intent::All);
        intent[..level].fill(Intent::All);
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
mod tests {
    use super::*;
    fn leaves(key: u8, keys: &[u8]) -> Tree<u8> {
        Tree::group(key, keys.iter().copied().map(Tree::leaf))
    }

    #[test]
    fn coarser_changes_reset_finer_selections_instead_of_reviving_them() {
        let tree = Tree::group(0, [leaves(10, &[11, 12]), leaves(20, &[21, 22])]);
        let all = Selection::new(&tree, &[]);
        let first = all.select(&tree, 1, 0..1);
        assert_eq!(first.leaves, 0..2);
        assert_eq!(first.intent[0], Intent::All);
        assert_eq!(first.select_all(&tree, 1).leaves, 0..4);

        let chosen = all.select(&tree, 0, 1..3);
        assert_eq!(chosen.leaves, 1..3);
        let first = chosen.select(&tree, 1, 0..1);
        assert_eq!(first.leaves, 0..2);
        assert_eq!(first.intent[0], Intent::All);
        assert_eq!(
            chosen.select_all(&tree, 1).intent,
            [Intent::All, Intent::All]
        );
        let second = first.select(&tree, 1, 1..2);
        assert_eq!(second.leaves, 2..4);
        assert_eq!(second.intent[0], Intent::All);
        assert_eq!(second.select(&tree, 1, 0..1).leaves, 0..2);
        assert_eq!(second.select_all(&tree, 1).leaves, 0..4);
    }

    #[test]
    fn intermediate_changes_clear_only_finer_levels_even_when_the_range_is_unchanged() {
        let tree = Tree::group(
            0,
            [
                Tree::group(10, [leaves(11, &[12, 13]), leaves(14, &[15, 16])]),
                Tree::group(20, [leaves(21, &[22, 23]), leaves(24, &[25, 26])]),
            ],
        );
        let selected = Selection::new(&tree, &[])
            .select(&tree, 2, 1..2)
            .select(&tree, 1, 0..1)
            .select(&tree, 0, 1..2);
        assert_eq!(selected.leaves, 5..6);
        for adjusted in [
            selected.select(&tree, 1, 0..1),
            selected.select_all(&tree, 1),
        ] {
            assert_eq!(adjusted.intent[0], Intent::All);
            assert_eq!(adjusted.intent[2], selected.intent[2]);
        }
        assert_eq!(selected.select(&tree, 1, 0..1).leaves, 4..6);
        assert_eq!(selected.select_all(&tree, 1).leaves, 4..8);
        assert_eq!(
            selected.select(&tree, 0, 0..1).intent[1..],
            selected.intent[1..]
        );
    }

    #[test]
    fn inserted_and_deleted_endpoints_use_ordered_bounds_not_ordinals() {
        let tree = leaves(0, &[10, 30, 50, 70]);
        let selected = Selection::new(&tree, &[]).select(&tree, 0, 1..3);
        let edited = leaves(0, &[5, 10, 30, 40, 50, 60, 70]);
        assert_eq!(Selection::new(&edited, &selected.intent).leaves, 2..5);
        let deleted_ends = leaves(0, &[5, 10, 40, 60, 70]);
        assert_eq!(Selection::new(&deleted_ends, &selected.intent).leaves, 2..3);
        let disjoint = leaves(0, &[80, 90]);
        let visible = Selection::new(&disjoint, &selected.intent);
        assert_eq!(visible.leaves, 0..2);
        assert_eq!(visible.intent, selected.intent);
        assert_eq!(Selection::new(&tree, &visible.intent).leaves, 1..3);
    }

    #[test]
    fn manual_full_span_is_bounded_but_all_includes_new_items() {
        let tree = leaves(0, &[10, 20]);
        let all = Selection::new(&tree, &[]);
        let explicit = all.select(&tree, 0, 0..2);
        let larger = leaves(0, &[5, 10, 15, 20, 30]);
        assert_eq!(Selection::new(&larger, &explicit.intent).leaves, 1..4);
        assert_eq!(Selection::new(&larger, &all.intent).leaves, 0..5);
        assert_eq!(
            Selection::new(&larger, &explicit.select_all(&tree, 0).intent).leaves,
            0..5
        );
    }

    #[test]
    fn coarser_changes_clear_hidden_finer_levels_and_keep_fine_end_alignment() {
        let tree = Tree::group(
            0,
            [
                leaves(10, &[11, 12]),
                Tree::group(20, [Tree::group(21, [leaves(22, &[23, 24])])]),
                leaves(30, &[]),
            ],
        );
        let selected = Selection::new(&tree, &[]).select(&tree, 1, 1..2);
        assert_eq!(
            selected
                .rows
                .iter()
                .map(|r| (r.level, r.slider.count))
                .collect::<Vec<_>>(),
            [(3, 2), (2, 2), (1, 2), (0, 2)]
        );
        let shallow = selected.select(&tree, 3, 0..1);
        assert_eq!(
            shallow.rows.iter().map(|r| r.level).collect::<Vec<_>>(),
            [3, 0]
        );
        assert_eq!(shallow.intent[..3], [Intent::All, Intent::All, Intent::All]);
        assert_eq!(shallow.select_all(&tree, 3).leaves, 0..4);
        assert!(Selection::new(&leaves(0, &[]), &[]).slider(0.0).is_none());
    }

    #[test]
    fn current_items_follow_playback_through_unequal_and_hidden_levels() {
        let tree = Tree::group(
            0,
            [
                leaves(10, &[11, 12]),
                Tree::group(20, [Tree::group(21, [leaves(22, &[23, 24, 25])])]),
                Tree::leaf(30),
            ],
        );
        let current = |selection: &Selection<u8>, position| {
            selection
                .rows
                .iter()
                .map(|row| {
                    (
                        row.level,
                        selection.current_item(row.level, position).unwrap(),
                    )
                })
                .collect::<Vec<_>>()
        };
        let all = Selection::new(&tree, &[]);
        assert_eq!(current(&all, 0.5), [(3, 0), (2, 0), (1, 0), (0, 0)]);
        assert_eq!(current(&all, 2.0), [(3, 1), (2, 1), (1, 1), (0, 2)]);
        assert_eq!(current(&all, 5.0), [(3, 2), (2, 2), (1, 2), (0, 5)]);
        assert_eq!(current(&all, 6.0), current(&all, 5.5));

        let deep = all.select(&tree, 3, 1..2);
        assert_eq!(current(&deep, 4.5), [(3, 1), (2, 0), (1, 0), (0, 2)]);
        assert_eq!(current(&deep, 5.0), current(&deep, 4.5));
        let shorter = deep.select(&tree, 0, 0..2);
        assert_eq!(current(&shorter, 4.0), [(3, 1), (2, 0), (1, 0), (0, 1)]);

        let shallow = all.select(&tree, 3, 0..1);
        assert_eq!(current(&shallow, 2.0), [(3, 0), (0, 1)]);
        assert_eq!(shallow.current_item(1, 0.5), None);
        assert_eq!(
            Selection::new(&leaves(0, &[]), &[]).current_item(0, 0.0),
            None
        );
    }

    #[test]
    fn cursor_tracks_its_leaf_and_fraction_across_insertions() {
        let tree = leaves(0, &[10, 20]);
        let cursor = tree.cursor(1.25).unwrap();
        assert_eq!(
            cursor,
            Cursor {
                item: 20,
                fraction: 0.25
            }
        );
        assert_eq!(leaves(0, &[5, 10, 20]).position(&cursor), Some(2.25));
        assert_eq!(
            tree.cursor(2.0),
            Some(Cursor {
                item: 20,
                fraction: 1.0
            })
        );
        assert_eq!(leaves(0, &[10]).position(&cursor), None);
        let selection = Selection::new(&tree, &[]).select(&tree, 0, 0..1);
        assert_eq!(selection.position(1.25), 0.0);
        assert_eq!(
            selection.cursor(&tree, 1.0),
            Some(Cursor {
                item: 10,
                fraction: 1.0
            })
        );
    }
}
