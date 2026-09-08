//! Measured boxes with baselines: the TeX/pict model. A [`Measured`]
//! box is (width, ascent, descent) plus a way to place itself; rows
//! compose on baselines, columns stack with a chosen child's baseline.
//!
//! Invariants the pretty-printing layer relies on: extents are known
//! at construction (before placement), construction has no side
//! effects so alternative layouts can be built and discarded, and
//! placement is one pure traversal from settled geometry to the
//! caller's [`Output`].
//!
//! The engine is generic in what placement produces. Leaves either
//! yield an `Out` from their settled [`Placement`] or contribute
//! directly to the frame's accumulator. Containers visit children in
//! placement order, so the later-placed contribution is the one on top
//! — painted last, asked first.

use kurbo::{Insets, Point, Rect, Size};
use uig::Placement;

pub mod choices;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RowAlignment {
    Baseline,
    Center,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Extent {
    pub width: f64,
    pub ascent: f64,
    pub descent: f64,
}

impl Extent {
    pub fn height(&self) -> f64 {
        self.ascent + self.descent
    }

    pub fn size(&self) -> Size {
        Size::new(self.width, self.height())
    }

    pub fn rect_at(&self, at: Point) -> Rect {
        Rect::from_origin_size(at, self.size())
    }
}

/// What a placement pass accumulates: a monoid whose combine names its
/// asymmetry. `base.over(above)` stacks `above` on top of `base`.
pub trait Output {
    fn empty() -> Self;
    fn over(self, above: Self) -> Self;
}

pub struct Measured<Out> {
    pub extent: Extent,
    place: Box<dyn FnOnce(Placement, &mut Out)>,
}

/// A child held back for its wrapper: place it, move it, or drop it.
pub struct PlaceInner<Out> {
    child: Measured<Out>,
    placement: Placement,
}

pub fn child_placement(parent: Placement, rect: Rect) -> Placement {
    Placement::new(rect, parent.clip_rect)
}

pub fn clipped_placement(placement: Placement, bounds: Rect) -> Placement {
    Placement {
        clip_rect: placement.clip_rect.intersect(bounds),
        ..placement
    }
}

impl<Out: Output> PlaceInner<Out> {
    pub fn place(self) -> Out {
        place(self.child, self.placement)
    }

    pub fn place_at(self, placement: Placement) -> Out {
        place(self.child, placement)
    }

    pub fn place_into(self, out: &mut Out) {
        place_into(self.child, self.placement, out)
    }

    pub fn place_at_into(self, placement: Placement, out: &mut Out) {
        place_into(self.child, placement, out)
    }

    pub fn extent(&self) -> Extent {
        self.child.extent
    }
}

pub fn leaf<Out: Output>(
    extent: Extent,
    place: impl FnOnce(Placement) -> Out + 'static,
) -> Measured<Out> {
    leaf_into(extent, move |placement, out| {
        contribute(out, place(placement))
    })
}

pub fn leaf_into<Out>(
    extent: Extent,
    place: impl FnOnce(Placement, &mut Out) + 'static,
) -> Measured<Out> {
    Measured {
        extent,
        place: Box::new(place),
    }
}

pub fn row<Out: Output + 'static>(gap: f64, children: Vec<Measured<Out>>) -> Measured<Out> {
    row_aligned(gap, children, false)
}

pub fn centered_row<Out: Output + 'static>(
    gap: f64,
    children: Vec<Measured<Out>>,
) -> Measured<Out> {
    row_aligned(gap, children, true)
}

fn row_aligned<Out: Output + 'static>(
    gap: f64,
    children: Vec<Measured<Out>>,
    centered: bool,
) -> Measured<Out> {
    let extent = row_extent(gap, centered, children.iter().map(|child| child.extent));
    leaf_into(extent, move |placement, out| {
        place_row(
            extent,
            placement,
            gap,
            centered,
            children,
            |child| child.extent,
            |child, placement| place_into(child, placement, out),
        );
    })
}

pub fn col<Out: Output + 'static>(
    baseline: usize,
    gap: f64,
    children: Vec<Measured<Out>>,
) -> Measured<Out> {
    let extent = col_extent(baseline, gap, children.iter().map(|child| child.extent));
    leaf_into(extent, move |placement, out| {
        place_col(
            placement,
            gap,
            children,
            |child| child.extent,
            |child, placement| place_into(child, placement, out),
        );
    })
}

pub fn layers<Out: Output + 'static>(children: Vec<Measured<Out>>) -> Measured<Out> {
    let extent = overlay_extent(children.iter().map(|child| child.extent));
    leaf_into(extent, move |placement, out| {
        place_layers(
            extent,
            placement,
            children,
            |child| child.extent,
            |child, placement| place_into(child, placement, out),
        );
    })
}

pub fn pad<Out: Output + 'static>(insets: Insets, child: Measured<Out>) -> Measured<Out> {
    let extent = padded_extent(insets, child.extent);
    leaf_into(extent, move |placement, out| {
        let placement = padded_placement(placement, insets, child.extent);
        place_into(child, placement, out)
    })
}

pub fn min_width<Out: Output + 'static>(min: f64, child: Measured<Out>) -> Measured<Out> {
    let deficit = (min - child.extent.width).max(0.0);
    pad(Insets::new(0.0, 0.0, deficit, 0.0), child)
}

pub fn fill_height<Out: Output + 'static>(child: Measured<Out>) -> Measured<Out> {
    around_into(child, |placement, inner, out| {
        inner.place_at_into(placement.fill_height(), out)
    })
}

/// A wrapper controls when and where its child continuation runs.
pub fn around<Out: Output + 'static>(
    child: Measured<Out>,
    place: impl FnOnce(Placement, PlaceInner<Out>) -> Out + 'static,
) -> Measured<Out> {
    around_into(child, move |placement, inner, out| {
        contribute(out, place(placement, inner))
    })
}

pub fn around_into<Out: 'static>(
    child: Measured<Out>,
    place: impl FnOnce(Placement, PlaceInner<Out>, &mut Out) + 'static,
) -> Measured<Out> {
    leaf_into(child.extent, move |placement, out| {
        place(placement, PlaceInner { child, placement }, out)
    })
}

pub fn overlay<Out: Output + 'static>(
    base: Measured<Out>,
    layer: Measured<Out>,
    position: impl FnOnce(Placement, Extent, &Out) -> Option<Placement> + 'static,
) -> Measured<Out> {
    let extent = layer.extent;
    around(base, move |placement, inner| {
        let out = inner.place();
        match position(placement, extent, &out) {
            Some(at) => out.over(place(layer, at)),
            None => out,
        }
    })
}

pub fn before<Out: Output + 'static>(
    child: Measured<Out>,
    place_before: impl FnOnce(Placement) -> Out + 'static,
) -> Measured<Out> {
    around(child, move |placement, inner| {
        place_before(placement).over(inner.place())
    })
}

pub fn after<Out: Output + 'static>(
    child: Measured<Out>,
    place_after: impl FnOnce(Placement) -> Out + 'static,
) -> Measured<Out> {
    around(child, move |placement, inner| {
        inner.place().over(place_after(placement))
    })
}

pub fn before_into<Out: Output + 'static>(
    child: Measured<Out>,
    place_before: impl FnOnce(Placement, &mut Out) + 'static,
) -> Measured<Out> {
    around_into(child, move |placement, inner, out| {
        place_before(placement, out);
        inner.place_into(out);
    })
}

pub fn after_into<Out: Output + 'static>(
    child: Measured<Out>,
    place_after: impl FnOnce(Placement, &mut Out) + 'static,
) -> Measured<Out> {
    around_into(child, move |placement, inner, out| {
        inner.place_into(out);
        place_after(placement, out);
    })
}

pub fn decorate<Out: Output + 'static>(
    child: Measured<Out>,
    draw: impl FnOnce(Rect) -> Out + 'static,
) -> Measured<Out> {
    before(child, move |placement| draw(placement.rect))
}

pub fn place<Out: Output>(layout: Measured<Out>, placement: Placement) -> Out {
    let mut out = Out::empty();
    place_into(layout, placement, &mut out);
    out
}

fn contribute<Out: Output>(out: &mut Out, above: Out) {
    let base = std::mem::replace(out, Out::empty());
    *out = base.over(above);
}

pub(crate) fn place_into<Out>(layout: Measured<Out>, placement: Placement, out: &mut Out) {
    (layout.place)(placement, out)
}

pub fn place_top_left<Out: Output>(layout: Measured<Out>, at: Point) -> Out {
    let placement = Placement::root(layout.extent.rect_at(at));
    place(layout, placement)
}

pub(crate) fn row_extent(
    gap: f64,
    centered: bool,
    children: impl Iterator<Item = Extent>,
) -> Extent {
    let (mut extent, mut count) = (Extent::default(), 0usize);
    for child in children {
        extent.width += child.width;
        if centered {
            if count == 0 || child.height() > extent.height() {
                extent.ascent = child.ascent;
                extent.descent = child.descent;
            }
        } else {
            extent.ascent = extent.ascent.max(child.ascent);
            extent.descent = extent.descent.max(child.descent);
        }
        count += 1;
    }
    extent.width += gap * count.saturating_sub(1) as f64;
    extent
}

pub(crate) fn col_extent(
    baseline: usize,
    gap: f64,
    children: impl ExactSizeIterator<Item = Extent>,
) -> Extent {
    assert!(children.len() == 0 || baseline < children.len());
    let mut extent = Extent::default();
    let mut height = 0.0;
    for (index, child) in children.enumerate() {
        if index > 0 {
            height += gap;
        }
        extent.width = extent.width.max(child.width);
        if index == baseline {
            extent.ascent = height + child.ascent;
        }
        height += child.height();
    }
    extent.descent = height - extent.ascent;
    extent
}

pub(crate) fn overlay_extent(children: impl Iterator<Item = Extent>) -> Extent {
    children.fold(Extent::default(), |extent, child| Extent {
        width: extent.width.max(child.width),
        ascent: extent.ascent.max(child.ascent),
        descent: extent.descent.max(child.descent),
    })
}

pub(crate) fn padded_extent(insets: Insets, child: Extent) -> Extent {
    Extent {
        width: child.width + insets.x0 + insets.x1,
        ascent: child.ascent + insets.y0,
        descent: child.descent + insets.y1,
    }
}

pub(crate) fn place_row<T>(
    extent: Extent,
    placement: Placement,
    gap: f64,
    centered: bool,
    children: Vec<T>,
    extent_of: impl Fn(&T) -> Extent,
    mut place: impl FnMut(T, Placement),
) {
    let mut x = placement.rect.x0;
    for child in children {
        let size = extent_of(&child);
        let y = placement.rect.y0
            + if centered {
                (extent.height() - size.height()) / 2.0
            } else {
                extent.ascent - size.ascent
            };
        let rect = size.rect_at(Point::new(x, y));
        place(
            child,
            child_placement(placement, rect).with_available_rect(Rect::new(
                rect.x0,
                placement.rect.y0,
                rect.x1,
                placement.rect.y1,
            )),
        );
        x += size.width + gap;
    }
}

pub(crate) fn place_col<T>(
    placement: Placement,
    gap: f64,
    children: Vec<T>,
    extent_of: impl Fn(&T) -> Extent,
    mut place: impl FnMut(T, Placement),
) {
    let mut y = placement.rect.y0;
    for child in children {
        let size = extent_of(&child);
        let rect = size.rect_at(Point::new(placement.rect.x0, y));
        place(
            child,
            child_placement(placement, rect).with_available_rect(Rect::new(
                placement.rect.x0,
                rect.y0,
                placement.rect.x1,
                rect.y1,
            )),
        );
        y += size.height() + gap;
    }
}

pub(crate) fn place_layers<T>(
    extent: Extent,
    placement: Placement,
    children: Vec<T>,
    extent_of: impl Fn(&T) -> Extent,
    mut place: impl FnMut(T, Placement),
) {
    for child in children {
        let size = extent_of(&child);
        let rect = size.rect_at(Point::new(
            placement.rect.x0,
            placement.rect.y0 + extent.ascent - size.ascent,
        ));
        place(
            child,
            child_placement(placement, rect).with_available_rect(placement.rect),
        );
    }
}

pub(crate) fn padded_placement(placement: Placement, insets: Insets, child: Extent) -> Placement {
    let rect = child.rect_at(Point::new(
        placement.rect.x0 + insets.x0,
        placement.rect.y0 + insets.y0,
    ));
    child_placement(placement, rect).with_available_rect(placement.available_rect.inset(-insets))
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
