//! A scroll viewport: the child, clipped to a viewport and shifted by
//! an offset. Pure in Puri's sense — the offsets are the CALLER's
//! state (like an editor's [`crate::edit::LineEditState`], custody
//! never lives here), extents are known before placement, and the
//! caller derives the clamp from the child's extent it already holds.
//! A placement entry, not a composable node, for now: nesting a
//! scroll area inside other layout needs a clip node kind, which
//! arrives with scroll bars — a later, separate widget.

use crate::draw::Canvas;
use crate::layout::{Extent, Node, place};
use kurbo::{Affine, Point, Rect, Size, Vec2};

/// How far `content` can scroll within `viewport`, per axis.
pub fn max_offset(content: Extent, viewport: Size) -> Vec2 {
    Vec2::new(
        (content.width - viewport.width).max(0.0),
        (content.height() - viewport.height).max(0.0),
    )
}

/// Places `child` shifted up-left by `offset` inside the
/// viewport-sized box at `at` (its top-left), clipped to it. The
/// caller clamps the offset (against [`max_offset`]) before placing.
pub fn place_scrolled<P: Canvas>(
    child: Node<P>,
    ctx: &mut P,
    at: Point,
    viewport: Size,
    offset: Vec2,
) {
    let ascent = child.extent.ascent;
    let rect = Rect::new(at.x, at.y, at.x + viewport.width, at.y + viewport.height);
    ctx.clip(rect, Affine::IDENTITY, |ctx| {
        place(
            child,
            ctx,
            Point::new(at.x - offset.x, at.y - offset.y + ascent),
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::{DrawCmd, DrawList, Shape};
    use crate::layout::leaf;

    #[test]
    fn content_shifts_by_the_offset_inside_a_clip() {
        let probe = leaf(
            Extent {
                width: 100.0,
                ascent: 0.0,
                descent: 300.0,
            },
            |list: &mut DrawList, at| {
                list.fill(
                    Rect::new(at.x, at.y, at.x + 1.0, at.y + 1.0),
                    peniko::Color::WHITE,
                    Affine::IDENTITY,
                );
            },
        );
        let mut list = DrawList::new();
        place_scrolled(
            probe,
            &mut list,
            Point::new(10.0, 20.0),
            Size::new(80.0, 50.0),
            Vec2::new(5.0, 40.0),
        );
        let [DrawCmd::Clip {
            shape: Shape::Rect(clip),
            children,
            ..
        }] = &list.0[..]
        else {
            panic!("expected one clip");
        };
        assert_eq!(*clip, Rect::new(10.0, 20.0, 90.0, 70.0));
        let [DrawCmd::Fill {
            shape: Shape::Rect(dot),
            ..
        }] = &children[..]
        else {
            panic!("expected the probe inside the clip");
        };
        assert_eq!((dot.x0, dot.y0), (5.0, -20.0));
    }

    #[test]
    fn max_offset_is_the_overflow_per_axis() {
        let content = Extent {
            width: 300.0,
            ascent: 100.0,
            descent: 150.0,
        };
        assert_eq!(
            max_offset(content, Size::new(200.0, 60.0)),
            Vec2::new(100.0, 190.0)
        );
        assert_eq!(max_offset(content, Size::new(400.0, 400.0)), Vec2::ZERO);
    }
}
