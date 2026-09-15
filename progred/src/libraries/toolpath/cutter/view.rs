use super::*;
use crate::display::{
    Layout, ProjectionInput, RowAlignment, activatable, aligned_row, structure, widget,
};
use puri::draw::Canvas;
use puri::{Affine, BezPath, Color, Stroke};
use std::rc::Rc;

/// A picture alongside, not instead of, the editable profile data.
pub(crate) fn display(
    input: &ProjectionInput<'_, crate::Editor, crate::frame::Hovered>,
) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    let tool = Tool::read(input.value?)?;
    let fields = structure::record_layout(input, |_| None)?;
    let target = input.targets.current();
    let picture = activatable(picture(&tool)?, target.hover, target.select);
    Some(aligned_row(
        RowAlignment::Top { baseline: 1 },
        12.0,
        [picture, fields],
    ))
}

pub(super) fn picture(tool: &Tool) -> Option<Layout<crate::Editor, crate::frame::Hovered>> {
    let mut radius: f64 = 0.0;
    let mut low = f64::INFINITY;
    let mut high = f64::NEG_INFINITY;
    for section in &tool.sections {
        let bounds = section.bounds()?;
        radius = radius.max(bounds.radius);
        low = low.min(bounds.min_axial);
        high = high.max(bounds.max_axial);
    }
    let width = 110.0;
    let height = 180.0;
    let margin = 8.0;
    let fit = ((width - 2.0 * margin) / (2.0 * radius)).min((height - 2.0 * margin) / (high - low));
    // This picture's quality is in logical display units, not tool units.
    let tolerance = 0.25 / fit;
    let mut paths = Vec::new();
    for section in &tool.sections {
        let points = section.outline(tolerance)?;
        let mut path = BezPath::new();
        for (i, p) in points.iter().enumerate() {
            let q = (-p.radius, -p.axial);
            if i == 0 {
                path.move_to(q);
            } else {
                path.line_to(q);
            }
        }
        for p in points.iter().rev() {
            path.line_to((p.radius, -p.axial));
        }
        path.close_path();
        paths.push((path, section.kind));
    }
    let transform = Affine::translate((width / 2.0, height / 2.0))
        * Affine::scale(fit)
        * Affine::translate((0.0, (low + high) / 2.0));
    for (p, _) in &mut paths {
        p.apply_affine(transform);
    }
    Some(Layout::program(Rc::new(move |context, _| {
        let scale = context.inputs.styles.scale;
        let paths = paths.clone();
        measured::choices::ChoiceLayout::fixed(widget::paint(
            widget::Extent {
                width: width * scale,
                ascent: height * scale / 2.0,
                descent: height * scale / 2.0,
            },
            move |canvas, placement| {
                let transform = Affine::translate((placement.rect.x0, placement.rect.y0))
                    * Affine::scale(scale);
                for (path, kind) in paths {
                    let color = match kind {
                        SectionKind::Cutting => Color::new([0.88, 0.37, 0.23, 1.0]),
                        SectionKind::NonCutting => Color::new([0.48, 0.54, 0.60, 1.0]),
                    };
                    canvas.fill(path.clone(), color, transform);
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
        ))
    })))
}
