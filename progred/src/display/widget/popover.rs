//! The editor's standard card policy, composed above out-of-flow box placement.

use super::style::Styles;
use super::{before, hover::block_hover};
use crate::display::{Layout, floating};
use kurbo::{Point, Rect, Size, Stroke};
use measured::Extent;
use puri_widgets::panel::Panel;
use std::rc::Rc;
use uig::Placement;

pub fn popover<World: 'static, Hover: Clone + PartialEq + 'static>(
    trigger: Layout<World, Hover>,
    content: Layout<World, Hover>,
) -> Layout<World, Hover> {
    floating(trigger, card(content), |scale, anchor, extent, bounds| {
        position(anchor, extent, bounds, 4.0 * scale)
    })
}

fn card<World: 'static, Hover: Clone + PartialEq + 'static>(
    content: Layout<World, Hover>,
) -> Layout<World, Hover> {
    block_hover(before(
        crate::display::padding((10.0, 10.0, 10.0, 10.0).into(), content),
        Rc::new(|context| {
            let panel = card_panel(context.toolkit.styles());
            Box::new(move |output, placement| {
                if !placement.clipped_out() {
                    output.render(move |canvas, _| panel.place(canvas, placement));
                }
            })
        }),
    ))
}

/// A card's ground: the palette's panel, bordered, its corners rounded.
pub fn card_panel(styles: &Styles) -> Panel {
    Panel {
        fill: Some(styles.palette.panel.into()),
        border: Some((card_border(styles.scale), styles.palette.border.into())),
        radius: 6.0 * styles.scale,
    }
}

pub fn card_border(scale: f64) -> Stroke {
    Stroke::new(scale)
}

/// A card of `extent` against `anchor`, within `bounds`; none while the
/// anchor is clipped out.
pub fn position(anchor: Placement, extent: Extent, bounds: Rect, gap: f64) -> Option<Placement> {
    (!anchor.clipped_out())
        .then(|| Placement::new(rect(anchor.rect, extent.size(), bounds, gap), bounds))
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
#[path = "tests/popover.rs"]
mod tests;
