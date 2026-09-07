//! The vello backend: a `Canvas` streaming into a `vello::Scene`.

use puri::draw::{GlyphRun, Shape};
use vello::Scene;
use vello::kurbo::{Affine, Stroke};
use vello::peniko::{Brush, Fill, ImageData};

pub struct VelloCanvas<'a>(pub &'a mut Scene);

impl puri::draw::CanvasSink for VelloCanvas<'_> {
    fn draw_image(&mut self, image: ImageData, transform: Affine) {
        self.0.draw_image(&image, transform);
    }

    fn fill_shape(&mut self, shape: Shape, brush: Brush, transform: Affine) {
        match shape {
            Shape::Rect(s) => self.0.fill(Fill::NonZero, transform, &brush, None, &s),
            Shape::RoundedRect(s) => self.0.fill(Fill::NonZero, transform, &brush, None, &s),
            Shape::Circle(s) => self.0.fill(Fill::NonZero, transform, &brush, None, &s),
            Shape::Line(s) => self.0.fill(Fill::NonZero, transform, &brush, None, &s),
            Shape::Path(s) => self.0.fill(Fill::NonZero, transform, &brush, None, &s),
        }
    }

    fn stroke_shape(&mut self, shape: Shape, style: Stroke, brush: Brush, transform: Affine) {
        match shape {
            Shape::Rect(s) => self.0.stroke(&style, transform, &brush, None, &s),
            Shape::RoundedRect(s) => self.0.stroke(&style, transform, &brush, None, &s),
            Shape::Circle(s) => self.0.stroke(&style, transform, &brush, None, &s),
            Shape::Line(s) => self.0.stroke(&style, transform, &brush, None, &s),
            Shape::Path(s) => self.0.stroke(&style, transform, &brush, None, &s),
        }
    }

    fn draw_glyphs(&mut self, run: GlyphRun) {
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

    fn with_clip(
        &mut self,
        shape: Shape,
        transform: Affine,
        content: Box<dyn FnOnce(&mut dyn puri::draw::CanvasSink) + '_>,
    ) {
        self.push_clip(&shape, transform);
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
