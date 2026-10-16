use super::*;
use crate::Extent;
use kurbo::{Point, Rect};
use std::{cell::Cell, rc::Rc};
use uig::Placement;

#[derive(Debug, Default, PartialEq)]
struct Recording(Vec<(&'static str, Placement)>);

impl crate::ObserveLayout for Recording {}

impl crate::Output for Recording {
    fn empty() -> Self {
        Self::default()
    }

    fn over(mut self, above: Self) -> Self {
        self.0.extend(above.0);
        self
    }
}

fn recording(label: &'static str, width: f64) -> ChoiceLayout<Recording> {
    ChoiceLayout::fixed(crate::leaf_into(
        Extent {
            width,
            ascent: 3.0,
            descent: 1.0,
        },
        move |placement, out: &mut Recording| out.0.push((label, placement)),
    ))
}

#[test]
fn top_aligned_row_uses_the_selected_child_heights() {
    for (available, height, label) in [(200.0, 180.0, "flat"), (130.0, 240.0, "tall")] {
        let mut build = ChoiceBuild::default();
        let picture = ChoiceLayout::fixed(crate::leaf_into(
            Extent {
                width: 110.0,
                ascent: 90.0,
                descent: 90.0,
            },
            |placement, out: &mut Recording| out.0.push(("picture", placement)),
        ));
        let fields = build.alternatives(vec![
            recording("flat", 60.0),
            ChoiceLayout::fixed(crate::leaf_into(
                Extent {
                    width: 8.0,
                    ascent: 3.0,
                    descent: 237.0,
                },
                |placement, out: &mut Recording| out.0.push(("tall", placement)),
            )),
        ]);
        let root = ChoiceLayout::aligned_row(
            crate::RowAlignment::Top { baseline: 1 },
            12.0,
            vec![picture, fields],
        );
        let layout = resolve_choices(build.finish(root), available, false);
        assert_eq!(layout.extent.height(), height);
        assert_eq!(layout.extent.ascent, 3.0);
        let clip = Rect::new(30.0, 40.0, 200.0, 160.0);
        let placement = Placement::new(layout.extent.rect_at(Point::new(20.0, 30.0)), clip);
        let Recording(out) = crate::place(layout, placement);
        assert_eq!(out[0].0, "picture");
        assert_eq!(out[1].0, label);
        assert_eq!(out[0].1.rect, Rect::new(20.0, 30.0, 130.0, 210.0));
        assert_eq!(out[1].1.rect.origin(), Point::new(142.0, 30.0));
        for (_, child) in out {
            assert_eq!(child.clip_rect, clip);
            assert_eq!(child.available_rect.y0, 30.0);
            assert_eq!(child.available_rect.height(), height);
        }
    }
}

#[test]
fn only_the_selected_form_places_and_wrappers_keep_settled_geometry() {
    let placements = Rc::new(Cell::new(0));
    let child = ChoiceLayout::map(recording("chosen", 10.0), 0.0, {
        let placements = placements.clone();
        move |child| {
            crate::after_into(
                crate::before_into(child, |p, out| out.0.push(("before", p))),
                move |p, out| {
                    placements.set(placements.get() + 1);
                    out.0.push(("after", p));
                },
            )
        }
    });
    let mut build = ChoiceBuild::default();
    let root = build.alternatives(vec![
        recording("discarded", 100.0),
        ChoiceLayout::pad(Insets::new(5.0, 6.0, 7.0, 8.0), child),
    ]);
    let layout = resolve_choices(build.finish(root), 30.0, false);
    assert_eq!(placements.get(), 0);
    assert_eq!(layout.extent.width, 22.0);
    assert_eq!(layout.extent.height(), 18.0);

    let clip = Rect::new(26.0, 37.0, 100.0, 100.0);
    let placement = Placement::new(layout.extent.rect_at(Point::new(20.0, 30.0)), clip);
    let out = crate::place(layout, placement);
    let child = Placement::new(Rect::new(25.0, 36.0, 35.0, 40.0), clip);
    assert_eq!(placements.get(), 1);
    assert_eq!(
        out,
        Recording(vec![("before", child), ("chosen", child), ("after", child)])
    );
}

#[test]
fn shared_children_prepare_once_and_place_in_the_chosen_form() {
    for available in [30.0, 100.0] {
        let mut build = ChoiceBuild::default();
        let prepared = Cell::new(0);
        let wide = build.shared(42, |_| {
            prepared.set(prepared.get() + 1);
            recording("shared", 20.0)
        });
        let narrow = build.shared(42, |_| panic!("shared description prepared twice"));
        let root = build.alternatives(vec![
            ChoiceLayout::aligned_row(
                crate::RowAlignment::Baseline,
                5.0,
                vec![recording("prefix", 60.0), wide],
            ),
            ChoiceLayout::col(0, 2.0, vec![recording("prefix", 10.0), narrow]),
        ]);
        let layout = resolve_choices(build.finish(root), available, false);
        assert_eq!(prepared.get(), 1);
        let placement = Placement::new(
            layout.extent.rect_at(Point::ORIGIN),
            Rect::new(0.0, 0.0, 200.0, 200.0),
        );
        let Recording(out) = crate::place(layout, placement);
        assert_eq!(
            out.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
            ["prefix", "shared"]
        );
        assert_eq!(
            out[1].1.rect,
            if available == 30.0 {
                Rect::new(0.0, 6.0, 20.0, 10.0)
            } else {
                Rect::new(65.0, 0.0, 85.0, 4.0)
            }
        );
    }
}

#[test]
fn a_preferred_form_chosen_after_exploring_the_fallback_keeps_its_natural_children() {
    let mut build = ChoiceBuild::default();
    let shared = |build: &mut ChoiceBuild<Recording>| {
        build.shared(7, |build| {
            build.alternatives(vec![recording("wide", 100.0), recording("narrow", 60.0)])
        })
    };
    let inline = shared(&mut build);
    let padded = shared(&mut build);
    let root = build.alternatives(vec![
        ChoiceLayout::aligned_row(
            crate::RowAlignment::Baseline,
            0.0,
            vec![recording("prefix", 10.0), inline],
        ),
        ChoiceLayout::pad(Insets::new(50.0, 0.0, 0.0, 0.0), padded),
    ]);
    let layout = resolve_choices(build.finish(root), 100.0, false);
    assert_eq!(layout.extent.width, 110.0);
    let placement = Placement::new(
        layout.extent.rect_at(Point::ORIGIN),
        Rect::new(0.0, 0.0, 200.0, 200.0),
    );
    let Recording(out) = crate::place(layout, placement);
    assert_eq!(
        out.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
        ["prefix", "wide"]
    );
}

#[test]
fn attached_content_is_settled_but_its_placement_remains_the_consumers_choice() {
    for show in [false, true] {
        let mut build = ChoiceBuild::default();
        let attached =
            build.alternatives(vec![recording("wide", 100.0), recording("narrow", 20.0)]);
        let root = ChoiceLayout::attach(recording("base", 10.0), attached, move |base, layer| {
            crate::overlay(base, layer, move |p, extent, _| {
                show.then(|| {
                    Placement::new(
                        extent.rect_at(Point::new(p.rect.x0, p.rect.y1)),
                        p.clip_rect,
                    )
                })
            })
        });
        let layout = resolve_choices(build.finish(root), 30.0, false);
        assert_eq!(layout.extent.width, 10.0);
        assert_eq!(layout.extent.height(), 4.0);
        let placement = Placement::new(
            layout.extent.rect_at(Point::ORIGIN),
            Rect::new(0.0, 0.0, 200.0, 200.0),
        );
        let Recording(out) = crate::place(layout, placement);
        assert_eq!(
            out.iter().map(|(name, _)| *name).collect::<Vec<_>>(),
            if show {
                vec!["base", "narrow"]
            } else {
                vec!["base"]
            }
        );
    }
}

#[derive(Clone, Copy)]
struct Output;

impl crate::ObserveLayout for Output {}

impl crate::Output for Output {
    fn empty() -> Self {
        Self
    }

    fn over(self, _: Self) -> Self {
        self
    }
}

fn fixed(width: f64) -> ChoiceLayout<Output> {
    ChoiceLayout::fixed(crate::leaf(
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
        crate::RowAlignment::Baseline,
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
        crate::RowAlignment::Baseline,
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
