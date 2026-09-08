use super::{Widget, fill_height, leaf, selectable_widget};
use crate::{Delim, Layout};
use measured::Extent;
use puri::{Affine, delim};
use std::rc::Rc;

pub fn side<World: 'static, Hover: 'static>(
    delim: Delim,
    side: delim::Side,
) -> Widget<World, Hover> {
    Rc::new(move |context| {
        #[cfg(feature = "profile")]
        let _profile = crate::profile::enter(crate::profile::Kind::Delimiter);
        let size = 14.0 * context.styles.scale;
        let gap = 2.0 * context.styles.scale;
        let brush = context.styles.dim.brush.clone();
        let (ascent, descent) = delim::minimum_span(size);
        leaf(
            Extent {
                width: delim::advance(delim, size) + gap,
                ascent,
                descent,
            },
            move |output, placement| {
                output.render(move |canvas, _| {
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
                });
            },
        )
    })
}

/// Only ink and geometry. Use `selectable_bracket` for an editor handle.
pub fn bracket<World: 'static, Hover: 'static>(
    delim: Delim,
    child: Layout<World, Hover>,
) -> Layout<World, Hover> {
    crate::row(
        0.0,
        [
            Layout::widget(fill_height(side(delim, delim::Side::Open))),
            child,
            Layout::widget(fill_height(side(delim, delim::Side::Close))),
        ],
    )
}

pub fn selectable_bracket<World: 'static, Hover: Clone + PartialEq + 'static>(
    delim: Delim,
    child: Layout<World, Hover>,
) -> Layout<World, Hover> {
    crate::row(
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
