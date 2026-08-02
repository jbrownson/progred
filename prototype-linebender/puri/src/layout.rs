//! Boxes with baselines: the TeX/pict model. A box is (width, ascent,
//! descent) plus a way to place itself; rows compose on baselines,
//! columns stack with a chosen child's baseline.
//!
//! Invariants the future pretty-printing layer relies on: extents are
//! known at construction (before placement), construction has no side
//! effects so alternative layouts can be built and discarded, and
//! placement is the single traversal that touches the context `P`.

use kurbo::{Insets, Point, Rect, Size};

/// A node's full settled rectangle and the effective enclosing
/// axis-aligned clipping area.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub rect: Rect,
    pub clip_rect: Rect,
}

impl Placement {
    pub const fn new(rect: Rect, clip_rect: Rect) -> Self {
        Self { rect, clip_rect }
    }

    pub const fn root(rect: Rect) -> Self {
        Self::new(rect, rect)
    }

    pub fn child(self, rect: Rect) -> Self {
        Self {
            rect,
            clip_rect: self.clip_rect,
        }
    }

    pub fn clipped_by(self, bounds: Rect) -> Self {
        Self {
            clip_rect: self.clip_rect.intersect(bounds),
            ..self
        }
    }

    pub fn visible_rect(self) -> Rect {
        self.rect.intersect(self.clip_rect)
    }

    pub fn clipped_out(self) -> bool {
        let visible = self.visible_rect();
        visible.width() <= 0.0 || visible.height() <= 0.0
    }

    pub fn contains(self, point: Point) -> bool {
        !self.clipped_out() && self.rect.contains(point) && self.clip_rect.contains(point)
    }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HAlign {
    Start,
    Center,
    End,
}

pub struct Node<P> {
    pub extent: Extent,
    kind: Kind<P>,
}

pub struct PlaceInner<P> {
    child: Node<P>,
    placement: Placement,
}

impl<P> PlaceInner<P> {
    pub fn place(self, ctx: &mut P) {
        place(self.child, ctx, self.placement);
    }
}

type PlaceAround<P> = Box<dyn FnOnce(&mut P, Placement, PlaceInner<P>)>;
type PlaceLeaf<P> = Box<dyn FnOnce(&mut P, Placement)>;

enum Kind<P> {
    Leaf(PlaceLeaf<P>),
    Row {
        children: Vec<Node<P>>,
        gap: f64,
    },
    Col {
        children: Vec<Node<P>>,
        align: HAlign,
        gap: f64,
    },
    Pad {
        child: Box<Node<P>>,
        insets: Insets,
    },
    Around {
        child: Box<Node<P>>,
        place: PlaceAround<P>,
    },
}

pub fn leaf<P>(extent: Extent, place: impl FnOnce(&mut P, Placement) + 'static) -> Node<P> {
    Node {
        extent,
        kind: Kind::Leaf(Box::new(place)),
    }
}

/// Children on one baseline: ascent and descent are the maxima.
pub fn row<P>(gap: f64, children: Vec<Node<P>>) -> Node<P> {
    let width = children.iter().map(|c| c.extent.width).sum::<f64>()
        + gap * children.len().saturating_sub(1) as f64;
    let ascent = children
        .iter()
        .map(|c| c.extent.ascent)
        .fold(0.0_f64, f64::max);
    let descent = children
        .iter()
        .map(|c| c.extent.descent)
        .fold(0.0_f64, f64::max);
    Node {
        extent: Extent {
            width,
            ascent,
            descent,
        },
        kind: Kind::Row { children, gap },
    }
}

/// Children stacked; the column's baseline is child `baseline`'s.
pub fn col<P>(align: HAlign, baseline: usize, gap: f64, children: Vec<Node<P>>) -> Node<P> {
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
    Node {
        extent,
        kind: Kind::Col {
            children,
            align,
            gap,
        },
    }
}

pub fn pad<P>(insets: Insets, child: Node<P>) -> Node<P> {
    let e = child.extent;
    Node {
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
pub fn min_width<P>(min: f64, child: Node<P>) -> Node<P> {
    let deficit = (min - child.extent.width).max(0.0);
    pad(Insets::new(0.0, 0.0, deficit, 0.0), child)
}

/// Transparently wraps this node's placement. `place_inner` places
/// its content and descendants exactly once when invoked.
pub fn around<P>(
    child: Node<P>,
    place: impl FnOnce(&mut P, Placement, PlaceInner<P>) + 'static,
) -> Node<P> {
    Node {
        extent: child.extent,
        kind: Kind::Around {
            child: Box::new(child),
            place: Box::new(place),
        },
    }
}

/// Run `place_before` while entering this node, before its content
/// and descendants.
pub fn before<P>(
    child: Node<P>,
    place_before: impl FnOnce(&mut P, Placement) + 'static,
) -> Node<P> {
    around(child, move |ctx, placement, place_inner| {
        place_before(ctx, placement);
        place_inner.place(ctx);
    })
}

/// Run `place_after` while leaving this node, after its content and
/// descendants.
pub fn after<P>(
    child: Node<P>,
    place_after: impl FnOnce(&mut P, Placement) + 'static,
) -> Node<P> {
    around(child, move |ctx, placement, place_inner| {
        place_inner.place(ctx);
        place_after(ctx, placement);
    })
}

/// The historical leading decoration operation, retained as the
/// rectangle-only spelling of [`before`].
pub fn decorate<P>(
    child: Node<P>,
    draw: impl FnOnce(&mut P, Rect) + 'static,
) -> Node<P> {
    before(child, move |ctx, placement| draw(ctx, placement.rect))
}

pub fn place<P>(node: Node<P>, ctx: &mut P, placement: Placement) {
    let extent = node.extent;
    let at = Point::new(placement.rect.x0, placement.rect.y0 + extent.ascent);
    match node.kind {
        Kind::Leaf(f) => f(ctx, placement),
        Kind::Row { children, gap } => {
            let mut x = at.x;
            for child in children {
                let advance = child.extent.width + gap;
                let rect = Rect::new(
                    x,
                    at.y - child.extent.ascent,
                    x + child.extent.width,
                    at.y + child.extent.descent,
                );
                place(child, ctx, placement.child(rect));
                x += advance;
            }
        }
        Kind::Col {
            children,
            align,
            gap,
        } => {
            let mut y = at.y - extent.ascent;
            for child in children {
                let slack = extent.width - child.extent.width;
                let x = at.x
                    + match align {
                        HAlign::Start => 0.0,
                        HAlign::Center => slack / 2.0,
                        HAlign::End => slack,
                    };
                let advance = child.extent.height() + gap;
                let child_baseline = y + child.extent.ascent;
                let rect = Rect::new(
                    x,
                    child_baseline - child.extent.ascent,
                    x + child.extent.width,
                    child_baseline + child.extent.descent,
                );
                place(child, ctx, placement.child(rect));
                y += advance;
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
            place(*child, ctx, placement.child(rect));
        }
        Kind::Around { child, place: wrap } => {
            wrap(
                ctx,
                placement,
                PlaceInner {
                    child: *child,
                    placement,
                },
            );
        }
    }
}

/// `at` is the top-left corner of the node.
pub fn place_top_left<P>(node: Node<P>, ctx: &mut P, at: Point) {
    let placement = Placement::root(node.extent.rect_at(at));
    place(node, ctx, placement);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probe(extent: Extent) -> Node<Vec<Placement>> {
        leaf(extent, move |placed: &mut Vec<Placement>, placement| {
            placed.push(placement)
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
    fn row_places_children_on_one_baseline() {
        let r = row(4.0, vec![probe(ext(10.0, 8.0, 2.0)), probe(ext(20.0, 12.0, 4.0))]);
        assert_eq!(r.extent, ext(34.0, 12.0, 4.0));

        let mut placed = Vec::new();
        place(
            r,
            &mut placed,
            Placement::root(Rect::new(0.0, 88.0, 34.0, 104.0)),
        );
        assert_eq!(
            placed.iter().map(|p| p.rect).collect::<Vec<_>>(),
            vec![
                Rect::new(0.0, 92.0, 10.0, 102.0),
                Rect::new(14.0, 88.0, 34.0, 104.0),
            ]
        );
    }

    #[test]
    fn col_takes_the_chosen_childs_baseline() {
        let c = col(
            HAlign::Start,
            1,
            2.0,
            vec![
                probe(ext(10.0, 8.0, 2.0)),
                probe(ext(4.0, 4.0, 0.0)),
                probe(ext(10.0, 8.0, 2.0)),
            ],
        );
        assert_eq!(c.extent, ext(10.0, 16.0, 12.0));

        let mut placed = Vec::new();
        place(
            c,
            &mut placed,
            Placement::root(Rect::new(0.0, 84.0, 10.0, 112.0)),
        );
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
    fn col_centers_narrow_children() {
        let c = col(
            HAlign::Center,
            0,
            0.0,
            vec![probe(ext(10.0, 5.0, 0.0)), probe(ext(30.0, 5.0, 0.0))],
        );
        let mut placed = Vec::new();
        place_top_left(c, &mut placed, Point::ZERO);
        assert_eq!(placed[0].rect.x0, 10.0);
        assert_eq!(placed[1].rect.x0, 0.0);
    }

    #[test]
    fn pad_grows_extent_and_offsets_the_child() {
        let p = pad(Insets::new(3.0, 5.0, 7.0, 1.0), probe(ext(10.0, 8.0, 2.0)));
        assert_eq!(p.extent, ext(20.0, 13.0, 3.0));

        let mut placed = Vec::new();
        place(
            p,
            &mut placed,
            Placement::root(Rect::new(0.0, 87.0, 20.0, 103.0)),
        );
        assert_eq!(placed[0].rect, Rect::new(3.0, 92.0, 13.0, 102.0));
    }

    #[test]
    fn min_width_pads_narrow_children_and_leaves_wide_ones() {
        let narrow = min_width(25.0, probe(ext(10.0, 8.0, 2.0)));
        assert_eq!(narrow.extent, ext(25.0, 8.0, 2.0));
        let mut placed = Vec::new();
        place_top_left(narrow, &mut placed, Point::ZERO);
        assert_eq!(placed[0].rect, Rect::new(0.0, 0.0, 10.0, 10.0));

        let wide = min_width(25.0, probe(ext(30.0, 8.0, 2.0)));
        assert_eq!(wide.extent, ext(30.0, 8.0, 2.0));
    }

    #[test]
    fn decorate_receives_the_subtree_rect() {
        struct Ctx {
            rects: Vec<Rect>,
            placed: Vec<Placement>,
        }
        let child = leaf(ext(10.0, 8.0, 2.0), |ctx: &mut Ctx, placement| {
            ctx.placed.push(placement)
        });
        let d = decorate(child, |ctx: &mut Ctx, rect| ctx.rects.push(rect));

        let mut ctx = Ctx {
            rects: Vec::new(),
            placed: Vec::new(),
        };
        place(
            d,
            &mut ctx,
            Placement::root(Rect::new(5.0, 92.0, 15.0, 102.0)),
        );
        assert_eq!(ctx.rects, vec![Rect::new(5.0, 92.0, 15.0, 102.0)]);
        assert_eq!(ctx.placed[0].rect, Rect::new(5.0, 92.0, 15.0, 102.0));
    }

    #[test]
    fn around_controls_the_inner_placement_and_sees_the_clip_rect() {
        struct Ctx {
            events: Vec<&'static str>,
        }
        let child = leaf(ext(20.0, 10.0, 10.0), |ctx: &mut Ctx, _| {
            ctx.events.push("inner");
        });
        let wrapped = around(child, |ctx: &mut Ctx, placement, place_inner| {
            assert_eq!(placement.rect, Rect::new(0.0, 0.0, 20.0, 20.0));
            assert_eq!(placement.clip_rect, Rect::new(5.0, -5.0, 25.0, 15.0));
            ctx.events.push("before");
            place_inner.place(ctx);
            ctx.events.push("after");
        });
        let mut ctx = Ctx {
            events: Vec::new(),
        };
        place(
            wrapped,
            &mut ctx,
            Placement::new(
                Rect::new(0.0, 0.0, 20.0, 20.0),
                Rect::new(5.0, -5.0, 25.0, 15.0),
            ),
        );
        assert_eq!(ctx.events, ["before", "inner", "after"]);
    }

    #[test]
    fn around_may_discard_the_inner_placement() {
        struct Ctx {
            placed: bool,
        }
        let child = leaf(ext(10.0, 5.0, 5.0), |ctx: &mut Ctx, _| ctx.placed = true);
        let wrapped = around(child, |_: &mut Ctx, _, _| {});
        let mut ctx = Ctx {
            placed: false,
        };
        place_top_left(wrapped, &mut ctx, Point::ZERO);
        assert!(!ctx.placed);
    }

    #[test]
    fn child_placements_inherit_the_enclosing_clip() {
        let root = Placement::new(
            Rect::new(0.0, 0.0, 100.0, 100.0),
            Rect::new(40.0, 20.0, 120.0, 80.0),
        );
        assert_eq!(root.clip_rect, Rect::new(40.0, 20.0, 120.0, 80.0));
        let child = root.child(Rect::new(20.0, 50.0, 60.0, 90.0));
        assert_eq!(child.clip_rect, root.clip_rect);
        assert_eq!(child.visible_rect(), Rect::new(40.0, 50.0, 60.0, 80.0));
        let hidden = child.child(Rect::new(70.0, 0.0, 90.0, 10.0));
        assert_eq!(hidden.clip_rect, root.clip_rect);
        assert!(hidden.clipped_out());

        let clipped = child.clipped_by(Rect::new(50.0, 0.0, 80.0, 100.0));
        assert_eq!(clipped.clip_rect, Rect::new(50.0, 20.0, 80.0, 80.0));
    }
}
