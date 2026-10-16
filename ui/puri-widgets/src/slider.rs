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

#[derive(Clone, Debug, PartialEq)]
pub struct TickLevel {
    /// Ordered boundary values for one complete level of detail.
    pub values: Vec<f64>,
    /// Full height in logical points.
    pub height: f64,
    /// Stroke width in logical points.
    pub width: f64,
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
        self.draw_with_ticks(canvas, rect, scale, &[]);
    }

    /// Omit an entire level if any of its visible boundaries are too close.
    /// Ticks never affect input, and no individual boundaries are thinned.
    pub fn draw_with_ticks(
        self,
        canvas: &mut dyn puri::draw::CanvasSink,
        rect: Rect,
        scale: f64,
        ticks: &[TickLevel],
    ) {
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
        for level in ticks.iter().filter(|level| {
            level.height.is_finite()
                && level.height > 0.0
                && level.width.is_finite()
                && level.width > 0.0
                && end > start
                && scale > 0.0
        }) {
            let visible = level
                .values
                .iter()
                .copied()
                .filter(|value| (self.min..=self.max).contains(value));
            let spaced = visible
                .clone()
                .zip(visible.clone().skip(1))
                .all(|(a, b)| (b - a) * (end - start) / (self.max - self.min) >= 8.0 * scale);
            if spaced {
                let half = (level.height * scale).min(rect.height()) / 2.0;
                for value in visible {
                    let x = start + (end - start) * (value - self.min) / (self.max - self.min);
                    canvas.stroke(
                        Line::new((x, y - half), (x, y + half)),
                        Stroke::new(level.width * scale),
                        Color::from_rgb8(92, 112, 138),
                        Affine::IDENTITY,
                    );
                }
            }
        }
        canvas.fill(
            Circle::new((x, y), RADIUS * scale),
            accent,
            Affine::IDENTITY,
        );
    }
}

#[cfg(test)]
mod tests;
