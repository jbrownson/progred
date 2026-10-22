use super::*;
use crate::display::host::{Host, Layout, ProjectionInput};
use crate::display::{RowAlignment, aligned_row, selectable, structure, widget};
use kurbo::{Affine, BezPath, PathEl, Stroke};
use peniko::Color;
use puri::draw::Canvas;
use std::rc::Rc;

/// The color a section of a cutter shows in, wherever the cutter is drawn.
pub(crate) fn color(kind: SectionKind) -> [u8; 3] {
    match kind {
        SectionKind::Cutting => [225, 94, 58],
        SectionKind::NonCutting => [122, 138, 153],
    }
}

/// A picture alongside, not instead of, the editable profile data.
pub(crate) fn display<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
) -> Option<Layout<W>> {
    let tool = Tool::read(input.value?.as_value())?;
    let fields = structure::Fields::default()
        .shown_empty()
        .rest(input, |key| key == ::grap::isa::ISA)?;
    let site = input.site();
    let picture = selectable(picture(&tool)?, &site);
    Some(aligned_row(
        RowAlignment::Top { baseline: 1 },
        12.0,
        [picture, fields],
    ))
}

pub(super) fn picture<W: Host>(tool: &Tool) -> Option<Layout<W>> {
    let (radius, low, high) = tool.sections().iter().try_fold(
        (0.0_f64, f64::INFINITY, f64::NEG_INFINITY),
        |(radius, low, high), section| {
            let bounds = section.bounds()?;
            Some((
                radius.max(bounds.radius),
                low.min(bounds.min_axial),
                high.max(bounds.max_axial),
            ))
        },
    )?;
    let width = 110.0;
    let height = 180.0;
    let margin = 8.0;
    let fit = ((width - 2.0 * margin) / (2.0 * radius)).min((height - 2.0 * margin) / (high - low));
    let transform = Affine::translate((width / 2.0, height / 2.0))
        * Affine::scale(fit)
        * Affine::translate((0.0, (low + high) / 2.0));
    // This picture's quality is in logical display units, not tool units.
    let tolerance = 0.25 / fit;
    let paths = tool
        .sections()
        .iter()
        .map(|section| {
            let points = section.outline(tolerance)?;
            let (first, rest) = points.split_first()?;
            // Up the profile on one side of the axis, and back down its mirror image.
            let left = |p: &Point| transform * kurbo::Point::new(-p.radius, -p.axial);
            let right = |p: &Point| transform * kurbo::Point::new(p.radius, -p.axial);
            let path = std::iter::once(PathEl::MoveTo(left(first)))
                .chain(rest.iter().map(|p| PathEl::LineTo(left(p))))
                .chain(points.iter().rev().map(|p| PathEl::LineTo(right(p))))
                .chain([PathEl::ClosePath])
                .collect::<BezPath>();
            Some((path, section.kind))
        })
        .collect::<Option<Vec<_>>>()?;
    Some(Layout::widget(Rc::new(move |context| {
        let scale = context.viewing().scale();
        let paths = paths.clone();
        widget::paint(
            widget::Extent {
                width: width * scale,
                ascent: height * scale / 2.0,
                descent: height * scale / 2.0,
            },
            move |canvas, placement| {
                let transform = Affine::translate((placement.rect.x0, placement.rect.y0))
                    * Affine::scale(scale);
                for (path, kind) in paths {
                    let [r, g, b] = color(kind);
                    canvas.fill(path.clone(), Color::from_rgb8(r, g, b), transform);
                    canvas.stroke(
                        path,
                        Stroke::new(0.75),
                        Color::new([0.25, 0.28, 0.32, 1.0]),
                        transform,
                    );
                }
                let mut center = BezPath::new();
                center.move_to((width / 2.0, margin / 2.0));
                center.line_to((width / 2.0, height - margin / 2.0));
                canvas.stroke(
                    center,
                    Stroke::new(0.5),
                    Color::new([0.25, 0.28, 0.32, 0.6]),
                    transform,
                );
            },
        )
    })))
}
