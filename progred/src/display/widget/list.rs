//! Paint-only list separator presentations. List interaction is composed outside.

use super::{Extent, leaf};
use crate::display::Layout;
use puri::{Affine, Stroke};
use std::rc::Rc;

/// Reserve spacing and draw an insertion line only for the supplied hover.
/// The list combinator supplies width expansion and interaction separately.
pub fn insertion_gap<W: 'static, H: Clone + PartialEq + 'static>(
    height: f64,
    target: Option<H>,
) -> Layout<W, H> {
    Layout::widget(Rc::new(move |context| {
        let scale = context.inputs.styles.scale;
        let brush = context.inputs.styles.selection_wash.clone();
        let target = target.clone();
        leaf(
            Extent {
                width: 0.0,
                ascent: 0.0,
                descent: height * scale,
            },
            move |output, placement| {
                let Some(hover) = target else {
                    return;
                };
                if placement.clipped_out() {
                    return;
                }
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
            },
        )
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::recording::{Recorded, record};
    use crate::display::widget::{HoverInput, HoverOutput, ResolvedHover};
    use puri::{DrawCmd, DrawList, Placement, Point, Rect, Shape};

    fn gap(active: bool) -> HoverOutput<(), u32> {
        let layout = insertion_gap(16.0, active.then_some(7));
        let Recorded::Widget(widget) = record(&layout) else {
            panic!("native gap")
        };
        let measured = crate::display::test_support::with_context(
            &crate::display::test_support::NoProject,
            |context| widget(context),
        );
        assert_eq!(measured.extent.width, 0.0);
        assert_eq!(measured.extent.height(), 16.0);
        super::super::frame::place(
            measured,
            Placement::new(
                Rect::new(10.0, 20.0, 100.0, 36.0),
                Rect::new(30.0, 0.0, 80.0, 100.0),
            )
            .with_available_rect(Rect::new(10.0, 20.0, 100.0, 36.0)),
            &HoverInput::default(),
        )
    }

    #[test]
    fn insertion_gap_only_paints_feedback_and_respects_the_clip() {
        let output = gap(true);
        assert!(output.handler.is_none());
        assert!(
            output
                .hover_geometry
                .probe(Some(Point::new(50.0, 28.0)), None, 0.0)
                .is_none()
        );
        for hovered in [None, Some(7), Some(8)] {
            let mut drawing = DrawList::new();
            puri::frame::render(
                gap(true)
                    .bind(ResolvedHover {
                        hovered,
                        ..Default::default()
                    })
                    .renders,
                &mut drawing,
            );
            if hovered == Some(7) {
                assert!(
                    matches!(&drawing.0[..], [DrawCmd::Stroke { shape: Shape::Line(line), .. }]
                    if line.p0 == Point::new(30.0, 28.0) && line.p1 == Point::new(80.0, 28.0))
                );
            } else {
                assert!(drawing.0.is_empty());
            }
        }
    }

    #[test]
    fn inactive_insertion_gap_is_only_spacing() {
        let gap = gap(false);
        assert!(
            gap.hover_geometry
                .probe(Some(Point::new(50.0, 28.0)), None, 0.0)
                .is_none()
        );
        assert!(gap.handler.is_none());
        assert!(gap.after_hover.is_empty());
    }
}
