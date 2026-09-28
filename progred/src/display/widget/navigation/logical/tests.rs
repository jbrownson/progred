use super::super::Direction;
use super::*;
use gid::{CellId, Step};
use std::rc::Rc;

/// A chosen layout, traversed the way `ObserveLayout` reports it.
#[derive(Clone)]
enum Shape {
    Empty,
    Stop(usize),
    Row(RowAlignment, Vec<Shape>),
    Column(usize, Vec<Shape>),
    Enclosed(usize, Box<Shape>),
}
use Shape::*;

fn row(children: impl IntoIterator<Item = Shape>) -> Shape {
    Row(RowAlignment::Baseline, children.into_iter().collect())
}

fn column(baseline: usize, stops: impl IntoIterator<Item = usize>) -> Shape {
    Column(baseline, stops.into_iter().map(Stop).collect())
}

fn enclosed(stop: usize, child: Shape) -> Shape {
    Enclosed(stop, Box::new(child))
}

fn target(stop: usize) -> Target<()> {
    Target {
        path: Rc::from([Step::Key(CellId::from_u128(stop as u128))]),
        select: Rc::new(|_, _| true),
    }
}

fn index(target: &Target<()>) -> usize {
    match target.path.as_ref() {
        [Step::Key(cell)] => u128::from_be_bytes(*cell.as_bytes()) as usize,
        _ => unreachable!(),
    }
}

fn build(shape: &Shape, selected: usize, construction: &mut Construction<()>) {
    let layout = |composition, children: &[Shape], construction: &mut Construction<()>| {
        construction.begin(composition);
        for child in children {
            construction.begin(Composition::Row(RowAlignment::Baseline));
            build(child, selected, construction);
            construction.end();
        }
        construction.end();
    };
    match shape {
        Empty => {}
        Stop(stop) => construction.target(target(*stop), *stop == selected),
        Row(alignment, children) => layout(Composition::Row(*alignment), children, construction),
        Column(baseline, children) => layout(
            Composition::Column {
                baseline: *baseline,
            },
            children,
            construction,
        ),
        Enclosed(stop, child) => {
            construction.begin_container(target(*stop), *stop == selected);
            build(child, selected, construction);
            construction.end_container();
        }
    }
}

fn destinations(shape: &Shape, selected: usize) -> [Option<usize>; 4] {
    let mut construction = Construction::default();
    build(shape, selected, &mut construction);
    construction
        .current
        .lines
        .destinations()
        .unwrap()
        .map(|target| target.as_ref().map(index))
}

fn stops(shape: &Shape) -> Vec<usize> {
    match shape {
        Empty => vec![],
        Stop(stop) => vec![*stop],
        Row(_, children) | Column(_, children) => children.iter().flat_map(stops).collect(),
        Enclosed(stop, child) => [*stop].into_iter().chain(stops(child)).collect(),
    }
}

/// Navigation lines, read back from every stop's destinations: reading order
/// follows Right from the one stop with no Left, and the stops of one line
/// share their Up and Down.
fn lines(shape: &Shape) -> Vec<Vec<usize>> {
    let all: Vec<_> = stops(shape)
        .into_iter()
        .map(|stop| (stop, destinations(shape, stop)))
        .collect();
    let find = |stop: usize| all.iter().find(|(s, _)| *s == stop).unwrap().1;
    let mut next = all
        .iter()
        .find(|(_, [left, ..])| left.is_none())
        .map(|(s, _)| *s);
    let mut lines: Vec<(Option<usize>, Option<usize>, Vec<usize>)> = vec![];
    while let Some(stop) = next {
        let [_, right, up, down] = find(stop);
        match lines.last_mut() {
            Some((u, d, line)) if (*u, *d) == (up, down) => line.push(stop),
            _ => lines.push((up, down, vec![stop])),
        }
        next = right;
    }
    assert_eq!(
        lines.iter().map(|(.., line)| line.len()).sum::<usize>(),
        all.len()
    );
    lines.into_iter().map(|(.., line)| line).collect()
}

#[test]
fn long_single_line_keeps_only_boundaries_and_the_selected_neighbors() {
    let mut construction = Construction::default();
    for stop in 0..10_000 {
        construction.target(target(stop), stop == 5000);
    }
    let lines = construction.current.lines;
    assert_eq!(lines.rows.len(), 1);
    assert!(lines.rows[0].levels.is_empty());
    assert_eq!(
        lines.destinations().unwrap().map(|t| t.as_ref().map(index)),
        [Some(4999), Some(5001), None, None]
    );
}

#[test]
fn baseline_row_grouping_does_not_change_logical_order() {
    let children = vec![
        Stop(7),
        enclosed(0, column(1, [1, 2])),
        Empty,
        Stop(3),
        enclosed(8, column(0, [4, 5, 6])),
        Stop(9),
    ];
    let flat = lines(&row(children.clone()));
    for split in 0..=children.len() {
        let (left, right) = children.split_at(split);
        let grouped = row([row(left.to_vec()), row(right.to_vec())]);
        assert_eq!(lines(&grouped), flat);
    }
}

#[test]
fn rows_align_the_declared_column_baselines() {
    let shape = row([column(2, [0, 1, 2]), column(0, [3, 4])]);
    assert_eq!(lines(&shape), [vec![0], vec![1], vec![2, 3], vec![4]]);
    assert_eq!(
        destinations(&shape, 3),
        [Some(2), Some(4), Some(1), Some(4)]
    );
}

#[test]
fn column_baselines_follow_children_not_stop_counts_or_decorative_gaps() {
    let nested = Column(3, vec![Empty, column(0, [0, 1]), Empty, column(1, [2, 3])]);
    assert_eq!(
        lines(&row([nested, Stop(4)])),
        [vec![0], vec![1], vec![2], vec![3, 4]]
    );
    let gap = Column(1, vec![Stop(0), Empty, Stop(1)]);
    assert_eq!(lines(&row([gap, Stop(2)])), [vec![0], vec![1, 2]]);
}

#[test]
fn container_wrappers_do_not_change_content_alignment() {
    let shape = row([enclosed(0, column(1, [1, 2])), Stop(3)]);
    assert_eq!(lines(&shape), [vec![0], vec![1], vec![2, 3]]);
}

#[test]
fn a_label_and_multiline_container_are_peers_before_entering_contents() {
    let shape = enclosed(4, row([Stop(0), enclosed(1, column(0, [2, 3]))]));
    assert_eq!(lines(&shape), [vec![4], vec![0, 1], vec![2], vec![3]]);
    // Up from the label selects its enclosing call, not the neighboring list.
    assert_eq!(
        destinations(&shape, 0),
        [Some(4), Some(1), Some(4), Some(2)]
    );
    assert_eq!(
        destinations(&shape, 1),
        [Some(0), Some(2), Some(4), Some(2)]
    );
}

#[test]
fn content_after_a_block_stays_on_the_line_it_is_drawn_on() {
    let shape = row([enclosed(0, column(0, [1, 2])), Stop(3)]);
    assert_eq!(lines(&shape), [vec![0], vec![1, 3], vec![2]]);
    assert_eq!(
        destinations(&shape, 3),
        [Some(1), Some(2), Some(0), Some(2)]
    );

    let shape = row([enclosed(0, column(0, [1, 2])), enclosed(3, Stop(4))]);
    assert_eq!(lines(&shape), [vec![0], vec![1, 3, 4], vec![2]]);

    let shape = row([
        Stop(0),
        enclosed(1, column(0, [2, 3])),
        Stop(4),
        enclosed(5, column(0, [6, 7])),
    ]);
    assert_eq!(lines(&shape), [vec![0, 1, 5], vec![2, 4, 6], vec![3, 7]]);
}

#[test]
fn nested_wrappers_share_one_vertical_entrance_but_keep_each_horizontal_stop() {
    let shape = enclosed(0, enclosed(1, enclosed(2, column(0, [3, 4]))));
    assert_eq!(lines(&shape), [vec![0, 1, 2], vec![3], vec![4]]);
    for stop in 0..3 {
        let destinations = destinations(&shape, stop);
        assert_eq!(destinations[1], Some(stop + 1));
        assert_eq!(destinations[2], None);
        assert_eq!(destinations[3], Some(3));
    }
    assert_eq!(
        destinations(&shape, 3),
        [Some(2), Some(4), Some(0), Some(4)]
    );
}

#[test]
fn unequal_container_depths_do_not_displace_neighboring_content_lines() {
    let shape = row([
        enclosed(0, enclosed(1, column(0, [2, 3]))),
        enclosed(4, column(0, [5, 6])),
    ]);
    assert_eq!(lines(&shape), [vec![0, 1, 4], vec![2, 5], vec![3, 6]]);
}

#[test]
fn multiline_children_can_start_on_later_lines_of_a_parent_container() {
    let shape = enclosed(0, Column(0, vec![Stop(1), enclosed(2, column(0, [3, 4]))]));
    assert_eq!(lines(&shape), [vec![0], vec![1], vec![2], vec![3], vec![4]]);
}

#[test]
fn empty_containers_are_single_stops() {
    assert_eq!(lines(&row([Stop(0), enclosed(1, Empty)])), [vec![0, 1]]);
}

#[test]
fn top_aligned_rows_keep_top_alignment_and_export_the_selected_baseline() {
    let top = Row(
        RowAlignment::Top { baseline: 1 },
        vec![Empty, column(1, [0, 1]), Stop(2)],
    );
    assert_eq!(lines(&top), [vec![0, 2], vec![1]]);
    assert_eq!(lines(&row([top, Stop(3)])), [vec![0, 2], vec![1, 3]]);
}

#[test]
fn multiline_container_precedes_its_first_line_and_reading_order_wraps() {
    let shape = enclosed(0, Column(0, vec![row([Stop(1), Stop(2)]), Stop(3)]));
    assert_eq!(lines(&shape), [vec![0], vec![1, 2], vec![3]]);
    assert_eq!(destinations(&shape, 0), [None, Some(1), None, Some(1)]);
    assert_eq!(
        destinations(&shape, 1),
        [Some(0), Some(2), Some(0), Some(3)]
    );
    assert_eq!(
        destinations(&shape, 2),
        [Some(1), Some(3), Some(0), Some(3)]
    );
}

#[test]
fn single_line_containers_do_not_add_vertical_steps() {
    let shape = Column(0, vec![enclosed(0, enclosed(1, Stop(2))), Stop(3)]);
    assert_eq!(lines(&shape), [vec![0, 1, 2], vec![3]]);
    assert_eq!(destinations(&shape, 0), [None, Some(1), None, Some(3)]);
}

#[test]
fn a_row_with_multiline_content_is_multiline_but_empty_decorations_add_no_lines() {
    let content = Column(0, vec![Empty, Stop(1), Empty, Stop(2)]);
    let shape = enclosed(0, row([Empty, content]));
    assert_eq!(lines(&shape), [vec![0], vec![1], vec![2]]);
}

#[test]
fn a_repeated_declaration_refines_its_open_whole_value_instead_of_adding_a_stop() {
    let arrived = Rc::new(std::cell::Cell::new(false));
    let mut construction = Construction::default();
    construction.target(target(0), false);
    construction.begin_container(target(1), false);
    let refined = arrived.clone();
    construction.target(
        Target {
            select: Rc::new(move |_, _| {
                refined.set(true);
                true
            }),
            ..target(1)
        },
        false,
    );
    construction.target(target(2), true);
    construction.end_container();
    let [left, ..] = construction.current.lines.destinations().unwrap();
    let left = left.unwrap();
    assert_eq!(index(&left), 1);
    assert!((left.select)(&mut (), None));
    assert!(arrived.get());
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
