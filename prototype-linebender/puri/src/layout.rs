//! Boxes with baselines: the TeX/pict model. A box is (width, ascent,
//! descent) plus a way to place itself; rows compose on baselines,
//! columns stack with a chosen child's baseline.
//!
//! Invariants the future pretty-printing layer relies on: extents are
//! known at construction (before placement), construction has no side
//! effects so alternative layouts can be built and discarded, and
//! placement is the single traversal that touches the context `P`.

use kurbo::{Insets, Point, Rect};

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

enum Kind<P> {
    Leaf(Box<dyn FnOnce(&mut P, Point)>),
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
    Decorate {
        child: Box<Node<P>>,
        draw: Box<dyn FnOnce(&mut P, Rect)>,
    },
}

/// `place` receives the baseline-left origin.
pub fn leaf<P>(extent: Extent, place: impl FnOnce(&mut P, Point) + 'static) -> Node<P> {
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

/// Runs `draw` with the subtree's settled rect before the subtree
/// places — backgrounds now, hit-region emission later.
pub fn decorate<P>(child: Node<P>, draw: impl FnOnce(&mut P, Rect) + 'static) -> Node<P> {
    Node {
        extent: child.extent,
        kind: Kind::Decorate {
            child: Box::new(child),
            draw: Box::new(draw),
        },
    }
}

/// `at` is the baseline-left origin of the node.
pub fn place<P>(node: Node<P>, ctx: &mut P, at: Point) {
    let extent = node.extent;
    match node.kind {
        Kind::Leaf(f) => f(ctx, at),
        Kind::Row { children, gap } => {
            let mut x = at.x;
            for child in children {
                let advance = child.extent.width + gap;
                place(child, ctx, Point::new(x, at.y));
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
                place(child, ctx, Point::new(x, child_baseline));
                y += advance;
            }
        }
        Kind::Pad { child, insets } => {
            place(*child, ctx, Point::new(at.x + insets.x0, at.y));
        }
        Kind::Decorate { child, draw } => {
            draw(
                ctx,
                Rect::new(
                    at.x,
                    at.y - extent.ascent,
                    at.x + extent.width,
                    at.y + extent.descent,
                ),
            );
            place(*child, ctx, at);
        }
    }
}

/// `at` is the top-left corner of the node.
pub fn place_top_left<P>(node: Node<P>, ctx: &mut P, at: Point) {
    let ascent = node.extent.ascent;
    place(node, ctx, Point::new(at.x, at.y + ascent));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probe(extent: Extent) -> Node<Vec<Point>> {
        leaf(extent, move |placed: &mut Vec<Point>, at| placed.push(at))
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
        place(r, &mut placed, Point::new(0.0, 100.0));
        assert_eq!(placed, vec![Point::new(0.0, 100.0), Point::new(14.0, 100.0)]);
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
        place(c, &mut placed, Point::new(0.0, 100.0));
        assert_eq!(
            placed,
            vec![
                Point::new(0.0, 92.0),
                Point::new(0.0, 100.0),
                Point::new(0.0, 110.0),
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
        place(c, &mut placed, Point::new(0.0, 5.0));
        assert_eq!(placed[0].x, 10.0);
        assert_eq!(placed[1].x, 0.0);
    }

    #[test]
    fn pad_grows_extent_and_offsets_the_child() {
        let p = pad(Insets::new(3.0, 5.0, 7.0, 1.0), probe(ext(10.0, 8.0, 2.0)));
        assert_eq!(p.extent, ext(20.0, 13.0, 3.0));

        let mut placed = Vec::new();
        place(p, &mut placed, Point::new(0.0, 100.0));
        assert_eq!(placed, vec![Point::new(3.0, 100.0)]);
    }

    #[test]
    fn decorate_receives_the_subtree_rect() {
        struct Ctx {
            rects: Vec<Rect>,
            placed: Vec<Point>,
        }
        let child = leaf(ext(10.0, 8.0, 2.0), |ctx: &mut Ctx, at| ctx.placed.push(at));
        let d = decorate(child, |ctx: &mut Ctx, rect| ctx.rects.push(rect));

        let mut ctx = Ctx {
            rects: Vec::new(),
            placed: Vec::new(),
        };
        place(d, &mut ctx, Point::new(5.0, 100.0));
        assert_eq!(ctx.rects, vec![Rect::new(5.0, 92.0, 15.0, 102.0)]);
        assert_eq!(ctx.placed, vec![Point::new(5.0, 100.0)]);
    }
}
