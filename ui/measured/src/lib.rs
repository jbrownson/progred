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
    kind: Kind<Out>,
}

/// A child held back for its wrapper: place it, move it, or drop it.
/// It may be collected as an isolated output or placed directly into
/// the frame's existing accumulator.
pub struct PlaceInner<Out> {
    child: Measured<Out>,
    placement: Placement,
}

pub fn child_placement(parent: Placement, rect: Rect) -> Placement {
    Placement::new(rect, parent.clip_rect)
}

pub fn clipped_placement(placement: Placement, bounds: Rect) -> Placement {
    Placement::new(placement.rect, placement.clip_rect.intersect(bounds))
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

type PlaceInto<Out> = Box<dyn FnOnce(Placement, &mut Out)>;
type PlaceAround<Out> = Box<dyn FnOnce(Placement, PlaceInner<Out>, &mut Out)>;

enum Kind<Out> {
    Leaf(PlaceInto<Out>),
    Row {
        children: Vec<Measured<Out>>,
        gap: f64,
        centered: bool,
    },
    Col {
        children: Vec<Measured<Out>>,
        gap: f64,
    },
    Overlay {
        children: Vec<Measured<Out>>,
    },
    Pad {
        child: Box<Measured<Out>>,
        insets: Insets,
    },
    Around {
        child: Box<Measured<Out>>,
        place: PlaceAround<Out>,
    },
}

pub fn leaf<Out: Output>(
    extent: Extent,
    place: impl FnOnce(Placement) -> Out + 'static,
) -> Measured<Out> {
    leaf_into(extent, move |placement, out| {
        contribute(out, place(placement))
    })
}

/// A leaf interpreted directly into the placement accumulator. This is
/// the production path for outputs whose algebra is naturally mutable;
/// [`leaf`] remains useful for small initial encodings in tests.
pub fn leaf_into<Out>(
    extent: Extent,
    place: impl FnOnce(Placement, &mut Out) + 'static,
) -> Measured<Out> {
    Measured {
        extent,
        kind: Kind::Leaf(Box::new(place)),
    }
}

/// Children on one baseline: ascent and descent are the maxima.
pub fn row<Out>(gap: f64, children: Vec<Measured<Out>>) -> Measured<Out> {
    row_aligned(gap, children, false)
}

/// Children centered vertically in the tallest child's line box. The
/// tallest child supplies the resulting baseline, so the row remains
/// typographic when composed into a larger layout.
pub fn centered_row<Out>(gap: f64, children: Vec<Measured<Out>>) -> Measured<Out> {
    row_aligned(gap, children, true)
}

fn row_aligned<Out>(gap: f64, children: Vec<Measured<Out>>, centered: bool) -> Measured<Out> {
    let width = children.iter().map(|c| c.extent.width).sum::<f64>()
        + gap * children.len().saturating_sub(1) as f64;
    let (ascent, descent) = if centered {
        children
            .iter()
            .reduce(|tallest, child| {
                if child.extent.height() > tallest.extent.height() {
                    child
                } else {
                    tallest
                }
            })
            .map(|child| (child.extent.ascent, child.extent.descent))
            .unwrap_or_default()
    } else {
        (
            children
                .iter()
                .map(|c| c.extent.ascent)
                .fold(0.0_f64, f64::max),
            children
                .iter()
                .map(|c| c.extent.descent)
                .fold(0.0_f64, f64::max),
        )
    };
    Measured {
        extent: Extent {
            width,
            ascent,
            descent,
        },
        kind: Kind::Row {
            children,
            gap,
            centered,
        },
    }
}

/// Children stacked; the column's baseline is child `baseline`'s.
pub fn col<Out>(baseline: usize, gap: f64, children: Vec<Measured<Out>>) -> Measured<Out> {
    let extent = if children.is_empty() {
        Extent::default()
    } else {
        assert!(baseline < children.len());
        let width = children
            .iter()
            .map(|c| c.extent.width)
            .fold(0.0_f64, f64::max);
        let total = children.iter().map(|c| c.extent.height()).sum::<f64>()
            + gap * (children.len() - 1) as f64;
        let ascent = children[..baseline]
            .iter()
            .map(|c| c.extent.height())
            .sum::<f64>()
            + gap * baseline as f64
            + children[baseline].extent.ascent;
        Extent {
            width,
            ascent,
            descent: total - ascent,
        }
    };
    Measured {
        extent,
        kind: Kind::Col { children, gap },
    }
}

/// Place children on the same origin and baseline in back-to-front
/// order. The overlay is large enough for every child.
pub fn layers<Out>(children: Vec<Measured<Out>>) -> Measured<Out> {
    let extent = Extent {
        width: children
            .iter()
            .map(|child| child.extent.width)
            .fold(0.0_f64, f64::max),
        ascent: children
            .iter()
            .map(|child| child.extent.ascent)
            .fold(0.0_f64, f64::max),
        descent: children
            .iter()
            .map(|child| child.extent.descent)
            .fold(0.0_f64, f64::max),
    };
    Measured {
        extent,
        kind: Kind::Overlay { children },
    }
}

pub fn pad<Out>(insets: Insets, child: Measured<Out>) -> Measured<Out> {
    let e = child.extent;
    Measured {
        extent: Extent {
            width: e.width + insets.x0 + insets.x1,
            ascent: e.ascent + insets.y0,
            descent: e.descent + insets.y1,
        },
        kind: Kind::Pad {
            child: Box::new(child),
            insets,
        },
    }
}

/// Holds `child` to at least `min` wide by padding on the right: a
/// frame's minimum, not the child's — the child keeps its own extent
/// and placement.
pub fn min_width<Out>(min: f64, child: Measured<Out>) -> Measured<Out> {
    let deficit = (min - child.extent.width).max(0.0);
    pad(Insets::new(0.0, 0.0, deficit, 0.0), child)
}

/// Transparently wraps this layout's placement. The wrapper receives
/// the settled placement and the held-back child, and returns the
/// combined output — placing the child exactly once, or not at all.
pub fn around<Out>(
    child: Measured<Out>,
    place: impl FnOnce(Placement, PlaceInner<Out>) -> Out + 'static,
) -> Measured<Out>
where
    Out: Output,
{
    around_into(child, move |placement, inner, out| {
        contribute(out, place(placement, inner))
    })
}

/// Transparently wrap direct placement. The callback may save and
/// restore a small piece of accumulator state around `inner.place_into`,
/// but the child's ordinary contributions flow straight to the frame.
pub fn around_into<Out>(
    child: Measured<Out>,
    place: impl FnOnce(Placement, PlaceInner<Out>, &mut Out) + 'static,
) -> Measured<Out> {
    Measured {
        extent: child.extent,
        kind: Kind::Around {
            child: Box::new(child),
            place: Box::new(place),
        },
    }
}

/// Pin `layer` over `base` at a position of the caller's choosing.
/// The layer is overhang: the node keeps the base's extent, so layout
/// never pays for what floats. `position` sees the base's settled
/// placement, the layer's extent, and the base's placed output — the
/// hook for overlays anchored to something the base discovered while
/// placing — and may decline, placing nothing.
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

/// Contribute under this layout, before its content and descendants.
pub fn before<Out: Output>(
    child: Measured<Out>,
    place_before: impl FnOnce(Placement) -> Out + 'static,
) -> Measured<Out> {
    around(child, move |placement, inner| {
        place_before(placement).over(inner.place())
    })
}

/// Contribute over this layout, on top of its content and descendants.
pub fn after<Out: Output>(
    child: Measured<Out>,
    place_after: impl FnOnce(Placement) -> Out + 'static,
) -> Measured<Out> {
    around(child, move |placement, inner| {
        inner.place().over(place_after(placement))
    })
}

/// Contribute directly before placing `child`, without constructing a
/// temporary subtree output.
pub fn before_into<Out: Output>(
    child: Measured<Out>,
    place_before: impl FnOnce(Placement, &mut Out) + 'static,
) -> Measured<Out> {
    around_into(child, move |placement, inner, out| {
        place_before(placement, out);
        inner.place_into(out);
    })
}

/// Contribute directly after placing `child`, without constructing a
/// temporary subtree output.
pub fn after_into<Out: Output>(
    child: Measured<Out>,
    place_after: impl FnOnce(Placement, &mut Out) + 'static,
) -> Measured<Out> {
    around_into(child, move |placement, inner, out| {
        inner.place_into(out);
        place_after(placement, out);
    })
}

/// The historical leading decoration operation, retained as the
/// rectangle-only spelling of [`before`].
pub fn decorate<Out: Output>(
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

fn place_into<Out: Output>(layout: Measured<Out>, placement: Placement, out: &mut Out) {
    let extent = layout.extent;
    let at = Point::new(placement.rect.x0, placement.rect.y0 + extent.ascent);
    match layout.kind {
        Kind::Leaf(f) => f(placement, out),
        Kind::Row {
            children,
            gap,
            centered,
        } => {
            let mut x = at.x;
            let top = at.y - extent.ascent;
            for child in children {
                let advance = child.extent.width + gap;
                let y = if centered {
                    top + (extent.height() - child.extent.height()) / 2.0
                } else {
                    at.y - child.extent.ascent
                };
                let rect = Rect::new(x, y, x + child.extent.width, y + child.extent.height());
                place_into(child, child_placement(placement, rect), out);
                x += advance;
            }
        }
        Kind::Col { children, gap } => {
            let mut y = at.y - extent.ascent;
            for child in children {
                let advance = child.extent.height() + gap;
                let child_baseline = y + child.extent.ascent;
                let rect = Rect::new(
                    at.x,
                    child_baseline - child.extent.ascent,
                    at.x + child.extent.width,
                    child_baseline + child.extent.descent,
                );
                place_into(child, child_placement(placement, rect), out);
                y += advance;
            }
        }
        Kind::Overlay { children } => {
            for child in children {
                let rect = Rect::new(
                    at.x,
                    at.y - child.extent.ascent,
                    at.x + child.extent.width,
                    at.y + child.extent.descent,
                );
                place_into(child, child_placement(placement, rect), out);
            }
        }
        Kind::Pad { child, insets } => {
            let child_at = Point::new(at.x + insets.x0, at.y);
            let rect = Rect::new(
                child_at.x,
                child_at.y - child.extent.ascent,
                child_at.x + child.extent.width,
                child_at.y + child.extent.descent,
            );
            place_into(*child, child_placement(placement, rect), out)
        }
        Kind::Around { child, place } => place(
            placement,
            PlaceInner {
                child: *child,
                placement,
            },
            out,
        ),
    }
}

/// `at` is the top-left corner of the layout.
pub fn place_top_left<Out: Output>(layout: Measured<Out>, at: Point) -> Out {
    let placement = Placement::root(layout.extent.rect_at(at));
    place(layout, placement)
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
