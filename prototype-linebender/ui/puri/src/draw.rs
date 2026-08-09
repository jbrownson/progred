//! Drawing as a final-tagless `Canvas` trait plus a recording interpreter.
//!
//! peniko provides the styling vocabulary and kurbo the geometry. The
//! `Shape` and `GlyphRun` types stay concrete in the trait so recordings
//! keep their identity (a rect records as a rect, not a bezier soup).
//! `DrawList` is the recording interpreter — tests, goldens, and future
//! fragment caching consume frames as data — and `replay` plays a
//! recording back into any canvas.

use kurbo::{Affine, BezPath, Circle, Line, Rect, RoundedRect, Stroke};
use peniko::{Brush, FontData};

#[derive(Debug, Clone)]
pub enum Shape {
    Rect(Rect),
    RoundedRect(RoundedRect),
    Circle(Circle),
    Line(Line),
    Path(BezPath),
}

impl From<Rect> for Shape {
    fn from(shape: Rect) -> Self {
        Self::Rect(shape)
    }
}

impl From<RoundedRect> for Shape {
    fn from(shape: RoundedRect) -> Self {
        Self::RoundedRect(shape)
    }
}

impl From<Circle> for Shape {
    fn from(shape: Circle) -> Self {
        Self::Circle(shape)
    }
}

impl From<Line> for Shape {
    fn from(shape: Line) -> Self {
        Self::Line(shape)
    }
}

impl From<BezPath> for Shape {
    fn from(shape: BezPath) -> Self {
        Self::Path(shape)
    }
}

/// A glyph positioned in run-local coordinates.
#[derive(Debug, Clone, PartialEq)]
pub struct Glyph {
    pub id: u32,
    pub x: f32,
    pub y: f32,
}

#[derive(Debug, Clone)]
pub struct GlyphRun {
    pub font: FontData,
    pub size: f32,
    pub glyphs: Vec<Glyph>,
    /// Variable-font axis positions, raw `F2Dot14` bits as parley reports them.
    pub normalized_coords: Vec<i16>,
    pub brush: Brush,
    pub hint: bool,
    pub transform: Affine,
    pub glyph_transform: Option<Affine>,
}

/// The drawing interface widgets and projections write to. Backends
/// stream (puri-vello), recorders capture (`DrawList`), tests interpret
/// however the assertion wants.
pub trait Canvas {
    fn fill(&mut self, shape: impl Into<Shape>, brush: impl Into<Brush>, transform: Affine);
    fn stroke(
        &mut self,
        shape: impl Into<Shape>,
        style: Stroke,
        brush: impl Into<Brush>,
        transform: Affine,
    );
    fn glyph_run(&mut self, run: GlyphRun);
    /// Draw `content` clipped to `shape`; the clip scope is the
    /// closure, so unbalanced push/pop is unrepresentable.
    fn clip(&mut self, shape: impl Into<Shape>, transform: Affine, content: impl FnOnce(&mut Self));
}

#[derive(Debug, Clone)]
pub enum DrawCmd {
    Fill {
        shape: Shape,
        brush: Brush,
        transform: Affine,
    },
    Stroke {
        shape: Shape,
        style: Stroke,
        brush: Brush,
        transform: Affine,
    },
    GlyphRun(GlyphRun),
    Clip {
        shape: Shape,
        transform: Affine,
        children: Vec<DrawCmd>,
    },
}

/// The recording interpreter: a frame captured as data.
#[derive(Debug, Clone, Default)]
pub struct DrawList(pub Vec<DrawCmd>);

impl DrawList {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Canvas for DrawList {
    fn fill(&mut self, shape: impl Into<Shape>, brush: impl Into<Brush>, transform: Affine) {
        self.0.push(DrawCmd::Fill {
            shape: shape.into(),
            brush: brush.into(),
            transform,
        });
    }

    fn stroke(
        &mut self,
        shape: impl Into<Shape>,
        style: Stroke,
        brush: impl Into<Brush>,
        transform: Affine,
    ) {
        self.0.push(DrawCmd::Stroke {
            shape: shape.into(),
            style,
            brush: brush.into(),
            transform,
        });
    }

    fn glyph_run(&mut self, run: GlyphRun) {
        self.0.push(DrawCmd::GlyphRun(run));
    }

    fn clip(&mut self, shape: impl Into<Shape>, transform: Affine, content: impl FnOnce(&mut Self)) {
        let outer = std::mem::take(&mut self.0);
        content(self);
        let children = std::mem::replace(&mut self.0, outer);
        self.0.push(DrawCmd::Clip {
            shape: shape.into(),
            transform,
            children,
        });
    }
}

/// Play a recording back into any canvas.
pub fn replay(list: &DrawList, canvas: &mut impl Canvas) {
    replay_cmds(&list.0, canvas);
}

fn replay_cmds<C: Canvas>(cmds: &[DrawCmd], canvas: &mut C) {
    for cmd in cmds {
        match cmd {
            DrawCmd::Fill {
                shape,
                brush,
                transform,
            } => canvas.fill(shape.clone(), brush.clone(), *transform),
            DrawCmd::Stroke {
                shape,
                style,
                brush,
                transform,
            } => canvas.stroke(shape.clone(), style.clone(), brush.clone(), *transform),
            DrawCmd::GlyphRun(run) => canvas.glyph_run(run.clone()),
            DrawCmd::Clip {
                shape,
                transform,
                children,
            } => canvas.clip(shape.clone(), *transform, |inner| {
                replay_cmds(children, inner);
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use peniko::Color;

    fn sample() -> DrawList {
        let mut list = DrawList::new();
        list.fill(
            Rect::new(0.0, 0.0, 10.0, 10.0),
            Color::WHITE,
            Affine::IDENTITY,
        );
        list.clip(Rect::new(1.0, 1.0, 9.0, 9.0), Affine::IDENTITY, |list| {
            list.stroke(
                Line::new((0.0, 0.0), (10.0, 10.0)),
                Stroke::new(2.0),
                Color::BLACK,
                Affine::translate((5.0, 0.0)),
            );
        });
        list
    }

    #[test]
    fn recording_keeps_call_order_and_nesting() {
        let list = sample();
        assert!(matches!(
            &list.0[..],
            [
                DrawCmd::Fill {
                    shape: Shape::Rect(_),
                    ..
                },
                DrawCmd::Clip { children, .. },
            ] if matches!(
                &children[..],
                [DrawCmd::Stroke {
                    shape: Shape::Line(_),
                    ..
                }]
            )
        ));
    }

    #[test]
    fn recording_keeps_geometry_exactly() {
        let DrawCmd::Fill {
            shape: Shape::Rect(rect),
            transform,
            ..
        } = &sample().0[0]
        else {
            panic!("expected a rect fill");
        };
        assert_eq!(*rect, Rect::new(0.0, 0.0, 10.0, 10.0));
        assert_eq!(*transform, Affine::IDENTITY);
    }

    #[test]
    fn replay_reproduces_the_recording() {
        let original = sample();
        let mut replayed = DrawList::new();
        replay(&original, &mut replayed);
        assert_eq!(format!("{original:?}"), format!("{replayed:?}"));
    }
}
