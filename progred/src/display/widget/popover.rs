//! The editor's standard card policy, composed above out-of-flow box placement.

use super::{before, hover::block_hover};
use crate::display::{Layout, floating};
use measured::Extent;
use puri::{Placement, Point, Rect, Size, Stroke};
use puri_widgets::panel::Panel;
use std::rc::Rc;

pub fn popover<World: 'static, Hover: Clone + PartialEq + 'static>(
    trigger: Layout<World, Hover>,
    content: Layout<World, Hover>,
) -> Layout<World, Hover> {
    floating(trigger, card(content), |scale, anchor, extent| {
        position(anchor, extent, 4.0 * scale)
    })
}

fn card<World: 'static, Hover: Clone + PartialEq + 'static>(
    content: Layout<World, Hover>,
) -> Layout<World, Hover> {
    block_hover(before(
        crate::display::padding((10.0, 10.0, 10.0, 10.0).into(), content),
        Rc::new(|context| {
            let panel = Panel {
                fill: Some(context.inputs.styles.palette.panel.into()),
                border: Some((
                    Stroke::new(context.inputs.styles.scale),
                    context.inputs.styles.palette.border.into(),
                )),
                radius: 6.0 * context.inputs.styles.scale,
            };
            Box::new(move |output, placement| {
                if !placement.clipped_out() {
                    output.render(move |canvas, _| panel.place(canvas, placement));
                }
            })
        }),
    ))
}

pub fn position(anchor: Placement, extent: Extent, gap: f64) -> Option<Placement> {
    (!anchor.clipped_out()).then(|| {
        Placement::new(
            rect(anchor.rect, extent.size(), anchor.clip_rect, gap),
            anchor.clip_rect,
        )
    })
}

pub fn rect(anchor: Rect, size: Size, bounds: Rect, gap: f64) -> Rect {
    let below = anchor.y1 + gap;
    let above = anchor.y0 - gap - size.height;
    let y = if below + size.height <= bounds.y1 || above < bounds.y0 {
        below.min((bounds.y1 - size.height).max(bounds.y0))
    } else {
        above
    };
    Rect::from_origin_size(Point::new(left(anchor, size, bounds), y), size)
}

/// Like [`rect`], for a card that can show less of itself: one that fits
/// neither below nor above its anchor takes the roomier side, shortened to
/// what fits there, rather than covering the anchor. It keeps at least
/// `minimum` of its height, overlapping the anchor if it must.
pub fn shortened(anchor: Rect, size: Size, bounds: Rect, gap: f64, minimum: f64) -> Rect {
    let below = bounds.y1 - anchor.y1 - gap;
    let above = anchor.y0 - gap - bounds.y0;
    let downward = size.height <= below || (size.height > above && below >= above);
    let room = if downward { below } else { above };
    let height = room.max(minimum).min(size.height);
    let y = if downward {
        (anchor.y1 + gap).min((bounds.y1 - height).max(bounds.y0))
    } else {
        (anchor.y0 - gap - height).max(bounds.y0)
    };
    Rect::from_origin_size(
        Point::new(left(anchor, size, bounds), y),
        Size::new(size.width, height),
    )
}

/// Aligned with the anchor's left edge, as far as the bounds allow.
fn left(anchor: Rect, size: Size, bounds: Rect) -> f64 {
    anchor
        .x0
        .clamp(bounds.x0, (bounds.x1 - size.width).max(bounds.x0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placement_uses_the_ancestor_clip_and_skips_an_offscreen_anchor() {
        let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
        let extent = Extent {
            width: 30.0,
            ascent: 0.0,
            descent: 40.0,
        };
        let place = |rect| position(Placement::new(rect, bounds), extent, 4.0);
        assert_eq!(
            place(Rect::new(85.0, 5.0, 95.0, 15.0)),
            Some(Placement::new(Rect::new(70.0, 19.0, 100.0, 59.0), bounds))
        );
        assert_eq!(
            place(Rect::new(85.0, 85.0, 95.0, 95.0)),
            Some(Placement::new(Rect::new(70.0, 41.0, 100.0, 81.0), bounds))
        );
        assert_eq!(place(Rect::new(110.0, 0.0, 120.0, 10.0)), None);
    }

    #[test]
    fn a_card_that_fits_neither_side_shortens_on_the_roomier_one() {
        let bounds = Rect::new(0.0, 0.0, 100.0, 100.0);
        let size = Size::new(30.0, 60.0);
        let place = |y0, y1| shortened(Rect::new(10.0, y0, 40.0, y1), size, bounds, 0.0, 10.0);
        // Whole below when it fits there, else whole above.
        assert_eq!(place(10.0, 20.0), Rect::new(10.0, 20.0, 40.0, 80.0));
        assert_eq!(place(70.0, 80.0), Rect::new(10.0, 10.0, 40.0, 70.0));
        // Neither: as much as fits on the roomier side, clear of the anchor.
        assert_eq!(place(40.0, 50.0), Rect::new(10.0, 50.0, 40.0, 100.0));
        assert_eq!(place(55.0, 65.0), Rect::new(10.0, 0.0, 40.0, 55.0));
        // Never shorter than the minimum, even if it then covers the anchor.
        let cramped = Rect::new(0.0, 0.0, 100.0, 30.0);
        assert_eq!(
            shortened(Rect::new(10.0, 10.0, 40.0, 20.0), size, cramped, 0.0, 15.0),
            Rect::new(10.0, 15.0, 40.0, 30.0)
        );
    }
}
