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
mod tests {
    use super::*;
    use puri::draw::{DrawCmd, DrawList};

    #[test]
    fn item_hits_and_feedback_use_the_same_notches_at_every_density() {
        for scale in [1.0, 2.0] {
            for count in [1, 4, 500] {
                let slider = RangeSlider::new(count, 0..count).unwrap();
                let rect = Rect::new(10.0, 30.0, 410.0, 30.0 + HEIGHT * scale);
                for item in 0..count {
                    let bounds = slider.item_rect(rect, scale, item).unwrap();
                    assert_eq!(slider.item_at(rect, scale, bounds.center()), Some(item));
                    assert!(rect.contains(bounds.center()));
                }
                assert_eq!(slider.item_rect(rect, scale, count), None);
                assert_eq!(slider.item_at(rect, scale, Point::new(rect.x0, 35.0)), None);
                assert_eq!(
                    slider.item_at(rect, scale, Point::new(200.0, rect.y1 + 1.0)),
                    None
                );
                let empty = Rect::new(0.0, 0.0, 0.0, HEIGHT * scale);
                assert_eq!(slider.item_at(empty, scale, Point::ZERO), None);
                assert_eq!(slider.item_rect(empty, scale, 0), None);
            }
        }
    }

    #[test]
    fn separators_fit_the_notches_and_use_physical_pixel_visibility() {
        let slider = RangeSlider::new(10, 2..8).unwrap();
        for scale in [1.0, 2.0, 3.0] {
            for spacing_px in [0.0, 1.5, 1.99, 2.0, 3.0, 8.0, 40.0] {
                let rect = Rect::new(
                    0.25,
                    0.0,
                    0.25 + 2.0 * HANDLE * scale + 10.0 * spacing_px,
                    HEIGHT * scale,
                );
                let mut list = DrawList::new();
                slider.draw(&mut list, rect, scale, None);
                let strokes: Vec<_> = list
                    .0
                    .iter()
                    .filter_map(|cmd| match cmd {
                        DrawCmd::Stroke { style, .. } => Some(style.width),
                        _ => None,
                    })
                    .collect();
                if spacing_px < 2.0 {
                    assert!(strokes.is_empty());
                } else {
                    assert_eq!(strokes.len(), slider.count + 1);
                    for width in strokes {
                        assert!((width - scale.min(spacing_px * 0.25)).abs() < 1e-12);
                    }
                }
                assert_eq!(
                    list.0
                        .iter()
                        .filter(|cmd| matches!(cmd, DrawCmd::Fill { .. }))
                        .count(),
                    4
                );
            }
        }
    }

    #[test]
    fn current_marker_uses_item_geometry_and_stays_visible_in_dense_rows() {
        for scale in [1.0, 2.0] {
            for count in [4, 500] {
                let slider = RangeSlider::new(count, 0..count).unwrap();
                let rect = Rect::new(10.0, 0.0, 110.0 * scale, HEIGHT * scale);
                let rail = RangeSlider::rail(rect, scale);
                let notch = rail.width() / count as f64;
                for current in [0, count / 2, count - 1] {
                    let mut drawing = DrawList::new();
                    slider.draw(&mut drawing, rect, scale, Some(current));
                    let Some(DrawCmd::Fill {
                        shape: puri::Shape::Rect(marker),
                        ..
                    }) = drawing.0.last()
                    else {
                        panic!("current item marker");
                    };
                    let center = rail.x0 + (current as f64 + 0.5) * notch;
                    assert!(marker.x0 <= center && marker.x1 >= center);
                    assert!(marker.x0 >= rail.x0 && marker.x1 <= rail.x1);
                    assert_eq!(marker.width(), notch.max(3.0 * scale));
                    assert_eq!(marker.height(), 2.0 * scale);
                    assert!(marker.y1 <= rect.y1);
                }
                let mut plain = DrawList::new();
                let mut invalid = DrawList::new();
                slider.draw(&mut plain, rect, scale, None);
                slider.draw(&mut invalid, rect, scale, Some(count));
                assert_eq!(plain.0.len(), invalid.0.len());
            }
        }
    }

    #[test]
    fn hidden_separators_keep_individual_items_selectable() {
        let slider = RangeSlider::new(10, 0..10).unwrap();
        let rect = Rect::new(0.0, 0.0, 35.0, 48.0);
        let point = Point::new(15.25, 24.0);
        let drag = slider.begin(rect, 2.0, point);
        assert_eq!(slider.dragged(drag, rect, 2.0, point), 3..4);
        assert_eq!(
            slider.dragged(drag, rect, 2.0, Point::new(21.25, 24.0)),
            3..8
        );
    }

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
