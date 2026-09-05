//! Lower display leaves to measured Puri drawing and interaction.

use crate::placed::{self, Placed, metrics_extent};
use crate::styles::Styles;
use measured::Measured;
use puri::draw::Canvas;
use puri::edit::{EditCtx, LineEditDescription, LineEditState};
use puri::text::{TextCtx, TextStyle};

pub fn text<C: 'static, Cv: Canvas + 'static>(
    ctx: &mut TextCtx,
    s: &str,
    style: &TextStyle,
) -> Measured<Placed<C, Cv>> {
    shaped_text(puri::text::text(ctx, s, style))
}

pub fn shaped_text<C: 'static, Cv: Canvas + 'static>(text: puri::Text) -> Measured<Placed<C, Cv>> {
    placed::leaf(metrics_extent(text.metrics()), move |canvas, placement| {
        text.place(canvas, placement)
    })
}

pub fn text_edit<C: 'static, Cv: Canvas + 'static>(
    description: LineEditDescription<'_>,
    tcx: &mut TextCtx,
    with: impl for<'a> Fn(&'a mut C) -> Option<EditCtx<'a>> + Clone + 'static,
) -> Measured<Placed<C, Cv>> {
    let edit = puri::edit::text_edit(description, tcx);
    placed::leaf(metrics_extent(edit.metrics()), move |p, placement| {
        edit.place(p, placement, with)
    })
}

pub fn line_edit<C: 'static, Cv: Canvas + 'static>(
    tcx: &mut TextCtx,
    styles: &Styles,
    line: &progred_display::LineEdit,
    editing: Option<&LineEditState>,
    edit: impl for<'a> Fn(&'a mut C) -> Option<EditCtx<'a>> + Clone + 'static,
) -> Measured<Placed<C, Cv>> {
    let style = styles.line_style(line);
    let placeholder_style = TextStyle {
        family: style.family,
        ..styles.dim.clone()
    };
    match editing {
        Some(state) => text_edit(
            LineEditDescription {
                state,
                focused: true,
                presentation: styles.line_presentation(line),
                style: &styles.edit,
                placeholder: line
                    .placeholder
                    .as_deref()
                    .map(|placeholder| (placeholder, &placeholder_style)),
            },
            tcx,
            edit,
        ),
        None => match line.placeholder.as_deref().filter(|_| line.text.is_empty()) {
            Some(placeholder) => text(
                tcx,
                &format!("{}{}{}", line.prefix, placeholder, line.suffix),
                &placeholder_style,
            ),
            None => text(
                tcx,
                &format!("{}{}{}", line.prefix, line.text, line.suffix),
                &style,
            ),
        },
    }
}
