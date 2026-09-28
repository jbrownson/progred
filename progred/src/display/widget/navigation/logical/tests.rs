//! Full-tree reference model used only as a regression oracle.
use super::super::Direction;
use super::*;
use crate::display::RowAlignment;

#[derive(Clone, Copy)]
pub(crate) enum Arrangement {
    Row(RowAlignment),
    Column { baseline: usize },
}

#[derive(Clone, Default)]
struct Fragment {
    baseline: usize,
    line_count: usize,
    contents: Contents,
}

#[derive(Clone, Default)]
enum Contents {
    #[default]
    Empty,
    Stop(usize),
    /// Child offsets count content lines only, never container entry steps.
    Arranged(Vec<(usize, Fragment)>),
    Enclosed {
        stop: usize,
        child: Box<Fragment>,
    },
}

#[derive(Clone, Copy)]
struct Parent {
    stop: usize,
    enters_content: bool,
}

struct PlacedStop {
    stop: usize,
    parent: Option<Parent>,
}

impl Fragment {
    fn stop(stop: usize) -> Self {
        Self {
            baseline: 0,
            line_count: 1,
            contents: Contents::Stop(stop),
        }
    }

    fn combine(arrangement: Arrangement, children: &mut Vec<Self>, start: usize) -> Self {
        if children.len() == start {
            return Self::default();
        }
        if children.len() == start + 1 {
            let mut child = children.pop().unwrap();
            if !matches!(
                arrangement,
                Arrangement::Row(RowAlignment::Baseline | RowAlignment::Top { baseline: 0 })
                    | Arrangement::Column { baseline: 0 }
            ) {
                child.baseline = 0;
            }
            return child;
        }
        let mut baseline = 0;
        let mut line_count = 0;
        let mut arranged = Vec::with_capacity(children.len() - start);
        match arrangement {
            Arrangement::Row(alignment) => {
                baseline = match alignment {
                    RowAlignment::Baseline => children[start..]
                        .iter()
                        .map(|child| child.baseline)
                        .max()
                        .unwrap_or(0),
                    RowAlignment::Top { baseline } => children
                        .get(start + baseline)
                        .map_or(0, |child| child.baseline),
                    RowAlignment::Center => 0,
                };
                for child in children.drain(start..) {
                    if child.line_count == 0 {
                        continue;
                    }
                    let offset = match alignment {
                        RowAlignment::Baseline => baseline - child.baseline,
                        RowAlignment::Top { .. } | RowAlignment::Center => 0,
                    };
                    line_count = line_count.max(offset + child.line_count);
                    arranged.push((offset, child));
                }
            }
            Arrangement::Column { baseline: anchor } => {
                for (index, child) in children.drain(start..).enumerate() {
                    if index == anchor {
                        baseline = line_count + child.baseline;
                    }
                    if child.line_count > 0 {
                        let offset = line_count;
                        line_count += child.line_count;
                        arranged.push((offset, child));
                    }
                }
            }
        }
        // Ordinary one-child layout decorators need no retained node.
        if arranged.len() == 1 && arranged[0].0 == 0 && arranged[0].1.baseline == baseline {
            return arranged.pop().unwrap().1;
        }
        Self {
            baseline,
            line_count,
            contents: Contents::Arranged(arranged),
        }
    }

    fn enclosed(self, stop: usize) -> Self {
        if self.line_count == 0 {
            return Self::stop(stop);
        }
        Self {
            baseline: self.baseline,
            line_count: self.line_count,
            contents: Contents::Enclosed {
                stop,
                child: Box::new(self),
            },
        }
    }

    fn place(&self, row: usize, parent: Option<Parent>, rows: &mut [Vec<PlacedStop>]) {
        match &self.contents {
            Contents::Empty => {}
            Contents::Stop(stop) => rows[row].push(PlacedStop {
                stop: *stop,
                parent,
            }),
            Contents::Arranged(children) => {
                for (offset, child) in children {
                    child.place(row + offset, parent, rows);
                }
            }
            Contents::Enclosed { stop, child } => {
                rows[row].push(PlacedStop {
                    stop: *stop,
                    parent,
                });
                child.place(
                    row,
                    Some(Parent {
                        stop: *stop,
                        enters_content: self.line_count > 1 && child.first_level_has_content(),
                    }),
                    rows,
                );
            }
        }
    }

    // The reference keeps the complete tree and can inspect the leading layer
    // directly. Production instead carries this one fact in each line summary.
    fn first_level_has_content(&self) -> bool {
        match &self.contents {
            Contents::Empty => false,
            Contents::Stop(_) => true,
            Contents::Enclosed { .. } => self.line_count == 1,
            Contents::Arranged(children) => children
                .iter()
                .any(|(offset, child)| *offset == 0 && child.first_level_has_content()),
        }
    }

    /// Align the chosen layout first, then visit enclosing selections before
    /// their contents. Wrappers never add to a sibling's alignment offset.
    fn navigation_rows(&self) -> Vec<Vec<usize>> {
        let mut content_rows: Vec<_> = (0..self.line_count).map(|_| Vec::new()).collect();
        self.place(0, None, &mut content_rows);
        let mut locations: Vec<Option<(usize, usize)>> = Vec::new();
        let mut result = Vec::new();
        for (row, stops) in content_rows.into_iter().enumerate() {
            let mut levels: Vec<Vec<usize>> = Vec::new();
            for PlacedStop { stop, parent } in stops {
                // Only an enclosing stop on this content line introduces an
                // entry step here. Later lines remain aligned with their peers.
                let depth = parent
                    .and_then(|parent| {
                        locations
                            .get(parent.stop)
                            .copied()
                            .flatten()
                            .filter(|(parent_row, _)| *parent_row == row)
                            .map(|(_, depth)| depth + usize::from(parent.enters_content))
                    })
                    .unwrap_or(0);
                locations.resize(locations.len().max(stop + 1), None);
                locations[stop] = Some((row, depth));
                levels.resize_with(levels.len().max(depth + 1), Vec::new);
                levels[depth].push(stop);
            }
            result.extend(levels);
        }
        result
    }

    fn destinations(&self, selected: usize) -> [Option<usize>; 4] {
        let rows = self.navigation_rows();
        let order: Vec<_> = rows.iter().flatten().copied().collect();
        let index = order.iter().position(|stop| *stop == selected);
        let row = rows.iter().position(|row| row.contains(&selected));
        let expected = [
            index
                .and_then(|i| i.checked_sub(1))
                .and_then(|i| order.get(i))
                .copied(),
            index.and_then(|i| order.get(i + 1)).copied(),
            row.and_then(|r| r.checked_sub(1))
                .and_then(|r| rows.get(r))
                .and_then(|r| r.first())
                .copied(),
            row.and_then(|r| rows.get(r + 1))
                .and_then(|r| r.first())
                .copied(),
        ];
        assert_eq!(self.summarize(selected).destinations().unwrap(), expected);
        expected
    }

    fn summarize(&self, selected: usize) -> Lines {
        let mut lines = match &self.contents {
            Contents::Empty => Lines::default(),
            Contents::Stop(stop) => Lines::stop(*stop, *stop == selected),
            Contents::Arranged(children) => {
                let mut lines = Lines::default();
                for (offset, child) in children {
                    lines.merge_at(child.summarize(selected), *offset);
                }
                lines.baseline = self.baseline;
                lines
            }
            Contents::Enclosed { stop, child } => {
                child.summarize(selected).enclosed(*stop, *stop == selected)
            }
        };
        lines.baseline = self.baseline;
        lines
    }
}

fn combine(arrangement: Arrangement, mut children: Vec<Fragment>) -> Fragment {
    let expected = Fragment::combine(arrangement, &mut children.clone(), 0);
    for selected in expected.navigation_rows().into_iter().flatten() {
        let composition = match arrangement {
            Arrangement::Row(alignment) => Composition::Row(alignment),
            Arrangement::Column { baseline } => Composition::Column { baseline },
        };
        let mut accumulator = Accumulator::new(composition);
        for child in &children {
            accumulator.push(child.summarize(selected));
        }
        assert_eq!(accumulator.lines.baseline, expected.baseline);
        assert_eq!(accumulator.lines.rows.len(), expected.line_count);
        assert_eq!(
            accumulator.lines.destinations().unwrap(),
            expected.destinations(selected)
        );
    }
    Fragment::combine(arrangement, &mut children, 0)
}

#[test]
fn long_single_line_keeps_only_boundaries_and_the_selected_neighbors() {
    let mut accumulator = Accumulator::default();
    for stop in 0..10_000 {
        accumulator.push(Lines::stop(stop, stop == 5000));
    }
    assert_eq!(accumulator.lines.rows.len(), 1);
    assert_eq!(accumulator.lines.rows[0].len(), 1);
    assert_eq!(
        accumulator.lines.destinations(),
        Some([Some(4999), Some(5001), None, None])
    );
}

#[test]
fn streaming_matches_the_reference_in_mixed_nested_layouts() {
    // Deterministic generated cases include decoration-only children, non-first
    // baselines, different enclosure depths, and selection in either column.
    fn next(seed: &mut u64) -> usize {
        *seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        (*seed >> 32) as usize
    }
    fn tree(seed: &mut u64, depth: usize, id: &mut usize) -> Fragment {
        if depth == 0 {
            if next(seed) % 5 == 0 {
                return Fragment::default();
            }
            let stop = *id;
            *id += 1;
            return Fragment::stop(stop);
        }
        let count = next(seed) % 4 + 1;
        let baseline = next(seed) % count;
        let arrangement = match next(seed) % 4 {
            0 => Arrangement::Row(RowAlignment::Baseline),
            1 => Arrangement::Row(RowAlignment::Top { baseline }),
            2 => Arrangement::Row(RowAlignment::Center),
            _ => Arrangement::Column { baseline },
        };
        let children = (0..count).map(|_| tree(seed, depth - 1, id)).collect();
        let mut value = combine(arrangement, children);
        if next(seed) % 2 == 0 {
            value = value.enclosed(*id);
            *id += 1;
        }
        value
    }
    let mut seed = 718;
    for _ in 0..200 {
        let value = tree(&mut seed, 4, &mut 0);
        for stop in value.navigation_rows().into_iter().flatten() {
            value.destinations(stop);
        }
    }
}

fn column(baseline: usize, stops: impl IntoIterator<Item = usize>) -> Fragment {
    combine(
        Arrangement::Column { baseline },
        stops.into_iter().map(Fragment::stop).collect(),
    )
}

#[test]
fn baseline_row_grouping_does_not_change_logical_order() {
    let fragments = || {
        vec![
            column(1, [1, 2]).enclosed(0),
            Fragment::default(),
            Fragment::stop(3),
            column(0, [4, 5, 6]),
        ]
    };
    let row = Arrangement::Row(RowAlignment::Baseline);
    let flat = combine(row, fragments());
    for split in 0..=4 {
        let mut children = fragments();
        let right = children.split_off(split);
        let grouped = combine(row, vec![combine(row, children), combine(row, right)]);
        assert_eq!(grouped.baseline, flat.baseline);
        assert_eq!(grouped.line_count, flat.line_count);
        assert_eq!(grouped.navigation_rows(), flat.navigation_rows());
    }
}

#[test]
fn combining_a_child_preserves_earlier_siblings_in_the_scratch_buffer() {
    let mut children = vec![Fragment::stop(0), Fragment::stop(1), Fragment::stop(2)];
    let child = Fragment::combine(Arrangement::Column { baseline: 1 }, &mut children, 1);
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].navigation_rows(), [vec![0]]);
    children.push(child);
    let parent = Fragment::combine(Arrangement::Row(RowAlignment::Baseline), &mut children, 0);
    assert!(children.is_empty());
    assert_eq!(parent.navigation_rows(), [vec![1], vec![0, 2]]);
}

#[test]
fn rows_align_the_declared_column_baselines() {
    let lines = combine(
        Arrangement::Row(RowAlignment::Baseline),
        vec![column(2, [0, 1, 2]), column(0, [3, 4])],
    );
    assert_eq!(lines.baseline, 2);
    assert_eq!(
        lines.navigation_rows(),
        [vec![0], vec![1], vec![2, 3], vec![4]]
    );
    assert_eq!(lines.destinations(3), [Some(2), Some(4), Some(1), Some(4)]);
}

#[test]
fn column_baselines_follow_children_not_stop_counts_or_decorative_gaps() {
    let nested = combine(
        Arrangement::Column { baseline: 3 },
        vec![
            Fragment::default(),
            column(0, [0, 1]),
            Fragment::default(),
            column(1, [2, 3]),
        ],
    );
    let lines = combine(
        Arrangement::Row(RowAlignment::Baseline),
        vec![nested, Fragment::stop(4)],
    );
    assert_eq!(lines.baseline, 3);
    assert_eq!(
        lines.navigation_rows(),
        [vec![0], vec![1], vec![2], vec![3, 4]]
    );

    let lines = combine(
        Arrangement::Row(RowAlignment::Baseline),
        vec![
            combine(
                Arrangement::Column { baseline: 1 },
                vec![Fragment::stop(0), Fragment::default(), Fragment::stop(1)],
            ),
            Fragment::stop(2),
        ],
    );
    assert_eq!(lines.navigation_rows(), [vec![0], vec![1, 2]]);
}

#[test]
fn container_wrappers_do_not_change_content_alignment() {
    let lines = combine(
        Arrangement::Row(RowAlignment::Baseline),
        vec![column(1, [1, 2]).enclosed(0), Fragment::stop(3)],
    );
    assert_eq!(lines.baseline, 1);
    assert_eq!(lines.navigation_rows(), [vec![0], vec![1], vec![2, 3]]);
    assert_eq!(lines.line_count, 2);
}

#[test]
fn a_label_and_multiline_container_are_peers_before_entering_contents() {
    let lines = combine(
        Arrangement::Row(RowAlignment::Baseline),
        vec![Fragment::stop(0), column(0, [2, 3]).enclosed(1)],
    )
    .enclosed(4);
    assert_eq!(lines.baseline, 0);
    assert_eq!(lines.line_count, 2);
    assert_eq!(
        lines.navigation_rows(),
        [vec![4], vec![0, 1], vec![2], vec![3]]
    );
    // Up from the label selects its enclosing call, not the neighboring list.
    assert_eq!(lines.destinations(0), [Some(4), Some(1), Some(4), Some(2)]);
    assert_eq!(lines.destinations(1), [Some(0), Some(2), Some(4), Some(2)]);
}

#[test]
fn nested_wrappers_share_one_vertical_entrance_but_keep_each_horizontal_stop() {
    let lines = column(0, [3, 4]).enclosed(2).enclosed(1).enclosed(0);
    assert_eq!(lines.navigation_rows(), [vec![0, 1, 2], vec![3], vec![4]]);
    for stop in 0..3 {
        let destinations = lines.destinations(stop);
        assert_eq!(destinations[1], Some(stop + 1));
        assert_eq!(destinations[2], None);
        assert_eq!(destinations[3], Some(3));
    }
    assert_eq!(lines.destinations(3), [Some(2), Some(4), Some(0), Some(4)]);
}

#[test]
fn unequal_container_depths_do_not_displace_neighboring_content_lines() {
    let lines = combine(
        Arrangement::Row(RowAlignment::Baseline),
        vec![
            column(0, [2, 3]).enclosed(1).enclosed(0),
            column(0, [5, 6]).enclosed(4),
        ],
    );
    assert_eq!(lines.line_count, 2);
    assert_eq!(
        lines.navigation_rows(),
        [vec![0, 1, 4], vec![2, 5], vec![3, 6]]
    );
    assert_eq!(lines.destinations(3)[1], Some(6));
    assert_eq!(lines.destinations(6)[0], Some(3));
}

#[test]
fn multiline_children_can_start_on_later_lines_of_a_parent_container() {
    let lines = combine(
        Arrangement::Column { baseline: 0 },
        vec![Fragment::stop(1), column(0, [3, 4]).enclosed(2)],
    )
    .enclosed(0);
    assert_eq!(
        lines.navigation_rows(),
        [vec![0], vec![1], vec![2], vec![3], vec![4]]
    );
}

#[test]
fn wrappers_preserve_directional_reading_order_when_the_label_is_on_the_right() {
    let lines = combine(
        Arrangement::Row(RowAlignment::Baseline),
        vec![column(0, [1, 2]).enclosed(0), Fragment::stop(3)],
    );
    assert_eq!(lines.navigation_rows(), [vec![0, 3], vec![1], vec![2]]);
    assert_eq!(lines.destinations(3), [Some(0), Some(1), None, Some(1)]);
}

#[test]
fn empty_containers_are_single_stops() {
    let lines = combine(
        Arrangement::Row(RowAlignment::Baseline),
        vec![Fragment::stop(0), Fragment::default().enclosed(1)],
    );
    assert_eq!(lines.navigation_rows(), [vec![0, 1]]);
}

#[test]
fn top_aligned_rows_keep_top_alignment_and_export_the_selected_baseline() {
    let top = combine(
        Arrangement::Row(RowAlignment::Top { baseline: 1 }),
        vec![Fragment::default(), column(1, [0, 1]), Fragment::stop(2)],
    );
    assert_eq!(top.baseline, 1);
    assert_eq!(top.navigation_rows(), [vec![0, 2], vec![1]]);
    let lines = combine(
        Arrangement::Row(RowAlignment::Baseline),
        vec![top, Fragment::stop(3)],
    );
    assert_eq!(lines.navigation_rows(), [vec![0, 2], vec![1, 3]]);
}

#[test]
fn multiline_container_precedes_its_first_line_and_reading_order_wraps() {
    let lines = combine(
        Arrangement::Column { baseline: 0 },
        vec![
            combine(
                Arrangement::Row(RowAlignment::Baseline),
                vec![Fragment::stop(1), Fragment::stop(2)],
            ),
            Fragment::stop(3),
        ],
    )
    .enclosed(0);
    assert_eq!(lines.navigation_rows(), [vec![0], vec![1, 2], vec![3]]);
    assert_eq!(lines.destinations(0), [None, Some(1), None, Some(1)]);
    assert_eq!(lines.destinations(1), [Some(0), Some(2), Some(0), Some(3)]);
    assert_eq!(lines.destinations(2), [Some(1), Some(3), Some(0), Some(3)]);
}

#[test]
fn single_line_containers_do_not_add_vertical_steps() {
    let lines = combine(
        Arrangement::Column { baseline: 0 },
        vec![Fragment::stop(2).enclosed(1).enclosed(0), Fragment::stop(3)],
    );
    assert_eq!(lines.navigation_rows(), [vec![0, 1, 2], vec![3]]);
    assert_eq!(lines.destinations(0), [None, Some(1), None, Some(3)]);
}

#[test]
fn a_row_with_multiline_content_is_multiline_but_empty_decorations_add_no_lines() {
    let content = combine(
        Arrangement::Column { baseline: 0 },
        vec![
            Fragment::default(),
            Fragment::stop(1),
            Fragment::default(),
            Fragment::stop(2),
        ],
    );
    let lines = combine(
        Arrangement::Row(RowAlignment::Baseline),
        vec![Fragment::default(), content],
    )
    .enclosed(0);
    assert_eq!(lines.navigation_rows(), [vec![0], vec![1], vec![2]]);
    assert_eq!(lines.line_count, 2);
}

#[test]
fn views_with_identical_paths_do_not_share_logical_stops() {
    use super::super::super::{HoverInput, HoverPass, view::Root};
    let paths: [Rc<[Step]>; 2] = std::array::from_fn(|_| Rc::from([Step::Key(gid::new_cell_id())]));
    let mut pass = HoverPass::<Vec<usize>, ()>::new(&HoverInput {
        ..Default::default()
    });
    for view in 0..2 {
        pass.in_view(Root::document(), |pass| {
            for (index, path) in paths.iter().enumerate() {
                pass.visit(|output| {
                    output.navigation_target(
                        Target {
                            path: path.clone(),
                            select: Rc::new(move |visits, _| {
                                visits.push(view * 10 + index);
                                true
                            }),
                        },
                        view == 0 && index == 0,
                    )
                });
            }
        });
    }
    let mut visits = vec![];
    assert!(
        pass.finish()
            .bind(Default::default())
            .handler
            .unwrap()
            .dispatch(
                &mut visits,
                Event::Navigate(Direction::Right),
                &mut Default::default()
            )
            .handled()
    );
    assert_eq!(visits, [1]);
}
