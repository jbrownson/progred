//! Lower display leaves to measured Puri drawing and interaction.

use crate::placed::{self, Placed, metrics_extent};
use measured::Measured;
use puri::draw::Canvas;
use puri::edit::{EditCtx, LineEditDescription};
use puri::text::{TextCtx, TextStyle};

pub fn text<C: 'static, Cv: Canvas + 'static>(
    ctx: &mut TextCtx,
    s: &str,
    style: &TextStyle,
) -> Measured<Placed<C, Cv>> {
    let text = puri::text::text(ctx, s, style);
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
