//! Paint-only list separator presentations. List interaction is composed outside.

use super::{Extent, leaf};
use crate::display::Layout;
use kurbo::{Affine, Stroke};
use std::rc::Rc;

/// Reserve spacing and draw an insertion line only for the supplied hover.
/// The list combinator supplies width expansion and interaction separately.
pub fn insertion_gap<W: 'static, H: Clone + PartialEq + 'static>(
    height: f64,
    target: Option<H>,
) -> Layout<W, H> {
    Layout::widget(Rc::new(move |context| {
        let scale = context.toolkit.styles().scale;
        let brush = context.toolkit.styles().selection_wash.clone();
        let target = target.clone();
        leaf(
            Extent {
                width: 0.0,
                ascent: 0.0,
                descent: height * scale,
            },
            move |output, placement| {
                if !placement.clipped_out()
                    && let Some(hover) = target
                {
                    output.render(move |canvas, resolved| {
                        if resolved.hovered.as_ref() == Some(&hover) {
                            let rect = placement.visible_rect();
                            let y = placement.rect.center().y;
                            if y >= rect.y0 && y <= rect.y1 {
                                canvas.stroke_shape(
                                    kurbo::Line::new((rect.x0, y), (rect.x1, y)).into(),
                                    Stroke::new(scale),
                                    brush,
                                    Affine::IDENTITY,
                                );
                            }
                        }
                    });
                }
            },
        )
    }))
}

#[cfg(test)]
mod tests;
