//! A bounded continuous control. The caller owns its value and gesture capture.
use puri::{Affine, Canvas, Circle, Color, Line, Point, Rect, Stroke};

pub const HEIGHT: f64 = 28.0;
const RADIUS: f64 = 6.0;

#[derive(Clone, Copy, Debug)]
pub struct Slider {
    pub min: f64,
    pub max: f64,
    pub value: f64,
}

impl Slider {
    pub fn new(min: f64, max: f64, value: f64) -> Option<Self> {
        (min.is_finite()
            && max.is_finite()
            && value.is_finite()
            && min < max
            && (max - min).is_finite())
        .then(|| Self {
            min,
            max,
            value: value.clamp(min, max),
        })
    }

    fn rail(rect: Rect, scale: f64) -> (f64, f64) {
        let inset = (RADIUS * scale).min(rect.width() / 2.0);
        (rect.x0 + inset, rect.x1 - inset)
    }

    pub fn value_at(self, rect: Rect, scale: f64, point: Point) -> f64 {
        let (start, end) = Self::rail(rect, scale);
        if end <= start {
            return self.value;
        }
        let t = ((point.x - start) / (end - start)).clamp(0.0, 1.0);
        self.min + t * (self.max - self.min)
    }

    pub fn draw(self, canvas: &mut dyn puri::draw::CanvasSink, rect: Rect, scale: f64) {
        let (start, end) = Self::rail(rect, scale);
        let y = rect.center().y;
        let x = start + (end - start) * (self.value - self.min) / (self.max - self.min);
        let accent = Color::from_rgba8(48, 126, 210, 255);
        canvas.stroke(
            Line::new((start, y), (end, y)),
            Stroke::new(3.0 * scale),
            Color::from_rgba8(196, 204, 214, 255),
            Affine::IDENTITY,
        );
        canvas.stroke(
            Line::new((start, y), (x, y)),
            Stroke::new(3.0 * scale),
            accent,
            Affine::IDENTITY,
        );
        canvas.fill(
            Circle::new((x, y), RADIUS * scale),
            accent,
            Affine::IDENTITY,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mapping_uses_the_painted_rail_and_clamps_unbounded_drags() {
        let s = Slider::new(-2.0, 8.0, 0.0).unwrap();
        let r = Rect::new(100.0, 0.0, 300.0, 56.0);
        assert_eq!(s.value_at(r, 2.0, Point::new(112.0, 20.0)), -2.0);
        assert_eq!(s.value_at(r, 2.0, Point::new(200.0, -500.0)), 3.0);
        assert_eq!(s.value_at(r, 2.0, Point::new(500.0, 20.0)), 8.0);
        assert!(Slider::new(1.0, 1.0, 0.0).is_none());
        assert!(Slider::new(0.0, 1.0, f64::NAN).is_none());
    }
}
