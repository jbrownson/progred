//! Drawing as a final-tagless `Canvas` trait plus a recording interpreter.
//!
//! peniko provides the styling vocabulary and kurbo the geometry. The
//! `Shape` and `GlyphRun` types stay concrete in the trait so recordings
//! keep their identity (a rect records as a rect, not a bezier soup).
//! `DrawList` is the recording interpreter — tests, goldens, and future
//! fragment caching consume frames as data — and `replay` plays a
//! recording back into any canvas.

use kurbo::{Affine, BezPath, Circle, Line, Rect, RoundedRect, Stroke};
use peniko::{Brush, FontData, ImageAlphaType, ImageData, ImageFormat};
use std::borrow::Cow;

/// A measured Puri leaf. Text asks its consumer to shape one line;
/// drawing programs carry their own metrics and use leaf-local logical
/// coordinates. `Paint` is deliberately supplied by the consumer —
/// Progred uses semantic faces while another application may use brushes
/// directly.
#[derive(Debug, Clone)]
pub enum Leaf<Paint> {
    Text {
        text: String,
        paint: Paint,
        script: crate::text::Script,
    },
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
    Mesh {
        scene: crate::mesh::Scene,
        transform: Affine,
    },
    Image {
        image: ImageData,
        transform: Affine,
    },
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
            Self::Mesh { scene, transform } => Command::Mesh { scene, transform },
            Self::Image { image, transform } => Command::Image { image, transform },
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
pub fn draw<Paint, C: CanvasSink + ?Sized>(
    drawing: Drawing<Paint>,
    canvas: &mut C,
    outer: Affine,
    resolve: impl Fn(&Paint) -> Brush,
) {
    draw_commands(drawing.commands, canvas, outer, &resolve);
}

fn draw_commands<Paint, C: CanvasSink + ?Sized>(
    commands: Vec<Command<Paint>>,
    canvas: &mut C,
    outer: Affine,
    resolve: &impl Fn(&Paint) -> Brush,
) {
    for command in commands {
        match command {
            Command::Mesh { scene, transform } => canvas.draw_mesh(scene, outer * transform),
            Command::Image { image, transform } => canvas.draw_image(image, outer * transform),
            Command::Fill {
                shape,
                paint,
                transform,
            } => canvas.fill_shape(shape, resolve(&paint), outer * transform),
            Command::Stroke {
                shape,
                style,
                paint,
                transform,
            } => canvas.stroke_shape(shape, style, resolve(&paint), outer * transform),
            Command::Clip {
                shape,
                transform,
                children,
            } => canvas.with_clip(
                shape,
                outer * transform,
                Box::new(|canvas| draw_commands(children, canvas, outer, resolve)),
            ),
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
pub trait Canvas: CanvasSink {
    fn mesh(&mut self, scene: crate::mesh::Scene, transform: Affine) {
        self.draw_mesh(scene, transform);
    }
    fn image(&mut self, image: ImageData, transform: Affine) {
        self.draw_image(image, transform);
    }
    fn fill(&mut self, shape: impl Into<Shape>, brush: impl Into<Brush>, transform: Affine) {
        self.fill_shape(shape.into(), brush.into(), transform);
    }
    fn stroke(
        &mut self,
        shape: impl Into<Shape>,
        style: Stroke,
        brush: impl Into<Brush>,
        transform: Affine,
    ) {
        self.stroke_shape(shape.into(), style, brush.into(), transform);
    }
    fn glyph_run(&mut self, run: GlyphRun) {
        self.draw_glyphs(run);
    }
    fn clip(
        &mut self,
        shape: impl Into<Shape>,
        transform: Affine,
        content: impl FnOnce(&mut dyn CanvasSink),
    ) {
        self.with_clip(shape.into(), transform, Box::new(content));
    }
}
impl<C: CanvasSink + ?Sized> Canvas for C {}

/// Object-safe canvas primitives implemented by drawing backends and recorders.
pub trait CanvasSink {
    /// Draw an orthographic mesh viewport at its natural pixel size. Backends
    /// may render directly on the GPU; this default is the CPU interpretation.
    fn draw_mesh(&mut self, scene: crate::mesh::Scene, transform: Affine) {
        if let Some(image) = scene.rasterize() {
            self.draw_image(image, transform);
        }
    }
    /// Draw at natural pixel size before applying the transform.
    fn draw_image(&mut self, image: ImageData, transform: Affine);
    fn fill_shape(&mut self, shape: Shape, brush: Brush, transform: Affine);
    fn stroke_shape(&mut self, shape: Shape, style: Stroke, brush: Brush, transform: Affine);
    fn draw_glyphs(&mut self, run: GlyphRun);
    /// The callback scopes the clip, keeping push/pop paired.
    fn with_clip(
        &mut self,
        shape: Shape,
        transform: Affine,
        content: Box<dyn FnOnce(&mut dyn CanvasSink) + '_>,
    );
}

/// `image`'s pixels as straight-alpha RGBA, for a backend that takes only
/// that layout; None for a format this doesn't know.
pub fn straight_rgba(image: &ImageData) -> Option<Cow<'_, [u8]>> {
    let bgra = match image.format {
        ImageFormat::Rgba8 => Some(false),
        ImageFormat::Bgra8 => Some(true),
        _ => None,
    }?;
    let premultiplied = image.alpha_type == ImageAlphaType::AlphaPremultiplied;
    Some(if bgra || premultiplied {
        let mut rgba = image.data.as_ref().to_vec();
        for pixel in rgba.chunks_exact_mut(4) {
            if bgra {
                pixel.swap(0, 2);
            }
            if premultiplied {
                let alpha = u16::from(pixel[3]);
                for channel in &mut pixel[..3] {
                    *channel = if alpha == 0 {
                        0
                    } else {
                        ((u16::from(*channel) * 255 + alpha / 2) / alpha).min(255) as u8
                    };
                }
            }
        }
        Cow::Owned(rgba)
    } else {
        Cow::Borrowed(image.data.as_ref())
    })
}

#[derive(Debug, Clone)]
pub enum DrawCmd {
    Mesh {
        scene: crate::mesh::Scene,
        transform: Affine,
    },
    Image {
        image: ImageData,
        transform: Affine,
    },
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

impl CanvasSink for DrawList {
    fn draw_mesh(&mut self, scene: crate::mesh::Scene, transform: Affine) {
        self.0.push(DrawCmd::Mesh { scene, transform });
    }
    fn draw_image(&mut self, image: ImageData, transform: Affine) {
        self.0.push(DrawCmd::Image { image, transform });
    }

    fn fill_shape(&mut self, shape: Shape, brush: Brush, transform: Affine) {
        self.0.push(DrawCmd::Fill {
            shape,
            brush,
            transform,
        });
    }

    fn stroke_shape(&mut self, shape: Shape, style: Stroke, brush: Brush, transform: Affine) {
        self.0.push(DrawCmd::Stroke {
            shape,
            style,
            brush,
            transform,
        });
    }

    fn draw_glyphs(&mut self, run: GlyphRun) {
        self.0.push(DrawCmd::GlyphRun(run));
    }

    fn with_clip(
        &mut self,
        shape: Shape,
        transform: Affine,
        content: Box<dyn FnOnce(&mut dyn crate::draw::CanvasSink) + '_>,
    ) {
        let outer = std::mem::take(&mut self.0);
        content(self);
        let children = std::mem::replace(&mut self.0, outer);
        self.0.push(DrawCmd::Clip {
            shape,
            transform,
            children,
        });
    }
}

/// Play a recording back into any canvas.
pub fn replay(list: &DrawList, canvas: &mut (impl Canvas + ?Sized)) {
    replay_at(list, canvas, Affine::IDENTITY);
}

/// Play a leaf-local recording at `outer` into any canvas.
pub fn replay_at(list: &DrawList, canvas: &mut (impl Canvas + ?Sized), outer: Affine) {
    replay_cmds(&list.0, canvas, outer);
}

fn replay_cmds<C: Canvas + ?Sized>(cmds: &[DrawCmd], canvas: &mut C, outer: Affine) {
    for cmd in cmds {
        match cmd {
            DrawCmd::Mesh { scene, transform } => canvas.mesh(scene.clone(), outer * *transform),
            DrawCmd::Image { image, transform } => canvas.image(image.clone(), outer * *transform),
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
mod tests;
