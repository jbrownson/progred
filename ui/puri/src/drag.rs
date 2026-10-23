//! Pointer displacement with a caller-supplied drag threshold.

use kurbo::{Point, Vec2};

pub struct Drag {
    origin: Point,
    previous: Point,
    scale: f64,
    threshold: f64,
    dragging: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Motion {
    pub movement: Vec2,
    pub distance: Vec2,
}

impl Drag {
    pub fn new(origin: Point, scale: f64, threshold: f64) -> Self {
        Self {
            origin,
            previous: origin,
            scale,
            threshold,
            dragging: false,
        }
    }

    pub fn advance(&mut self, point: Point) -> Option<Motion> {
        let distance = (point - self.origin) / self.scale;
        let movement = if self.dragging {
            (point - self.previous) / self.scale
        } else {
            distance
        };
        self.dragging |= distance.hypot() >= self.threshold;
        self.previous = point;
        self.dragging.then_some(Motion { movement, distance })
    }
}

#[cfg(test)]
mod tests;
