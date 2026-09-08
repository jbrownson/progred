//! Prepare Puri drawing descriptions as ordinary widget continuations.

use crate::display::Paint;
use crate::display::widget::HoverPass;
use crate::display::widget::{self, Context};
use measured::{Extent, Measured};
use puri::text::TextStyle;
use puri::{Affine, Leaf};

pub fn prepare<W: 'static, H: 'static>(
    context: &mut Context<'_, '_, W, H>,
    leaf: &Leaf<Paint>,
) -> Measured<HoverPass<W, H>> {
    #[cfg(all(test, feature = "layout-profile"))]
    let _profile = crate::display::profile::enter(match leaf {
        Leaf::Text { .. } => crate::display::profile::Kind::Text,
        Leaf::Drawing(_) => crate::display::profile::Kind::Drawing,
    });
    match leaf {
        Leaf::Text {
            text,
            paint,
            script,
        } => {
            let style = match paint {
                Paint::Face(face) => {
                    widget::style::face_style(context.inputs.styles, *face).clone()
                }
                Paint::Brush(brush) => TextStyle {
                    brush: brush.clone(),
                    ..context.inputs.styles.name.clone()
                },
            };
            let text = puri::text::scripted_text(context.text, text, &style, *script);
            widget::leaf(widget::extent(text.metrics()), move |output, placement| {
                output.render(move |canvas, _| text.place(canvas, placement))
            })
        }
        Leaf::Drawing(drawing) => {
            let scale = context.inputs.styles.scale;
            let drawing = drawing.clone().map_paint(|paint| match paint {
                Paint::Face(face) => widget::style::face_style(context.inputs.styles, face)
                    .brush
                    .clone(),
                Paint::Brush(brush) => brush,
            });
            widget::leaf(
                Extent {
                    width: drawing.width * scale,
                    ascent: drawing.ascent * scale,
                    descent: drawing.descent * scale,
                },
                move |output, placement| {
                    output.render(move |canvas, _| {
                        let transform = Affine::translate((placement.rect.x0, placement.rect.y0))
                            * Affine::scale(scale);
                        puri::draw::draw(drawing, canvas, transform, Clone::clone);
                    })
                },
            )
        }
    }
}
