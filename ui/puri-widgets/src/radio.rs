//! A radio indicator; callers own labels, grouping, selection and handlers.
use puri::{Affine, Canvas, Circle, Color, Rect, Stroke};

pub const SIZE: f64 = 20.0;

#[derive(Clone, Copy)]
pub struct Radio {
    pub selected: bool,
}

impl Radio {
    pub fn draw(self, canvas: &mut dyn puri::draw::CanvasSink, rect: Rect, scale: f64) {
        let center = rect.center();
        let accent = Color::from_rgb8(48, 126, 210);
        canvas.stroke(
            Circle::new(center, 6.0 * scale),
            Stroke::new(1.5 * scale),
            if self.selected {
                accent
            } else {
                Color::from_rgb8(130, 142, 156)
            },
            Affine::IDENTITY,
        );
        if self.selected {
            canvas.fill(Circle::new(center, 3.5 * scale), accent, Affine::IDENTITY);
        }
    }
}
