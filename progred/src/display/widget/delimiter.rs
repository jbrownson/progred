use super::{Widget, fill_height, paint, selectable_widget};
use crate::display::{Delim, Layout};
use measured::Extent;
use puri::{Affine, delim};
use std::rc::Rc;

pub fn side<World: 'static, Hover: 'static>(
    delim: Delim,
    side: delim::Side,
) -> Widget<World, Hover> {
    Rc::new(move |context| {
        #[cfg(all(test, feature = "layout-profile"))]
        let _profile = crate::display::profile::enter(crate::display::profile::Kind::Delimiter);
        let size = 14.0 * context.inputs.styles.scale;
        let gap = 2.0 * context.inputs.styles.scale;
        let brush = context.inputs.styles.dim.brush.clone();
        let (ascent, descent) = delim::minimum_span(size);
        paint(
            Extent {
                width: delim::advance(delim, size) + gap,
                ascent,
                descent,
            },
            move |canvas, placement| {
                let inset = match side {
                    delim::Side::Open => 0.0,
                    delim::Side::Close => gap,
                };
                delim::draw_stretched(
                    delim,
                    side,
                    size,
                    ascent,
                    placement.rect.height() - ascent,
                    brush,
                    canvas,
                    Affine::translate((placement.rect.x0 + inset, placement.rect.y0)),
                )
            },
        )
    })
}

/// Only ink and geometry. Use `selectable_bracket` for an editor handle.
pub fn bracket<World: 'static, Hover: 'static>(
    delim: Delim,
    child: Layout<World, Hover>,
) -> Layout<World, Hover> {
    crate::display::row(
        0.0,
        [
            Layout::widget(fill_height(side(delim, delim::Side::Open))),
            child,
            Layout::widget(fill_height(side(delim, delim::Side::Close))),
        ],
    )
}

pub fn selectable_bracket(
    delim: Delim,
    child: Layout<crate::Editor, crate::frame::Hovered>,
) -> Layout<crate::Editor, crate::frame::Hovered> {
    crate::display::row(
        0.0,
        [
            Layout::widget(fill_height(selectable_widget(side(
                delim,
                delim::Side::Open,
            )))),
            child,
            Layout::widget(fill_height(selectable_widget(side(
                delim,
                delim::Side::Close,
            )))),
        ],
    )
}
