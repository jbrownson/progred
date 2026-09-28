//! Navigation from the chosen layout composition, not pixel geometry.
use super::{DIRECTIONS, DispatchContext, Event, EventOutcome, Handler, Target};
use crate::display::RowAlignment;
use gid::Step;
use std::{collections::HashMap, rc::Rc};

struct Stop<C> {
    target: Target<C>,
    selected: bool,
}

#[derive(Clone, Copy)]
pub(crate) enum Arrangement {
    Row(RowAlignment),
    Column { baseline: usize },
}

#[derive(Default)]
pub(crate) struct Lines {
    baseline: usize,
    content_lines: usize,
    rows: Vec<Vec<usize>>,
}

impl Lines {
    fn stop(stop: usize) -> Self {
        Self {
            baseline: 0,
            content_lines: 1,
            rows: vec![vec![stop]],
        }
    }

    fn combine(arrangement: Arrangement, children: &mut Vec<Self>, start: usize) -> Self {
        match arrangement {
            Arrangement::Row(alignment) => {
                let baseline = match alignment {
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
                children.drain(start..).fold(
                    Self {
                        baseline,
                        ..Self::default()
                    },
                    |mut lines, child| {
                        let offset = match alignment {
                            RowAlignment::Baseline => baseline - child.baseline,
                            RowAlignment::Top { .. } | RowAlignment::Center => 0,
                        };
                        lines.content_lines = lines.content_lines.max(child.content_lines);
                        if lines.rows.is_empty() && offset == 0 {
                            lines.rows = child.rows;
                            return lines;
                        }
                        lines
                            .rows
                            .resize_with(lines.rows.len().max(offset + child.rows.len()), Vec::new);
                        for (row, child) in lines.rows.iter_mut().skip(offset).zip(child.rows) {
                            if row.is_empty() {
                                *row = child;
                            } else {
                                row.extend(child);
                            }
                        }
                        lines
                    },
                )
            }
            Arrangement::Column { baseline } => children.drain(start..).enumerate().fold(
                Self::default(),
                |mut lines, (index, child)| {
                    if index == baseline {
                        lines.baseline = lines.rows.len() + child.baseline;
                    }
                    lines.content_lines += child.content_lines;
                    if lines.rows.is_empty() {
                        lines.rows = child.rows;
                    } else {
                        lines.rows.extend(child.rows);
                    }
                    lines
                },
            ),
        }
    }

    fn enclosed(mut self, stop: usize) -> Self {
        if self.content_lines > 1 {
            self.rows.insert(0, vec![stop]);
            self.baseline += 1;
        } else if let Some(first) = self.rows.first_mut() {
            first.insert(0, stop);
        } else {
            self = Self::stop(stop);
        }
        self
    }

    fn destinations(&self, selected: usize) -> [Option<usize>; 4] {
        let order: Vec<_> = self.rows.iter().flatten().copied().collect();
        let index = order.iter().position(|stop| *stop == selected);
        let row = self.rows.iter().position(|row| row.contains(&selected));
        [
            index
                .and_then(|i| i.checked_sub(1))
                .and_then(|i| order.get(i))
                .copied(),
            index.and_then(|i| order.get(i + 1)).copied(),
            row.and_then(|r| r.checked_sub(1))
                .and_then(|r| self.rows.get(r))
                .and_then(|r| r.first())
                .copied(),
            row.and_then(|r| self.rows.get(r + 1))
                .and_then(|r| r.first())
                .copied(),
        ]
    }
}

pub(crate) struct Construction<C> {
    stops: Vec<Stop<C>>,
    occurrences: HashMap<Rc<[Step]>, usize>,
    children: Vec<Lines>,
}

impl<C> Default for Construction<C> {
    fn default() -> Self {
        Self {
            stops: Vec::new(),
            occurrences: HashMap::new(),
            children: Vec::new(),
        }
    }
}

impl<C: 'static> Construction<C> {
    fn register(&mut self, target: Target<C>, selected: bool) -> Option<usize> {
        let stop = Stop { target, selected };
        match self.occurrences.get(&stop.target.path) {
            Some(&index) => {
                self.stops[index] = stop;
                None
            }
            None => {
                let index = self.stops.len();
                self.occurrences.insert(stop.target.path.clone(), index);
                self.stops.push(stop);
                Some(index)
            }
        }
    }

    pub fn target(&mut self, target: Target<C>, selected: bool) {
        if let Some(index) = self.register(target, selected) {
            self.children.push(Lines::stop(index));
        }
    }

    /// Child fragments occupy a suffix of one frame-local scratch buffer.
    pub fn begin(&self) -> usize {
        self.children.len()
    }

    pub fn end(&mut self, start: usize, arrangement: Arrangement) {
        let lines = Lines::combine(arrangement, &mut self.children, start);
        self.children.push(lines);
    }

    pub fn begin_container(&mut self, target: Target<C>, selected: bool) -> (Option<usize>, usize) {
        (self.register(target, selected), self.begin())
    }

    pub fn end_container(&mut self, (stop, start): (Option<usize>, usize)) {
        let lines = Lines::combine(
            Arrangement::Row(RowAlignment::Baseline),
            &mut self.children,
            start,
        );
        self.children.push(match stop {
            Some(stop) => lines.enclosed(stop),
            None => lines,
        });
    }

    pub fn finish<H: 'static>(mut self) -> Option<Handler<C, DispatchContext<C, H>>> {
        let selected = self.stops.iter().position(|stop| stop.selected)?;
        let destinations = Lines::combine(
            Arrangement::Row(RowAlignment::Baseline),
            &mut self.children,
            0,
        )
        .destinations(selected)
        .map(|index| index.map(|index| self.stops[index].target.clone()));
        let mut handler = Handler::new();
        handler.on(move |world, event, _| match event {
            Event::Navigate(direction) => {
                let target = DIRECTIONS
                    .iter()
                    .position(|d| *d == direction)
                    .and_then(|index| destinations[index].as_ref());
                EventOutcome::from_handled(
                    event,
                    target.is_some_and(|target| (target.select)(world, Some(direction))),
                )
            }
            _ => EventOutcome::decline(event),
        });
        Some(handler)
    }
}

#[cfg(test)]
mod tests {
    use super::super::Direction;
    use super::*;

    fn combine(arrangement: Arrangement, mut children: Vec<Lines>) -> Lines {
        Lines::combine(arrangement, &mut children, 0)
    }

    fn column(baseline: usize, stops: impl IntoIterator<Item = usize>) -> Lines {
        combine(
            Arrangement::Column { baseline },
            stops.into_iter().map(Lines::stop).collect(),
        )
    }

    #[test]
    fn baseline_row_grouping_does_not_change_logical_order() {
        let fragments = || {
            vec![
                column(1, [1, 2]).enclosed(0),
                Lines::default(),
                Lines::stop(3),
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
            assert_eq!(grouped.content_lines, flat.content_lines);
            assert_eq!(grouped.rows, flat.rows);
        }
    }

    #[test]
    fn combining_a_child_preserves_earlier_siblings_in_the_scratch_buffer() {
        let mut children = vec![Lines::stop(0), Lines::stop(1), Lines::stop(2)];
        let child = Lines::combine(Arrangement::Column { baseline: 1 }, &mut children, 1);
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].rows, [vec![0]]);
        children.push(child);
        let parent = Lines::combine(Arrangement::Row(RowAlignment::Baseline), &mut children, 0);
        assert!(children.is_empty());
        assert_eq!(parent.rows, [vec![1], vec![0, 2]]);
    }

    #[test]
    fn rows_align_the_declared_column_baselines() {
        let lines = combine(
            Arrangement::Row(RowAlignment::Baseline),
            vec![column(2, [0, 1, 2]), column(0, [3, 4])],
        );
        assert_eq!(lines.baseline, 2);
        assert_eq!(lines.rows, [vec![0], vec![1], vec![2, 3], vec![4]]);
        assert_eq!(lines.destinations(3), [Some(2), Some(4), Some(1), Some(4)]);
    }

    #[test]
    fn column_baselines_follow_children_not_stop_counts_or_decorative_gaps() {
        let nested = combine(
            Arrangement::Column { baseline: 3 },
            vec![
                Lines::default(),
                column(0, [0, 1]),
                Lines::default(),
                column(1, [2, 3]),
            ],
        );
        let lines = combine(
            Arrangement::Row(RowAlignment::Baseline),
            vec![nested, Lines::stop(4)],
        );
        assert_eq!(lines.baseline, 3);
        assert_eq!(lines.rows, [vec![0], vec![1], vec![2], vec![3, 4]]);

        let lines = combine(
            Arrangement::Row(RowAlignment::Baseline),
            vec![
                combine(
                    Arrangement::Column { baseline: 1 },
                    vec![Lines::stop(0), Lines::default(), Lines::stop(1)],
                ),
                Lines::stop(2),
            ],
        );
        assert_eq!(lines.rows, [vec![0], vec![1, 2]]);
    }

    #[test]
    fn leading_container_stops_preserve_the_contents_baseline() {
        let lines = combine(
            Arrangement::Row(RowAlignment::Baseline),
            vec![column(1, [1, 2]).enclosed(0), Lines::stop(3)],
        );
        assert_eq!(lines.baseline, 2);
        assert_eq!(lines.rows, [vec![0], vec![1], vec![2, 3]]);
        assert_eq!(lines.content_lines, 2);
    }

    #[test]
    fn top_aligned_rows_keep_top_alignment_and_export_the_selected_baseline() {
        let top = combine(
            Arrangement::Row(RowAlignment::Top { baseline: 1 }),
            vec![Lines::default(), column(1, [0, 1]), Lines::stop(2)],
        );
        assert_eq!(top.baseline, 1);
        assert_eq!(top.rows, [vec![0, 2], vec![1]]);
        let lines = combine(
            Arrangement::Row(RowAlignment::Baseline),
            vec![top, Lines::stop(3)],
        );
        assert_eq!(lines.rows, [vec![0, 2], vec![1, 3]]);
    }

    #[test]
    fn multiline_container_precedes_its_first_line_and_reading_order_wraps() {
        let lines = combine(
            Arrangement::Column { baseline: 0 },
            vec![
                combine(
                    Arrangement::Row(RowAlignment::Baseline),
                    vec![Lines::stop(1), Lines::stop(2)],
                ),
                Lines::stop(3),
            ],
        )
        .enclosed(0);
        assert_eq!(lines.rows, [vec![0], vec![1, 2], vec![3]]);
        assert_eq!(lines.destinations(0), [None, Some(1), None, Some(1)]);
        assert_eq!(lines.destinations(1), [Some(0), Some(2), Some(0), Some(3)]);
        assert_eq!(lines.destinations(2), [Some(1), Some(3), Some(0), Some(3)]);
    }

    #[test]
    fn single_line_containers_do_not_add_vertical_steps() {
        let lines = combine(
            Arrangement::Column { baseline: 0 },
            vec![Lines::stop(2).enclosed(1).enclosed(0), Lines::stop(3)],
        );
        assert_eq!(lines.rows, [vec![0, 1, 2], vec![3]]);
        assert_eq!(lines.destinations(0), [None, Some(1), None, Some(3)]);
    }

    #[test]
    fn a_row_with_multiline_content_is_multiline_but_empty_decorations_add_no_lines() {
        let content = combine(
            Arrangement::Column { baseline: 0 },
            vec![
                Lines::default(),
                Lines::stop(1),
                Lines::default(),
                Lines::stop(2),
            ],
        );
        let lines = combine(
            Arrangement::Row(RowAlignment::Baseline),
            vec![Lines::default(), content],
        )
        .enclosed(0);
        assert_eq!(lines.rows, [vec![0], vec![1], vec![2]]);
        assert_eq!(lines.content_lines, 2);
    }

    #[test]
    fn views_with_identical_paths_do_not_share_logical_stops() {
        use super::super::super::{HoverInput, HoverPass, view::Root};
        let paths: [Rc<[Step]>; 2] =
            std::array::from_fn(|_| Rc::from([Step::Key(gid::new_cell_id())]));
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
}
