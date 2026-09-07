//! Shaped completion rows. The consumer composes them with its layout,
//! scroll container, hover targets, and popup policy.

use puri::draw::CanvasSink;
use puri::{Affine, Color, Placement, Rect, RoundedRect, Text, TextCtx, TextMetrics, TextStyle};

use std::ops::Range;

pub struct Entry<'a> {
    pub display: &'a str,
    pub detail: Option<&'a str>,
    pub matches: &'a [Range<usize>],
    pub style: &'a TextStyle,
}

pub struct Style<'a> {
    pub detail: &'a TextStyle,
    pub more: &'a TextStyle,
    pub scale: f64,
    pub chosen: Color,
}

struct Line {
    segments: Vec<Text>,
    metrics: TextMetrics,
}

impl Line {
    fn one(text: Text) -> Self {
        Self {
            metrics: text.metrics(),
            segments: vec![text],
        }
    }

    fn from_segments(segments: Vec<Text>) -> Self {
        Self {
            metrics: segments.iter().map(Text::metrics).fold(
                TextMetrics::default(),
                |line, segment| TextMetrics {
                    width: line.width + segment.width,
                    ascent: line.ascent.max(segment.ascent),
                    descent: line.descent.max(segment.descent),
                },
            ),
            segments,
        }
    }

    fn draw(self, canvas: &mut (impl CanvasSink + ?Sized), x: f64, baseline: f64, clip: Rect) {
        self.segments.into_iter().fold(x, |x, segment| {
            let metrics = segment.metrics();
            segment.place(
                canvas,
                Placement::new(
                    Rect::new(
                        x,
                        baseline - metrics.ascent,
                        x + metrics.width,
                        baseline + metrics.descent,
                    ),
                    clip,
                ),
            );
            x + metrics.width
        });
    }
}

pub struct Row {
    display: Line,
    detail: Option<Line>,
    metrics: TextMetrics,
    scale: f64,
    chosen: Color,
}

impl Row {
    fn new(display: Line, detail: Option<Line>, padding: f64, style: &Style<'_>) -> Self {
        Self {
            metrics: TextMetrics {
                width: display.metrics.width
                    + detail
                        .as_ref()
                        .map_or(0.0, |detail| 8.0 * style.scale + detail.metrics.width)
                    + 16.0 * style.scale,
                ascent: detail.as_ref().map_or(display.metrics.ascent, |detail| {
                    display.metrics.ascent.max(detail.metrics.ascent)
                }) + padding,
                descent: detail.as_ref().map_or(display.metrics.descent, |detail| {
                    display.metrics.descent.max(detail.metrics.descent)
                }) + padding,
            },
            display,
            detail,
            scale: style.scale,
            chosen: style.chosen,
        }
    }

    pub fn metrics(&self) -> TextMetrics {
        self.metrics
    }

    pub fn draw(self, canvas: &mut (impl CanvasSink + ?Sized), placement: Placement, chosen: bool) {
        if chosen {
            canvas.fill_shape(
                RoundedRect::from_rect(placement.rect, 4.0 * self.scale).into(),
                self.chosen.into(),
                Affine::IDENTITY,
            );
        }
        let x = placement.rect.x0 + 8.0 * self.scale;
        let baseline = placement.rect.y0 + self.metrics.ascent;
        self.display.draw(canvas, x, baseline, placement.clip_rect);
        if let Some(detail) = self.detail {
            let detail_x = placement.rect.x1 - 8.0 * self.scale - detail.metrics.width;
            detail.draw(canvas, detail_x, baseline, placement.clip_rect);
        }
    }
}

pub struct Completion {
    pub rows: Vec<Row>,
    pub more: Option<Row>,
}

fn highlighted(tcx: &mut TextCtx, text: &str, matches: &[Range<usize>], style: &TextStyle) -> Line {
    if matches.is_empty() {
        return Line::one(puri::text(tcx, text, style));
    }
    let bold = TextStyle {
        weight: Some(700.0),
        ..style.clone()
    };
    let mut segments = Vec::new();
    let mut at = 0;
    for span in matches {
        if span.start > at {
            segments.push(puri::text(tcx, &text[at..span.start], style));
        }
        segments.push(puri::text(tcx, &text[span.clone()], &bold));
        at = span.end;
    }
    if at < text.len() {
        segments.push(puri::text(tcx, &text[at..], style));
    }
    Line::from_segments(segments)
}

impl Completion {
    pub fn new<'a>(
        tcx: &mut TextCtx,
        entries: impl IntoIterator<Item = Entry<'a>>,
        show_more: bool,
        style: Style<'_>,
    ) -> Self {
        let mut rows = entries
            .into_iter()
            .map(|entry| {
                let display = highlighted(tcx, entry.display, entry.matches, entry.style);
                let detail = entry
                    .detail
                    .map(|detail| Line::one(puri::text(tcx, detail, style.detail)));
                Row::new(display, detail, 2.0 * style.scale, &style)
            })
            .collect::<Vec<_>>();
        let mut more = show_more.then(|| {
            Row::new(
                Line::one(puri::text(tcx, "…", style.more)),
                None,
                3.0 * style.scale,
                &style,
            )
        });
        let width = rows
            .iter()
            .chain(more.iter())
            .map(|row| row.metrics.width)
            .fold(0.0, f64::max);
        for row in rows.iter_mut().chain(more.iter_mut()) {
            row.metrics.width = width;
        }
        Self { rows, more }
    }
}
