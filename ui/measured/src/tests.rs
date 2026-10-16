use super::*;

impl<T> ObserveLayout for Vec<T> {}

impl Output for Vec<Placement> {
    fn empty() -> Self {
        Vec::new()
    }

    fn over(mut self, above: Self) -> Self {
        self.extend(above);
        self
    }
}

fn probe(extent: Extent) -> Measured<Vec<Placement>> {
    leaf(extent, move |placement| vec![placement])
}

fn direct_probe(extent: Extent) -> Measured<Vec<Placement>> {
    leaf_into(extent, move |placement, out: &mut Vec<Placement>| {
        out.push(placement)
    })
}

fn ext(width: f64, ascent: f64, descent: f64) -> Extent {
    Extent {
        width,
        ascent,
        descent,
    }
}

#[test]
fn children_inherit_the_enclosing_clip_until_a_container_narrows_it() {
    let root = Placement::new(
        Rect::new(0.0, 0.0, 100.0, 100.0),
        Rect::new(40.0, 20.0, 120.0, 80.0),
    );
    let child = child_placement(root, Rect::new(20.0, 50.0, 60.0, 90.0));
    assert_eq!(child.clip_rect, root.clip_rect);
    assert_eq!(child.visible_rect(), Rect::new(40.0, 50.0, 60.0, 80.0));
    let clipped = clipped_placement(child, Rect::new(50.0, 0.0, 80.0, 100.0));
    assert_eq!(clipped.clip_rect, Rect::new(50.0, 20.0, 80.0, 80.0));
}

#[test]
fn row_places_children_on_one_baseline() {
    let r = row(
        4.0,
        vec![probe(ext(10.0, 8.0, 2.0)), probe(ext(20.0, 12.0, 4.0))],
    );
    assert_eq!(r.extent, ext(34.0, 12.0, 4.0));

    let placed = place(r, Placement::root(Rect::new(0.0, 88.0, 34.0, 104.0)));
    assert_eq!(
        placed.iter().map(|p| p.rect).collect::<Vec<_>>(),
        vec![
            Rect::new(0.0, 92.0, 10.0, 102.0),
            Rect::new(14.0, 88.0, 34.0, 104.0),
        ]
    );
}

#[test]
fn row_offers_its_height_without_stretching_children_implicitly() {
    let layout = row(
        2.0,
        vec![
            probe(ext(5.0, 3.0, 2.0)),
            fill_height(probe(ext(5.0, 3.0, 2.0))),
            probe(ext(20.0, 30.0, 10.0)),
        ],
    );
    let placement = Placement::new(
        layout.extent.rect_at(Point::new(10.0, 20.0)),
        Rect::new(0.0, 35.0, 100.0, 50.0),
    );
    let output = place(layout, placement);
    assert_eq!(output[0].rect, Rect::new(10.0, 47.0, 15.0, 52.0));
    assert_eq!(output[0].available_rect, Rect::new(10.0, 20.0, 15.0, 60.0));
    assert_eq!(output[1].rect, Rect::new(17.0, 20.0, 22.0, 60.0));
    assert_eq!(output[1].clip_rect, placement.clip_rect);
}

#[test]
fn a_column_offers_its_width_and_fill_width_takes_it() {
    let layout = col(
        0,
        0.0,
        vec![
            probe(ext(5.0, 3.0, 2.0)),
            fill_width(probe(ext(0.0, 1.0, 0.0))),
            probe(ext(20.0, 3.0, 2.0)),
        ],
    );
    assert_eq!(layout.extent.width, 20.0);
    let output = place(layout, Placement::root(Rect::new(10.0, 0.0, 30.0, 11.0)));
    assert_eq!(output[0].rect.width(), 5.0);
    assert_eq!(output[1].rect, Rect::new(10.0, 5.0, 30.0, 6.0));
}

#[test]
fn nested_rows_offer_their_own_span_not_an_ancestors() {
    let nested = row(0.0, vec![fill_height(probe(ext(5.0, 3.0, 2.0)))]);
    let layout = row(0.0, vec![nested, probe(ext(20.0, 30.0, 10.0))]);
    let placement = Placement::root(layout.extent.rect_at(Point::ZERO));
    let output = place(layout, placement);
    assert_eq!(output[0].rect.height(), 5.0);
    assert_eq!(output[0].available_rect.height(), 5.0);
}

#[test]
fn columns_and_padding_offer_space_without_changing_the_clip() {
    let layout = col(
        0,
        2.0,
        vec![
            pad(Insets::new(1.0, 2.0, 3.0, 4.0), probe(ext(5.0, 3.0, 2.0))),
            probe(ext(40.0, 8.0, 2.0)),
        ],
    );
    let placement = Placement::new(
        layout.extent.rect_at(Point::ZERO),
        Rect::new(0.0, 0.0, 10.0, 100.0),
    );
    let output = place(layout, placement);
    assert_eq!(output[0].rect, Rect::new(1.0, 2.0, 6.0, 7.0));
    assert_eq!(output[0].available_rect, Rect::new(1.0, 2.0, 37.0, 7.0));
    assert_eq!(output[0].clip_rect, placement.clip_rect);
}

#[test]
fn top_aligned_row_exposes_the_chosen_baseline_to_its_parent() {
    for fields_height in [10.0, 240.0] {
        let tool = row_aligned(
            12.0,
            vec![
                probe(ext(110.0, 90.0, 90.0)),
                probe(ext(200.0, 8.0, fields_height - 8.0)),
            ],
            RowAlignment::Top { baseline: 1 },
        );
        let layout = row(6.0, vec![probe(ext(10.0, 8.0, 2.0)), tool]);
        let output = place_top_left(layout, Point::new(20.0, 30.0));
        let [arrow, picture, fields] = output.as_slice() else {
            panic!("three placed children");
        };
        assert_eq!(arrow.rect.y0, fields.rect.y0);
        assert_eq!(picture.rect.y0, fields.rect.y0);
        assert_eq!(picture.rect.height(), 180.0);
        assert_eq!(fields.rect.height(), fields_height);
    }
}

#[test]
fn centered_row_centers_short_children_in_the_tallest_line_box() {
    let r = centered_row(
        4.0,
        vec![probe(ext(10.0, 8.0, 2.0)), probe(ext(20.0, 12.0, 8.0))],
    );
    assert_eq!(r.extent, ext(34.0, 12.0, 8.0));

    let placed = place(r, Placement::root(Rect::new(0.0, 88.0, 34.0, 108.0)));
    assert_eq!(
        placed.iter().map(|p| p.rect).collect::<Vec<_>>(),
        vec![
            Rect::new(0.0, 93.0, 10.0, 103.0),
            Rect::new(14.0, 88.0, 34.0, 108.0),
        ]
    );
}

#[test]
fn col_takes_the_chosen_childs_baseline() {
    let c = col(
        1,
        2.0,
        vec![
            probe(ext(10.0, 8.0, 2.0)),
            probe(ext(4.0, 4.0, 0.0)),
            probe(ext(10.0, 8.0, 2.0)),
        ],
    );
    assert_eq!(c.extent, ext(10.0, 16.0, 12.0));

    let placed = place(c, Placement::root(Rect::new(0.0, 84.0, 10.0, 112.0)));
    assert_eq!(
        placed.iter().map(|p| p.rect).collect::<Vec<_>>(),
        vec![
            Rect::new(0.0, 84.0, 10.0, 94.0),
            Rect::new(0.0, 96.0, 4.0, 100.0),
            Rect::new(0.0, 102.0, 10.0, 112.0),
        ]
    );
}

#[test]
fn layers_share_an_origin_and_baseline_in_paint_order() {
    let stacked = layers(vec![
        probe(ext(10.0, 8.0, 2.0)),
        probe(ext(20.0, 12.0, 4.0)),
    ]);
    assert_eq!(stacked.extent, ext(20.0, 12.0, 4.0));
    let placements = place_top_left(stacked, Point::new(5.0, 7.0));
    assert_eq!(placements.len(), 2);
    assert_eq!(placements[0].rect, Rect::new(5.0, 11.0, 15.0, 21.0));
    assert_eq!(placements[1].rect, Rect::new(5.0, 7.0, 25.0, 23.0));
}

#[test]
fn pad_grows_extent_and_offsets_the_child() {
    let p = pad(Insets::new(3.0, 5.0, 7.0, 1.0), probe(ext(10.0, 8.0, 2.0)));
    assert_eq!(p.extent, ext(20.0, 13.0, 3.0));

    let placed = place(p, Placement::root(Rect::new(0.0, 87.0, 20.0, 103.0)));
    assert_eq!(placed[0].rect, Rect::new(3.0, 92.0, 13.0, 102.0));
}

#[test]
fn min_width_pads_narrow_children_and_leaves_wide_ones() {
    let narrow = min_width(25.0, probe(ext(10.0, 8.0, 2.0)));
    assert_eq!(narrow.extent, ext(25.0, 8.0, 2.0));
    let placed = place_top_left(narrow, Point::ZERO);
    assert_eq!(placed[0].rect, Rect::new(0.0, 0.0, 10.0, 10.0));

    let wide = min_width(25.0, probe(ext(30.0, 8.0, 2.0)));
    assert_eq!(wide.extent, ext(30.0, 8.0, 2.0));
}

#[test]
fn before_contributes_under_the_child_and_after_on_top() {
    let child = probe(ext(10.0, 8.0, 2.0));
    let marked = before(child, |placement| {
        vec![Placement::root(placement.rect.inflate(1.0, 1.0))]
    });
    let placed = place_top_left(marked, Point::ZERO);
    assert_eq!(placed[0].rect, Rect::new(-1.0, -1.0, 11.0, 11.0));
    assert_eq!(placed[1].rect, Rect::new(0.0, 0.0, 10.0, 10.0));

    let child = probe(ext(10.0, 8.0, 2.0));
    let covered = after(child, |placement| {
        vec![Placement::root(placement.rect.inflate(1.0, 1.0))]
    });
    let placed = place_top_left(covered, Point::ZERO);
    assert_eq!(placed[0].rect, Rect::new(0.0, 0.0, 10.0, 10.0));
    assert_eq!(placed[1].rect, Rect::new(-1.0, -1.0, 11.0, 11.0));
}

#[test]
fn direct_leaves_and_decorators_preserve_placement_order() {
    let child = direct_probe(ext(10.0, 8.0, 2.0));
    let marked = before_into(child, |placement, out| {
        out.push(Placement::root(placement.rect.inflate(1.0, 1.0)));
    });
    let covered = after_into(marked, |placement, out| {
        out.push(Placement::root(placement.rect.inflate(2.0, 2.0)));
    });
    let placed = place_top_left(covered, Point::ZERO);
    assert_eq!(
        placed.iter().map(|p| p.rect).collect::<Vec<_>>(),
        vec![
            Rect::new(-1.0, -1.0, 11.0, 11.0),
            Rect::new(0.0, 0.0, 10.0, 10.0),
            Rect::new(-2.0, -2.0, 12.0, 12.0),
        ]
    );
}

#[test]
fn around_controls_the_inner_placement_and_sees_the_clip_rect() {
    let child = leaf(ext(20.0, 10.0, 10.0), |_| vec!["inner"]);
    let wrapped = around(child, |placement, place_inner| {
        assert_eq!(placement.rect, Rect::new(0.0, 0.0, 20.0, 20.0));
        assert_eq!(placement.clip_rect, Rect::new(5.0, -5.0, 25.0, 15.0));
        let mut out = vec!["before"];
        out.extend(place_inner.place());
        out.push("after");
        out
    });
    let placed = place(
        wrapped,
        Placement::new(
            Rect::new(0.0, 0.0, 20.0, 20.0),
            Rect::new(5.0, -5.0, 25.0, 15.0),
        ),
    );
    assert_eq!(placed, ["before", "inner", "after"]);
}

#[test]
fn around_into_places_its_child_in_the_existing_accumulator() {
    let child = leaf_into(ext(20.0, 10.0, 10.0), |_, out: &mut Vec<&str>| {
        out.push("inner")
    });
    let wrapped = around_into(child, |placement, inner, out| {
        assert_eq!(placement.rect, Rect::new(1.0, 0.0, 21.0, 20.0));
        out.push("before");
        inner.place_into(out);
        out.push("after");
    });
    let layout = row(
        0.0,
        vec![leaf(ext(1.0, 1.0, 0.0), |_| vec!["outer"]), wrapped],
    );
    assert_eq!(
        place_top_left(layout, Point::ZERO),
        ["outer", "before", "inner", "after"]
    );
}

#[test]
fn overlay_floats_a_layer_positioned_from_the_base_output() {
    let base = probe(ext(10.0, 8.0, 2.0));
    let layer = probe(ext(4.0, 3.0, 1.0));
    let floated = overlay(base, layer, |placement, extent, out: &Vec<Placement>| {
        assert_eq!(out[0].rect, placement.rect);
        Some(Placement::root(
            extent.rect_at(Point::new(out[0].rect.x1, out[0].rect.y0)),
        ))
    });
    // The layer is overhang: the node charges only the base.
    assert_eq!(floated.extent, ext(10.0, 8.0, 2.0));
    let placed = place_top_left(floated, Point::ZERO);
    assert_eq!(placed[0].rect, Rect::new(0.0, 0.0, 10.0, 10.0));
    assert_eq!(placed[1].rect, Rect::new(10.0, 0.0, 14.0, 4.0));

    let base = probe(ext(10.0, 8.0, 2.0));
    let declined = overlay(base, probe(ext(4.0, 3.0, 1.0)), |_, _, _| None);
    assert_eq!(place_top_left(declined, Point::ZERO).len(), 1);
}

#[test]
fn around_may_discard_the_inner_placement() {
    let child = leaf(ext(10.0, 5.0, 5.0), |_| vec![true]);
    let wrapped = around(child, |_, _| Vec::new());
    assert!(place_top_left(wrapped, Point::ZERO).is_empty());
}

impl Output for Vec<&'static str> {
    fn empty() -> Self {
        Vec::new()
    }

    fn over(mut self, above: Self) -> Self {
        self.extend(above);
        self
    }
}

impl Output for Vec<bool> {
    fn empty() -> Self {
        Vec::new()
    }

    fn over(mut self, above: Self) -> Self {
        self.extend(above);
        self
    }
}
