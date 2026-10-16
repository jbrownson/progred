//! Equal-width discrete choices with an inclusive selection, represented as a
//! half-open range. No identities, retained state, or knowledge of the items.
use puri::{Affine, Canvas, Color, Line, Point, Rect, Stroke};
use std::ops::Range;

pub const HEIGHT: f64 = 20.0;
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

    pub fn item_at(&self, rect: Rect, scale: f64, point: Point) -> Option<usize> {
        let rail = Self::rail(rect, scale);
        (self.count > 0 && rail.width() > 0.0 && rail.contains(point))
            .then(|| (self.position(rect, scale, point).floor() as usize).min(self.count - 1))
    }

    pub fn item_rect(&self, rect: Rect, scale: f64, item: usize) -> Option<Rect> {
        let rail = Self::rail(rect, scale);
        (item < self.count && rail.width() > 0.0).then(|| {
            let width = rail.width() / self.count as f64;
            let half_height = (rect.height() / 2.0 - 2.0 * scale).max(0.0);
            Rect::new(
                rail.x0 + width * item as f64,
                rect.center().y - half_height,
                rail.x0 + width * (item + 1) as f64,
                rect.center().y + half_height,
            )
        })
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

    /// The rectangle is in physical pixels; scale converts logical styling units.
    /// `current` marks one item without changing the selected range or hit areas.
    pub fn draw(
        &self,
        canvas: &mut dyn puri::draw::CanvasSink,
        rect: Rect,
        scale: f64,
        current: Option<usize>,
    ) {
        let rail = Self::rail(rect, scale);
        let x = |i| rail.x0 + rail.width() * i as f64 / self.count as f64;
        let y = rect.center().y;
        let half_height = (rect.height() / 2.0 - 2.0 * scale).max(0.0);
        let accent = Color::from_rgb8(140, 183, 224);
        canvas.fill(
            Rect::new(rail.x0, y - half_height, rail.x1, y + half_height),
            Color::from_rgb8(216, 225, 236),
            Affine::IDENTITY,
        );
        canvas.fill(
            Rect::new(
                x(self.selected.start),
                y - half_height,
                x(self.selected.end),
                y + half_height,
            ),
            accent,
            Affine::IDENTITY,
        );
        let notch_width_px = rail.width() / self.count as f64;
        if notch_width_px >= 2.0 {
            for i in 0..=self.count {
                canvas.stroke(
                    Line::new((x(i), y - half_height), (x(i), y + half_height)),
                    Stroke::new(scale.min(notch_width_px * 0.25)),
                    Color::from_rgba8(255, 255, 255, 160),
                    Affine::IDENTITY,
                );
            }
        }
        for i in [self.selected.start, self.selected.end] {
            canvas.fill(
                Rect::new(x(i) - scale, y - half_height, x(i) + scale, y + half_height),
                Color::from_rgb8(28, 85, 150),
                Affine::IDENTITY,
            );
        }
        if let Some(i) = current.filter(|i| *i < self.count && rail.width() > 0.0) {
            let width = notch_width_px.max(3.0 * scale).min(rail.width());
            let left = ((x(i) + x(i + 1) - width) / 2.0).clamp(rail.x0, rail.x1 - width);
            canvas.fill(
                Rect::new(
                    left,
                    y + half_height - (2.0 * scale).min(2.0 * half_height),
                    left + width,
                    y + half_height,
                ),
                Color::from_rgb8(240, 178, 67),
                Affine::IDENTITY,
            );
        }
    }
}

#[cfg(test)]
mod tests;
