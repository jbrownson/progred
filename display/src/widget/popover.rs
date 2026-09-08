//! The editor's standard card policy, composed above out-of-flow box placement.

use super::{before, hover::block_hover};
use crate::{Layout, floating};
use measured::Extent;
use puri::{Color, Placement, Point, Rect, Size, Stroke};
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
        crate::padding((10.0, 10.0, 10.0, 10.0).into(), content),
        Rc::new(|context| {
            let panel = Panel {
                fill: Some(Color::new([0.985, 0.985, 0.99, 1.0]).into()),
                border: Some((
                    Stroke::new(context.styles.scale),
                    context.styles.dim.brush.clone(),
                )),
                radius: 6.0 * context.styles.scale,
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
    let x = anchor
        .x0
        .clamp(bounds.x0, (bounds.x1 - size.width).max(bounds.x0));
    Rect::from_origin_size(Point::new(x, y), size)
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
}
