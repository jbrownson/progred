//! Lower display leaves to measured Puri drawing and interaction.

use crate::placed::{self, HoverPass, metrics_extent};
use measured::Measured;
use puri::edit::LineEditDescription;
use puri::text::{TextCtx, TextStyle};

pub fn text<C: 'static>(ctx: &mut TextCtx, s: &str, style: &TextStyle) -> Measured<HoverPass<C>> {
    shaped_text(puri::text::text(ctx, s, style))
}

pub fn shaped_text<C: 'static>(text: puri::Text) -> Measured<HoverPass<C>> {
    placed::leaf(metrics_extent(text.metrics()), move |canvas, placement| {
        text.place(canvas, placement)
    })
}

pub fn text_edit<C: 'static>(
    description: LineEditDescription<'_>,
    tcx: &mut TextCtx,
    with: impl Fn(&mut C, &puri::edit::EditOperation<'_>) -> bool + Clone + 'static,
) -> Measured<HoverPass<C>> {
    let edit = puri::edit::text_edit(description, tcx);
    placed::leaf(metrics_extent(edit.metrics()), move |p, placement| {
        edit.place(p, placement, with)
    })
}
