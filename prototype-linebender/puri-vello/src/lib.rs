//! The vello backend: a `Canvas` streaming into a `vello::Scene`.

use puri::draw::{Canvas, GlyphRun, Shape};
use vello::Scene;
use vello::kurbo::{Affine, Stroke};
use vello::peniko::{Brush, Fill};

pub struct VelloCanvas<'a>(pub &'a mut Scene);

impl Canvas for VelloCanvas<'_> {
    fn fill(&mut self, shape: impl Into<Shape>, brush: impl Into<Brush>, transform: Affine) {
        let brush = brush.into();
        match shape.into() {
            Shape::Rect(s) => self.0.fill(Fill::NonZero, transform, &brush, None, &s),
            Shape::RoundedRect(s) => self.0.fill(Fill::NonZero, transform, &brush, None, &s),
            Shape::Circle(s) => self.0.fill(Fill::NonZero, transform, &brush, None, &s),
            Shape::Line(s) => self.0.fill(Fill::NonZero, transform, &brush, None, &s),
            Shape::Path(s) => self.0.fill(Fill::NonZero, transform, &brush, None, &s),
        }
    }

    fn stroke(
        &mut self,
        shape: impl Into<Shape>,
        style: Stroke,
        brush: impl Into<Brush>,
        transform: Affine,
    ) {
        let brush = brush.into();
        match shape.into() {
            Shape::Rect(s) => self.0.stroke(&style, transform, &brush, None, &s),
            Shape::RoundedRect(s) => self.0.stroke(&style, transform, &brush, None, &s),
            Shape::Circle(s) => self.0.stroke(&style, transform, &brush, None, &s),
            Shape::Line(s) => self.0.stroke(&style, transform, &brush, None, &s),
            Shape::Path(s) => self.0.stroke(&style, transform, &brush, None, &s),
        }
    }

    fn glyph_run(&mut self, run: GlyphRun) {
        self.0
            .draw_glyphs(&run.font)
            .font_size(run.size)
            .brush(&run.brush)
            .hint(run.hint)
            .transform(run.transform)
            .glyph_transform(run.glyph_transform)
            .normalized_coords(&run.normalized_coords)
            .draw(
                Fill::NonZero,
                run.glyphs.iter().map(|glyph| vello::Glyph {
                    id: glyph.id,
                    x: glyph.x,
                    y: glyph.y,
                }),
            );
    }

    fn clip(&mut self, shape: impl Into<Shape>, transform: Affine, content: impl FnOnce(&mut Self)) {
        self.push_clip(&shape.into(), transform);
        content(self);
        self.pop_clip();
    }
}

impl VelloCanvas<'_> {
    /// Raw clip bracket, for wrapper canvases that interleave their own
    /// state around `Canvas::clip`.
    pub fn push_clip(&mut self, shape: &Shape, transform: Affine) {
        match shape {
            Shape::Rect(s) => self.0.push_clip_layer(Fill::NonZero, transform, s),
            Shape::RoundedRect(s) => self.0.push_clip_layer(Fill::NonZero, transform, s),
            Shape::Circle(s) => self.0.push_clip_layer(Fill::NonZero, transform, s),
            Shape::Line(s) => self.0.push_clip_layer(Fill::NonZero, transform, s),
            Shape::Path(s) => self.0.push_clip_layer(Fill::NonZero, transform, s),
        }
    }

    pub fn pop_clip(&mut self) {
        self.0.pop_layer();
    }
}
