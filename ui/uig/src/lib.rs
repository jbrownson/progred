//! Shared UI geometry: the vocabulary layout engines and widget
//! libraries exchange without depending on each other.

use kurbo::{Point, Rect};

/// A widget's measured rectangle, available allocation, and enclosing clip.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub rect: Rect,
    pub available_rect: Rect,
    pub clip_rect: Rect,
}

impl Placement {
    pub const fn new(rect: Rect, clip_rect: Rect) -> Self {
        Self {
            rect,
            available_rect: rect,
            clip_rect,
        }
    }

    pub const fn root(rect: Rect) -> Self {
        Self::new(rect, rect)
    }

    pub const fn with_available_rect(self, available_rect: Rect) -> Self {
        Self {
            available_rect,
            ..self
        }
    }

    pub fn fill_height(self) -> Self {
        Self {
            rect: Rect::new(
                self.rect.x0,
                self.available_rect.y0,
                self.rect.x1,
                self.available_rect.y1,
            ),
            ..self
        }
    }

    pub fn visible_rect(self) -> Rect {
        self.rect.intersect(self.clip_rect)
    }

    pub fn clipped_out(self) -> bool {
        let visible = self.visible_rect();
        visible.width() <= 0.0 || visible.height() <= 0.0
    }

    pub fn contains(self, point: Point) -> bool {
        !self.clipped_out() && self.rect.contains(point) && self.clip_rect.contains(point)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visibility_and_hits_use_both_rectangles() {
        let placement = Placement::new(
            Rect::new(20.0, 50.0, 60.0, 90.0),
            Rect::new(40.0, 20.0, 120.0, 80.0),
        );
        assert_eq!(placement.visible_rect(), Rect::new(40.0, 50.0, 60.0, 80.0));
        assert!(!placement.contains(Point::new(30.0, 60.0)));
        assert!(placement.contains(Point::new(50.0, 60.0)));
        let hidden = Placement::new(Rect::new(70.0, 0.0, 90.0, 10.0), placement.clip_rect);
        assert!(hidden.clipped_out());
    }

    #[test]
    fn available_space_is_not_a_hit_target_or_a_clip() {
        let placement = Placement::new(
            Rect::new(10.0, 20.0, 15.0, 30.0),
            Rect::new(0.0, 25.0, 100.0, 50.0),
        )
        .with_available_rect(Rect::new(10.0, 0.0, 15.0, 100.0));
        assert!(!placement.contains(Point::new(12.0, 40.0)));
        let filled = placement.fill_height();
        assert_eq!(filled.rect, placement.available_rect);
        assert_eq!(filled.clip_rect, placement.clip_rect);
        assert!(filled.contains(Point::new(12.0, 40.0)));
        assert!(!filled.contains(Point::new(12.0, 60.0)));
    }
}
