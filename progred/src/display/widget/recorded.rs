//! A leaf a program draws into. The program runs at most once per frame,
//! when the leaf is first hit-tested or painted, recording fills linked to the
//! calls that drew them. The leaf hit-tests those fills for their sources,
//! links them to their source on Cmd+click and hover, and highlights the
//! selected and hovered sources.

use crate::frame::Hovered;
use crate::hover::{Hover, SourceCalls, SourceTrace};
use crate::placed::{HoverPass, leaf};
use crate::sources::Sources;
use gid::Step;
use kurbo::{Affine, Point, Rect, Shape as _};
use measured::{Extent, Measured};
use peniko::Brush;
use puri::draw::{Canvas, DrawList};
use std::cell::LazyCell;
use std::rc::Rc;

/// Runs a program into the recorder, reading through the host; false, for a
/// decline or a halt, discards what it drew.
pub type Record = Rc<dyn Fn(&dyn grap::Host, &mut Recorder) -> bool>;

/// Collects what a program draws: fills, each linked to the calls that drew
/// it.
pub struct Recorder {
    drawn: Recorded,
    input: Option<SourceTrace>,
    faces: Faces,
}

impl Recorder {
    fn new(input: Option<SourceTrace>, faces: Faces) -> Self {
        Self {
            drawn: Recorded::default(),
            input,
            faces,
        }
    }

    /// Fill `shape` with `paint`, resolving a face here so the program never
    /// sees the theme. The fill is a hit when the calls that drew it name a
    /// source.
    pub fn fill(
        &mut self,
        shape: puri::Shape,
        paint: crate::display::Paint,
        transform: Affine,
        calls: Option<grap::CallTrace>,
    ) {
        if let Some(calls) = calls
            .map(|trace| SourceCalls::new(trace, self.input.clone()))
            .filter(SourceCalls::has_source)
        {
            self.drawn
                .hits
                .push(Hit::new(shape.clone(), transform, calls));
        }
        self.drawn
            .commands
            .fill(shape, self.faces.resolve(paint), transform);
    }
}

/// A leaf `extent` logical units big that `record` draws into. `program`
/// locates the program below this occurrence, so the calls a fill traces back
/// to inside it name the document.
pub fn recorded(extent: Extent, program: Vec<Step>, record: Record) -> crate::view::Layout {
    crate::display::Layout::widget(Rc::new(move |context| {
        recorded_leaf(
            context.inputs,
            context.path,
            extent,
            &program,
            record.clone(),
        )
    }))
}

fn recorded_leaf(
    cx: &crate::projection::Cx,
    path: &[Step],
    extent: Extent,
    program: &[Step],
    record: Record,
) -> Measured<HoverPass<crate::Editor>> {
    let Extent {
        width,
        ascent,
        descent,
    } = extent;
    let scale = cx.styles.scale;
    let scaled = Extent {
        width: width * scale,
        ascent: ascent * scale,
        descent: descent * scale,
    };
    let faces = Faces::new(cx.styles);
    let program_path: Vec<_> = path.iter().chain(program).cloned().collect();
    let input = cx
        .edits
        .source(&program_path)
        .map(|location| SourceTrace::from_location(&cx.sources, &location));
    let document = cx.sources.doc.clone();
    let libraries = cx.sources.libraries.clone();
    // Recording runs after the projection pass's borrows end, so it keeps
    // its own clones of the document and libraries.
    let drawing = Rc::new(LazyCell::new(move || {
        let sources = Sources {
            doc: &document,
            libraries: &libraries,
        };
        draw(&record, &sources, input, faces)
    }));
    let highlight = cx.styles.accent_wash.brush.clone();
    let selected_highlight = cx.styles.selection_wash.clone();
    let selected = cx.selected_trace.clone();
    leaf(scaled, move |builder, placement| {
        let outer =
            Affine::translate((placement.rect.x0, placement.rect.y0)) * Affine::scale(scale);
        let probe_drawing = drawing.clone();
        builder.claim_dynamic(placement, move |point| {
            probe_drawing.target_at(point, outer)
        });
        let command = builder.command_modifier();
        crate::projection::source_link::handlers(builder, placement, scale, command);
        builder.render(move |canvas: &mut dyn puri::draw::CanvasSink, hover| {
            canvas.clip(
                Rect::new(0.0, 0.0, width, ascent + descent),
                outer,
                |canvas| {
                    puri::draw::replay_at(&drawing.commands, canvas, outer);
                    if let Some(source) = &selected {
                        drawing.highlight(canvas, outer, source, &selected_highlight);
                    }
                    if let Some(source) = hover.hovered_trace.as_ref() {
                        drawing.highlight(canvas, outer, source, &highlight);
                    }
                },
            );
        });
    })
}

/// Runs `record`, keeping what it drew only if it completes.
fn draw(
    record: &Record,
    host: &dyn grap::Host,
    input: Option<SourceTrace>,
    faces: Faces,
) -> Recorded {
    let mut recorder = Recorder::new(input, faces);
    if record(host, &mut recorder) {
        recorder.drawn
    } else {
        Recorded::default()
    }
}

/// What a recording drew, for tests of the programs that draw.
#[cfg(test)]
pub(crate) struct Drawing(Recorded);

#[cfg(test)]
impl Drawing {
    /// Runs `record` as a leaf would.
    pub(crate) fn of(
        record: &Record,
        host: &dyn grap::Host,
        input: Option<SourceTrace>,
        styles: &crate::styles::Styles,
    ) -> Self {
        Self(draw(record, host, input, Faces::new(styles)))
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.commands.0.is_empty() && self.0.hits.is_empty()
    }

    pub(crate) fn hits(&self) -> usize {
        self.0.hits.len()
    }

    pub(crate) fn target_at(&self, point: Point) -> Option<Hovered> {
        self.0.target_at(point, Affine::IDENTITY)
    }

    /// Whether highlighting `source` fills anything.
    pub(crate) fn highlights(&self, source: &SourceTrace) -> bool {
        let mut highlighted = DrawList::new();
        self.0.highlight(
            &mut highlighted,
            Affine::IDENTITY,
            source,
            &Brush::default(),
        );
        !highlighted.0.is_empty()
    }
}

/// Recording happens after the frame's style borrow ends, so it keeps a copy.
struct Faces(crate::styles::Styles);

impl Faces {
    fn new(styles: &crate::styles::Styles) -> Self {
        Self(styles.clone())
    }

    fn resolve(&self, paint: crate::display::Paint) -> Brush {
        match paint {
            crate::display::Paint::Brush(brush) => brush,
            crate::display::Paint::Face(face) => {
                crate::display::widget::style::face_style(&self.0, face)
                    .brush
                    .clone()
            }
        }
    }
}

/// What a program drew.
#[derive(Default)]
struct Recorded {
    commands: DrawList,
    hits: Vec<Hit>,
}

struct Hit {
    shape: puri::Shape,
    transform: Affine,
    inverse: Affine,
    bounds: Rect,
    source: HitSource,
}

enum HitSource {
    Exact(SourceTrace),
    Calls(SourceCalls),
}

impl From<SourceTrace> for HitSource {
    fn from(source: SourceTrace) -> Self {
        Self::Exact(source)
    }
}

impl From<SourceCalls> for HitSource {
    fn from(source: SourceCalls) -> Self {
        Self::Calls(source)
    }
}

impl Hit {
    fn new(shape: puri::Shape, transform: Affine, source: impl Into<HitSource>) -> Self {
        let bounds = transform.transform_rect_bbox(shape_bounds(&shape));
        Self {
            shape,
            transform,
            inverse: transform.inverse(),
            bounds,
            source: source.into(),
        }
    }

    fn contains(&self, point: Point) -> bool {
        self.bounds.contains(point) && shape_contains(&self.shape, self.inverse * point)
    }
}

impl Recorded {
    fn target_at(&self, point: Point, outer: Affine) -> Option<Hovered> {
        let point = outer.inverse() * point;
        self.hits
            .iter()
            .rev()
            .find(|hit| hit.contains(point))
            .map(|hit| {
                Hovered::Tree(match &hit.source {
                    HitSource::Calls(calls) => Hover::Calls(calls.clone()),
                    HitSource::Exact(source) => Hover::Source(source.clone()),
                })
            })
    }

    fn highlight<C: Canvas + ?Sized>(
        &self,
        canvas: &mut C,
        outer: Affine,
        source: &SourceTrace,
        brush: &Brush,
    ) {
        for hit in self.hits.iter().filter(|hit| match &hit.source {
            HitSource::Calls(calls) => calls.contains(source),
            HitSource::Exact(hit) => hit == source,
        }) {
            canvas.fill(hit.shape.clone(), brush.clone(), outer * hit.transform);
        }
    }
}

fn shape_bounds(shape: &puri::Shape) -> Rect {
    match shape {
        puri::Shape::Rect(shape) => shape.bounding_box(),
        puri::Shape::RoundedRect(shape) => shape.bounding_box(),
        puri::Shape::Circle(shape) => shape.bounding_box(),
        puri::Shape::Line(shape) => shape.bounding_box(),
        puri::Shape::Path(shape) => shape.bounding_box(),
    }
}

fn shape_contains(shape: &puri::Shape, point: Point) -> bool {
    match shape {
        puri::Shape::Rect(shape) => shape.contains(point),
        puri::Shape::RoundedRect(shape) => shape.contains(point),
        puri::Shape::Circle(shape) => shape.contains(point),
        puri::Shape::Line(shape) => shape.contains(point),
        puri::Shape::Path(shape) => shape.contains(point),
    }
}

#[cfg(test)]
mod tests;
