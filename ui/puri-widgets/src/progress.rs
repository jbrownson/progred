//! A paint-only progress bar. The caller owns progress and visibility.
use puri::{Affine, Canvas, Color, Rect};

pub struct ProgressBar {
    pub fraction: f64,
    pub track: Color,
    pub fill: Color,
}

impl ProgressBar {
    pub fn fill_rect(&self, rect: Rect) -> Rect {
        let fraction = if self.fraction.is_finite() {
            self.fraction.clamp(0.0, 1.0)
        } else {
            0.0
        };
        Rect::new(rect.x0, rect.y0, rect.x0 + rect.width() * fraction, rect.y1)
    }

    pub fn draw(&self, canvas: &mut dyn puri::draw::CanvasSink, rect: Rect) {
        canvas.fill(rect, self.track, Affine::IDENTITY);
        canvas.fill(self.fill_rect(rect), self.fill, Affine::IDENTITY);
    }
}

#[cfg(test)]
#[path = "tests/progress.rs"]
mod tests;
