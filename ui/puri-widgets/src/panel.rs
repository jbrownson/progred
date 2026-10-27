use puri::draw::CanvasSink;
use puri::{Affine, Brush, Placement, RoundedRect, Shape, Stroke};

pub struct Panel {
    pub fill: Option<Brush>,
    pub border: Option<(Stroke, Brush)>,
    pub radius: f64,
}

impl Panel {
    pub fn place(&self, canvas: &mut (impl CanvasSink + ?Sized), placement: Placement) {
        let shape = if self.radius == 0.0 {
            Shape::Rect(placement.rect)
        } else {
            Shape::RoundedRect(RoundedRect::from_rect(placement.rect, self.radius))
        };
        if let Some(fill) = &self.fill {
            canvas.fill_shape(shape.clone(), fill.clone(), Affine::IDENTITY);
        }
        if let Some((stroke, brush)) = &self.border {
            canvas.stroke_shape(shape, stroke.clone(), brush.clone(), Affine::IDENTITY);
        }
    }
}

#[cfg(test)]
#[path = "tests/panel.rs"]
mod tests;
