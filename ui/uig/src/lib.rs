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

    pub fn fill_width(self) -> Self {
        Self {
            rect: Rect::new(
                self.available_rect.x0,
                self.rect.y0,
                self.available_rect.x1,
                self.rect.y1,
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
mod tests;
