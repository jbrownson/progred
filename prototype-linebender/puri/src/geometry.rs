use kurbo::{Point, Rect};

/// A widget's full settled rectangle and the effective enclosing
/// axis-aligned clipping area supplied by its caller.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Placement {
    pub rect: Rect,
    pub clip_rect: Rect,
}

impl Placement {
    pub const fn new(rect: Rect, clip_rect: Rect) -> Self {
        Self { rect, clip_rect }
    }

    pub const fn root(rect: Rect) -> Self {
        Self::new(rect, rect)
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
        let hidden = Placement::new(
            Rect::new(70.0, 0.0, 90.0, 10.0),
            placement.clip_rect,
        );
        assert!(hidden.clipped_out());
    }
}
