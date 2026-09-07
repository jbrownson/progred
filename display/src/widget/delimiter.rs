use super::{MeasuredSide, Side, leaf, selectable_side};
use crate::{Delim, Layout};
use measured::{Extent, pad};
use peniko::kurbo::Insets;
use puri::Affine;
use puri::delim;
use std::rc::Rc;

pub fn side<World: 'static, Hover: 'static>(delim: Delim, side: delim::Side) -> Side<World, Hover> {
    Rc::new(move |context| {
        let size = 14.0 * context.styles.scale;
        let gap = 2.0 * context.styles.scale;
        let brush = context.styles.dim.brush.clone();
        MeasuredSide {
            maximum_width: delim::maximum_advance(delim, size) + gap,
            measure: Box::new(move |span| {
                let drawing = delim::stretched(delim, side, size, span.ascent, span.descent, brush);
                let extent = Extent {
                    width: drawing.width,
                    ascent: drawing.ascent,
                    descent: drawing.descent,
                };
                let ink = leaf(extent, move |output, placement| {
                    output.render(move |canvas, _| {
                        puri::draw::draw(
                            drawing,
                            canvas,
                            Affine::translate((placement.rect.x0, placement.rect.y0)),
                            Clone::clone,
                        )
                    });
                });
                pad(
                    match side {
                        delim::Side::Open => Insets::new(0.0, 0.0, gap, 0.0),
                        delim::Side::Close => Insets::new(gap, 0.0, 0.0, 0.0),
                    },
                    ink,
                )
            }),
        }
    })
}

/// Only ink and geometry. Use `selectable_bracket` for an editor handle.
pub fn bracket<World: 'static, Hover: 'static>(
    delim: Delim,
    child: Layout<World, Hover>,
) -> Layout<World, Hover> {
    crate::surround(
        side(delim, delim::Side::Open),
        child,
        side(delim, delim::Side::Close),
    )
}

pub fn selectable_bracket<World: 'static, Hover: Clone + 'static>(
    delim: Delim,
    child: Layout<World, Hover>,
) -> Layout<World, Hover> {
    crate::surround(
        selectable_side(side(delim, delim::Side::Open)),
        child,
        selectable_side(side(delim, delim::Side::Close)),
    )
}
