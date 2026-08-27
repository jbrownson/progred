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

/// A measured Puri leaf. Text asks its consumer to shape one line;
/// drawing programs carry their own metrics and use leaf-local logical
/// coordinates. `Paint` is deliberately supplied by the consumer —
/// Progred uses semantic faces while another application may use brushes
/// directly.
#[derive(Debug, Clone)]
pub enum Leaf<Paint> {
    Text { text: String, paint: Paint },
    Drawing(Drawing<Paint>),
}

/// An initially encoded Puri canvas program with explicit baseline metrics.
/// It is the data interpreter of the same fill/stroke/clip language exposed
/// by [`Canvas`], suitable for projection output and tests.
#[derive(Debug, Clone)]
pub struct Drawing<Paint> {
    pub width: f64,
    pub ascent: f64,
    pub descent: f64,
    pub commands: Vec<Command<Paint>>,
}

impl<Paint> Drawing<Paint> {
    pub fn map_paint<Mapped>(self, map: impl Fn(Paint) -> Mapped) -> Drawing<Mapped> {
        Drawing {
            width: self.width,
            ascent: self.ascent,
            descent: self.descent,
            commands: self
                .commands
                .into_iter()
                .map(|command| command.map_paint(&map))
                .collect(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Command<Paint> {
    Fill {
        shape: Shape,
        paint: Paint,
        transform: Affine,
    },
    Stroke {
        shape: Shape,
        style: Stroke,
        paint: Paint,
        transform: Affine,
    },
    Clip {
        shape: Shape,
        transform: Affine,
        children: Vec<Command<Paint>>,
    },
}

impl<Paint> Command<Paint> {
    fn map_paint<Mapped>(self, map: &impl Fn(Paint) -> Mapped) -> Command<Mapped> {
        match self {
            Self::Fill {
                shape,
                paint,
                transform,
            } => Command::Fill {
                shape,
                paint: map(paint),
                transform,
            },
            Self::Stroke {
                shape,
                style,
                paint,
                transform,
            } => Command::Stroke {
                shape,
                style,
                paint: map(paint),
                transform,
            },
            Self::Clip {
                shape,
                transform,
                children,
            } => Command::Clip {
                shape,
                transform,
                children: children
                    .into_iter()
                    .map(|child| child.map_paint(map))
                    .collect(),
            },
        }
    }
}

/// Interpret an initially encoded drawing into any Puri canvas. `outer`
/// places the leaf; command transforms remain local to it.
pub fn draw<Paint, C: Canvas>(
    drawing: Drawing<Paint>,
    canvas: &mut C,
    outer: Affine,
    resolve: impl Fn(&Paint) -> Brush,
) {
    draw_commands(drawing.commands, canvas, outer, &resolve);
}

fn draw_commands<Paint, C: Canvas>(
    commands: Vec<Command<Paint>>,
    canvas: &mut C,
    outer: Affine,
    resolve: &impl Fn(&Paint) -> Brush,
) {
    for command in commands {
        match command {
            Command::Fill {
                shape,
                paint,
                transform,
            } => canvas.fill(shape, resolve(&paint), outer * transform),
            Command::Stroke {
                shape,
                style,
                paint,
                transform,
            } => canvas.stroke(shape, style, resolve(&paint), outer * transform),
            Command::Clip {
                shape,
                transform,
                children,
            } => canvas.clip(shape, outer * transform, |canvas| {
                draw_commands(children, canvas, outer, resolve)
            }),
        }
    }
}

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
    replay_at(list, canvas, Affine::IDENTITY);
}

/// Play a leaf-local recording at `outer` into any canvas.
pub fn replay_at(list: &DrawList, canvas: &mut impl Canvas, outer: Affine) {
    replay_cmds(&list.0, canvas, outer);
}

fn replay_cmds<C: Canvas>(cmds: &[DrawCmd], canvas: &mut C, outer: Affine) {
    for cmd in cmds {
        match cmd {
            DrawCmd::Fill {
                shape,
                brush,
                transform,
            } => canvas.fill(shape.clone(), brush.clone(), outer * *transform),
            DrawCmd::Stroke {
                shape,
                style,
                brush,
                transform,
            } => canvas.stroke(
                shape.clone(),
                style.clone(),
                brush.clone(),
                outer * *transform,
            ),
            DrawCmd::GlyphRun(run) => {
                let mut run = run.clone();
                run.transform = outer * run.transform;
                canvas.glyph_run(run);
            }
            DrawCmd::Clip {
                shape,
                transform,
                children,
            } => canvas.clip(shape.clone(), outer * *transform, |inner| {
                replay_cmds(children, inner, outer);
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

    #[test]
    fn replay_at_places_every_recorded_command() {
        let outer = Affine::translate((20.0, 30.0));
        let mut replayed = DrawList::new();
        replay_at(&sample(), &mut replayed, outer);
        assert!(matches!(
            &replayed.0[..],
            [
                DrawCmd::Fill { transform, .. },
                DrawCmd::Clip {
                    transform: clip_transform,
                    children,
                    ..
                },
            ] if *transform == outer
                && *clip_transform == outer
                && matches!(
                    &children[..],
                    [DrawCmd::Stroke { transform, .. }]
                        if *transform == outer * Affine::translate((5.0, 0.0))
                )
        ));
    }

    #[test]
    fn an_initial_drawing_interprets_through_the_canvas_language() {
        let drawing = Drawing {
            width: 10.0,
            ascent: 8.0,
            descent: 2.0,
            commands: vec![
                Command::Fill {
                    shape: Shape::Circle(Circle::new((5.0, 5.0), 4.0)),
                    paint: Color::WHITE,
                    transform: Affine::IDENTITY,
                },
                Command::Clip {
                    shape: Shape::Rect(Rect::new(0.0, 0.0, 10.0, 10.0)),
                    transform: Affine::IDENTITY,
                    children: vec![Command::Stroke {
                        shape: Shape::Line(Line::new((0.0, 0.0), (10.0, 10.0))),
                        style: Stroke::new(2.0),
                        paint: Color::BLACK,
                        transform: Affine::IDENTITY,
                    }],
                },
            ],
        };
        let mut recorded = DrawList::new();
        draw(
            drawing,
            &mut recorded,
            Affine::translate((20.0, 30.0)),
            |paint| Brush::from(*paint),
        );
        assert!(matches!(
            &recorded.0[..],
            [
                DrawCmd::Fill {
                    shape: Shape::Circle(_),
                    transform,
                    ..
                },
                DrawCmd::Clip { children, .. },
            ] if *transform == Affine::translate((20.0, 30.0))
                && matches!(children.as_slice(), [DrawCmd::Stroke { shape: Shape::Line(_), .. }])
        ));
    }
}
