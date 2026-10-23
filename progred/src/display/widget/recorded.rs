//! A leaf a program draws into. The program runs at most once per frame,
//! when the leaf is first hit-tested or painted, recording fills linked to the
//! calls that drew them. The leaf hit-tests those fills for their sources,
//! links them to their source on Cmd+click and hover, and highlights the
//! selected and hovered sources.

use super::HoverPass;
use super::placed::leaf;
use super::style::{Styles, brush};
use crate::display::Host;
use crate::display::widget::source::SourceTrace;
use crate::hover::{Hover, SourceCalls};
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
    styles: Styles,
}

impl Recorder {
    fn new(input: Option<SourceTrace>, styles: Styles) -> Self {
        Self {
            drawn: Recorded::default(),
            input,
            styles,
        }
    }

    /// Fill `shape` with `paint`, resolving a face here so the program never
    /// sees the theme. The fill is a hit when the calls that drew it name a
    /// source.
    pub fn fill(
        &mut self,
        shape: puri::draw::Shape,
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
            .fill(shape, brush(&self.styles, paint), transform);
    }
}

/// A leaf `extent` logical units big that `record` draws into. `program`
/// locates the program below this occurrence, when it is stored there, so the
/// calls a fill traces back to inside it name the document.
pub fn recorded<W: Host>(
    extent: Extent,
    program: Option<Vec<Step>>,
    record: Record,
) -> crate::display::host::Layout<W> {
    crate::display::Layout::widget(Rc::new(move |context| {
        recorded_leaf(context.toolkit, extent, program.as_deref(), record.clone())
    }))
}

fn recorded_leaf<W: Host>(
    toolkit: &dyn crate::display::Toolkit,
    extent: Extent,
    program: Option<&[Step]>,
    record: Record,
) -> Measured<HoverPass<W, W::Hover>> {
    let Extent {
        width,
        ascent,
        descent,
    } = extent;
    let styles = toolkit.styles();
    let scale = styles.scale;
    let highlight = styles.accent_wash.brush.clone();
    let selected_highlight = styles.selection_wash.clone();
    let scaled = Extent {
        width: width * scale,
        ascent: ascent * scale,
        descent: descent * scale,
    };
    let input = program.and_then(|program| toolkit.trace_below(program));
    let sources = toolkit.sources();
    let document = sources.doc.clone();
    let libraries = sources.libraries.clone();
    // Recording runs after the projection pass's borrows end, so it keeps
    // its own clones of the styles, the document and the libraries.
    let styles = styles.clone();
    let drawing = Rc::new(LazyCell::new(move || {
        let sources = Sources {
            doc: &document,
            libraries: &libraries,
        };
        draw(&record, &sources, input, styles)
    }));
    let selected = toolkit.selected_trace().cloned();
    leaf(scaled, move |placed, placement| {
        let outer =
            Affine::translate((placement.rect.x0, placement.rect.y0)) * Affine::scale(scale);
        let probe_drawing = drawing.clone();
        placed.claim_dynamic(placement, move |point| {
            probe_drawing.target_at(point, outer).map(W::Hover::from)
        });
        let command = placed.command_modifier();
        crate::display::widget::source_link::handlers(placed, placement, scale, command);
        placed.render(move |canvas: &mut dyn puri::draw::CanvasSink, hover| {
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
    styles: Styles,
) -> Recorded {
    let mut recorder = Recorder::new(input, styles);
    if record(host, &mut recorder) {
        recorder.drawn
    } else {
        Recorded::default()
    }
}

/// What a program drew.
#[derive(Default)]
struct Recorded {
    commands: DrawList,
    hits: Vec<Hit>,
}

struct Hit {
    shape: puri::draw::Shape,
    transform: Affine,
    inverse: Affine,
    bounds: Rect,
    calls: SourceCalls,
}

impl Hit {
    fn new(shape: puri::draw::Shape, transform: Affine, calls: SourceCalls) -> Self {
        let bounds = transform.transform_rect_bbox(shape_bounds(&shape));
        Self {
            shape,
            transform,
            inverse: transform.inverse(),
            bounds,
            calls,
        }
    }

    fn contains(&self, point: Point) -> bool {
        self.bounds.contains(point) && shape_contains(&self.shape, self.inverse * point)
    }
}

impl Recorded {
    fn target_at(&self, point: Point, outer: Affine) -> Option<Hover> {
        let point = outer.inverse() * point;
        self.hits
            .iter()
            .rev()
            .find(|hit| hit.contains(point))
            .map(|hit| Hover::Calls(hit.calls.clone()))
    }

    fn highlight<C: Canvas + ?Sized>(
        &self,
        canvas: &mut C,
        outer: Affine,
        source: &SourceTrace,
        brush: &Brush,
    ) {
        for hit in self.hits.iter().filter(|hit| hit.calls.contains(source)) {
            canvas.fill(hit.shape.clone(), brush.clone(), outer * hit.transform);
        }
    }
}

fn shape_bounds(shape: &puri::draw::Shape) -> Rect {
    match shape {
        puri::draw::Shape::Rect(shape) => shape.bounding_box(),
        puri::draw::Shape::RoundedRect(shape) => shape.bounding_box(),
        puri::draw::Shape::Circle(shape) => shape.bounding_box(),
        puri::draw::Shape::Line(shape) => shape.bounding_box(),
        puri::draw::Shape::Path(shape) => shape.bounding_box(),
    }
}

fn shape_contains(shape: &puri::draw::Shape, point: Point) -> bool {
    match shape {
        puri::draw::Shape::Rect(shape) => shape.contains(point),
        puri::draw::Shape::RoundedRect(shape) => shape.contains(point),
        puri::draw::Shape::Circle(shape) => shape.contains(point),
        puri::draw::Shape::Line(shape) => shape.contains(point),
        puri::draw::Shape::Path(shape) => shape.contains(point),
    }
}

#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;
