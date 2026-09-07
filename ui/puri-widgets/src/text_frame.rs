//! A text-sized empty frame and the outline shared with filled frames.

use puri::draw::CanvasSink;
use puri::text::{TextCtx, TextMetrics, TextStyle, text};
use puri::{Affine, Brush, Placement, Rect, RoundedRect, Stroke};

pub struct EmptyFrame {
    metrics: TextMetrics,
    scale: f64,
    brush: Brush,
}

impl EmptyFrame {
    pub fn metrics(&self) -> TextMetrics {
        self.metrics
    }

    pub fn place(self, canvas: &mut (impl CanvasSink + ?Sized), placement: Placement) {
        canvas.stroke_shape(
            outline(self.scale, placement.rect).into(),
            Stroke::new(self.scale),
            self.brush,
            Affine::IDENTITY,
        );
    }
}

pub fn empty_width(text_size: f32, scale: f64) -> f64 {
    1.5 * f64::from(text_size) * scale
}

pub fn outline(scale: f64, rect: Rect) -> RoundedRect {
    RoundedRect::from_rect(rect.inflate(scale, 0.0), 4.0 * scale)
}

pub fn empty(ctx: &mut TextCtx, style: &TextStyle, brush: Brush) -> EmptyFrame {
    let scale = f64::from(ctx.scale);
    EmptyFrame {
        metrics: TextMetrics {
            width: empty_width(style.size, scale),
            ..text(ctx, "", style).metrics()
        },
        scale,
        brush,
    }
}
