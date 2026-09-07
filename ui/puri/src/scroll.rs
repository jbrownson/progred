//! Convert scroll input and its unconsumed remainder through the same units.

use crate::handler::{ScrollDelta, ScrollOutcome};
use kurbo::{Size, Vec2};

/// Positive conversion factors: pixels per logical point, points per line,
/// and the page extent in logical points. Policy belongs to the caller.
pub struct Units {
    pub scale: f64,
    pub line: f64,
    pub page: Size,
}

impl Units {
    pub fn handle(
        &self,
        delta: ScrollDelta,
        handle: impl FnOnce(Vec2) -> ScrollOutcome<Vec2>,
    ) -> ScrollOutcome {
        let (input, units) = match delta {
            ScrollDelta::PixelDelta(point) => (
                Vec2::new(point.x, point.y),
                Vec2::new(1.0 / self.scale, 1.0 / self.scale),
            ),
            ScrollDelta::LineDelta(x, y) => (
                Vec2::new(f64::from(x), f64::from(y)),
                Vec2::new(self.line, self.line),
            ),
            ScrollDelta::PageDelta(x, y) => (
                Vec2::new(f64::from(x), f64::from(y)),
                Vec2::new(self.page.width, self.page.height),
            ),
        };
        handle(Vec2::new(input.x * units.x, input.y * units.y)).map(|remaining| {
            let remaining = Vec2::new(remaining.x / units.x, remaining.y / units.y);
            match delta {
                ScrollDelta::PixelDelta(_) => {
                    ScrollDelta::PixelDelta((remaining.x, remaining.y).into())
                }
                ScrollDelta::LineDelta(..) => {
                    ScrollDelta::LineDelta(remaining.x as f32, remaining.y as f32)
                }
                ScrollDelta::PageDelta(..) => {
                    ScrollDelta::PageDelta(remaining.x as f32, remaining.y as f32)
                }
            }
        })
    }
}
