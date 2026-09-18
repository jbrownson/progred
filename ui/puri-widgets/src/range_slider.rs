//! Equal-width discrete choices with an inclusive selection, represented as a
//! half-open range. No identities, retained state, or knowledge of the items.
use puri::{Affine, Canvas, Color, Line, Point, Rect, Stroke};
use std::ops::Range;

pub const HEIGHT: f64 = 24.0;
const HANDLE: f64 = 5.0;

#[derive(Clone, Debug)]
pub struct RangeSlider {
    pub count: usize,
    pub selected: Range<usize>,
}

#[derive(Clone, Copy, Debug)]
pub enum Drag {
    Start { end: usize },
    End { start: usize },
    Span { anchor: usize },
}

impl RangeSlider {
    pub fn new(count: usize, selected: Range<usize>) -> Option<Self> {
        (selected.start < selected.end && selected.end <= count).then_some(Self { count, selected })
    }

    fn rail(rect: Rect, scale: f64) -> Rect {
        let inset = (HANDLE * scale).min(rect.width() / 2.0);
        Rect::new(rect.x0 + inset, rect.y0, rect.x1 - inset, rect.y1)
    }

    fn position(&self, rect: Rect, scale: f64, point: Point) -> f64 {
        let rail = Self::rail(rect, scale);
        if rail.width() <= 0.0 {
            return 0.0;
        }
        ((point.x - rail.x0) / rail.width()).clamp(0.0, 1.0) * self.count as f64
    }

    pub fn begin(&self, rect: Rect, scale: f64, point: Point) -> Drag {
        let rail = Self::rail(rect, scale);
        let x = |i| rail.x0 + rail.width() * i as f64 / self.count as f64;
        // Cap the handle hit area so even densely packed notches have a center.
        let reach = (HANDLE * scale).min(rail.width() / self.count as f64 * 0.25);
        if (point.x - x(self.selected.start)).abs() <= reach {
            Drag::Start {
                end: self.selected.end,
            }
        } else if (point.x - x(self.selected.end)).abs() <= reach {
            Drag::End {
                start: self.selected.start,
            }
        } else {
            Drag::Span {
                anchor: (self.position(rect, scale, point).floor() as usize).min(self.count - 1),
            }
        }
    }

    pub fn dragged(&self, drag: Drag, rect: Rect, scale: f64, point: Point) -> Range<usize> {
        let position = self.position(rect, scale, point);
        match drag {
            Drag::Start { end } => (position.round() as usize).min(end - 1)..end,
            Drag::End { start } => start..(position.round() as usize).clamp(start + 1, self.count),
            Drag::Span { anchor } => {
                let i = (position.floor() as usize).min(self.count - 1);
                anchor.min(i)..anchor.max(i) + 1
            }
        }
    }

    pub fn draw(&self, canvas: &mut dyn puri::draw::CanvasSink, rect: Rect, scale: f64) {
        let rail = Self::rail(rect, scale);
        let x = |i| rail.x0 + rail.width() * i as f64 / self.count as f64;
        let y = rect.center().y;
        let accent = Color::from_rgb8(48, 126, 210);
        canvas.fill(
            Rect::new(rail.x0, y - 4.0 * scale, rail.x1, y + 4.0 * scale),
            Color::from_rgba8(196, 204, 214, 220),
            Affine::IDENTITY,
        );
        canvas.fill(
            Rect::new(
                x(self.selected.start),
                y - 4.0 * scale,
                x(self.selected.end),
                y + 4.0 * scale,
            ),
            accent,
            Affine::IDENTITY,
        );
        for i in 0..=self.count {
            canvas.stroke(
                Line::new((x(i), y - 4.0 * scale), (x(i), y + 4.0 * scale)),
                Stroke::new(scale),
                Color::from_rgba8(255, 255, 255, 200),
                Affine::IDENTITY,
            );
        }
        for i in [self.selected.start, self.selected.end] {
            canvas.fill(
                Rect::new(
                    x(i) - 2.0 * scale,
                    y - 8.0 * scale,
                    x(i) + 2.0 * scale,
                    y + 8.0 * scale,
                ),
                accent,
                Affine::IDENTITY,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn click_drag_and_handles_share_painted_geometry() {
        let slider = RangeSlider::new(4, 0..4).unwrap();
        let rect = Rect::new(0.0, 0.0, 410.0, 24.0);
        let point = Point::new(155.0, 12.0);
        let drag = slider.begin(rect, 1.0, point);
        assert_eq!(slider.dragged(drag, rect, 1.0, point), 1..2);
        assert_eq!(
            slider.dragged(drag, rect, 1.0, Point::new(355.0, -500.0)),
            1..4
        );
        assert_eq!(
            slider.dragged(drag, rect, 1.0, Point::new(-500.0, 0.0)),
            0..2
        );
        let drag = slider.begin(rect, 1.0, Point::new(405.0, 12.0));
        assert_eq!(
            slider.dragged(drag, rect, 1.0, Point::new(205.0, 0.0)),
            0..2
        );
        let drag = slider.begin(rect, 1.0, Point::new(5.0, 12.0));
        assert_eq!(
            slider.dragged(drag, rect, 1.0, Point::new(305.0, 0.0)),
            3..4
        );
        assert!(RangeSlider::new(0, 0..0).is_none());
    }
}
