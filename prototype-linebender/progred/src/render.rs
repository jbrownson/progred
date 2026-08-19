//! Lower display leaves to measured Puri drawing and interaction.

use measured::{self, Measured};
use crate::styles::Styles;
pub use progred_display::LineEdit;
use puri::draw::Canvas;
use puri::edit::{EditCtx, LineEditDescription, LineEditState};
use puri::handler::HasHandler;
use puri::text::{TextCtx, TextStyle};

pub fn text<P: Canvas>(ctx: &mut TextCtx, s: &str, style: &TextStyle) -> Measured<P> {
    let text = puri::text::text(ctx, s, style);
    measured::leaf(text.metrics().into(), move |canvas, placement| {
        text.place(canvas, placement)
    })
}

pub fn text_edit<C: 'static, P: Canvas + HasHandler<C>>(
    description: LineEditDescription<'_>,
    tcx: &mut TextCtx,
    with: impl for<'a> Fn(&'a mut C) -> Option<EditCtx<'a>> + Clone + 'static,
) -> Measured<P> {
    let edit = puri::edit::text_edit(description, tcx);
    measured::leaf(edit.metrics().into(), move |p, placement| {
        edit.place(p, placement, with)
    })
}

pub fn line_edit<C: 'static, P: Canvas + HasHandler<C>>(
    tcx: &mut TextCtx,
    styles: &Styles,
    line: &LineEdit,
    editing: Option<&LineEditState>,
    edit: impl for<'a> Fn(&'a mut C) -> Option<EditCtx<'a>> + Clone + 'static,
) -> Measured<P> {
    match editing {
        Some(state) => text_edit(
            LineEditDescription {
                state,
                focused: true,
                presentation: styles.line_presentation(&line.prefix, &line.suffix),
                style: &styles.edit,
                placeholder: None,
            },
            tcx,
            edit,
        ),
        None => text(
            tcx,
            &format!("{}{}{}", line.prefix, line.text, line.suffix),
            &styles.string,
        ),
    }
}
