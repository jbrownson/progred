//! Shaped completion rows. The consumer composes them with its layout,
//! scroll container, hover targets, and popup policy.

use puri::draw::CanvasSink;
use puri::{Affine, Color, Placement, Rect, RoundedRect, Text, TextCtx, TextMetrics, TextStyle};

use std::ops::Range;

pub struct Entry<'a> {
    pub display: &'a str,
    /// Where in `display` its subscripts are.
    pub subscripts: &'a [Range<usize>],
    /// What kind of thing it is, read dim just after it.
    pub kind: Option<&'a str>,
    pub detail: Option<&'a str>,
    pub matches: &'a [Range<usize>],
    pub style: &'a TextStyle,
}

/// The room between an entry and its kind, before scaling.
const KIND_GAP: f64 = 6.0;

pub struct Style<'a> {
    pub detail: &'a TextStyle,
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
    kind: Option<Line>,
    detail: Option<Line>,
    metrics: TextMetrics,
    scale: f64,
    chosen: Color,
}

impl Row {
    fn new(display: Line, kind: Option<Line>, detail: Option<Line>, style: &Style<'_>) -> Self {
        let padding = 2.0 * style.scale;
        let lines = || std::iter::once(&display).chain(&kind).chain(&detail);
        Self {
            metrics: TextMetrics {
                width: display.metrics.width
                    + kind
                        .as_ref()
                        .map_or(0.0, |kind| KIND_GAP * style.scale + kind.metrics.width)
                    + detail
                        .as_ref()
                        .map_or(0.0, |detail| 8.0 * style.scale + detail.metrics.width)
                    + 16.0 * style.scale,
                ascent: lines().map(|line| line.metrics.ascent).fold(0.0, f64::max) + padding,
                descent: lines().map(|line| line.metrics.descent).fold(0.0, f64::max) + padding,
            },
            display,
            kind,
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
        let after = x + self.display.metrics.width + KIND_GAP * self.scale;
        self.display.draw(canvas, x, baseline, placement.clip_rect);
        if let Some(kind) = self.kind {
            kind.draw(canvas, after, baseline, placement.clip_rect);
        }
        if let Some(detail) = self.detail {
            let detail_x = placement.rect.x1 - 8.0 * self.scale - detail.metrics.width;
            detail.draw(canvas, detail_x, baseline, placement.clip_rect);
        }
    }
}

/// `text` in `style`, bold where it `matches` and, where it's one of its
/// `subscripts`, a subscript in `dim`'s brush.
fn highlighted(
    tcx: &mut TextCtx,
    text: &str,
    matches: &[Range<usize>],
    subscripts: &[Range<usize>],
    style: &TextStyle,
    dim: &TextStyle,
) -> Line {
    if matches.is_empty() && subscripts.is_empty() {
        return Line::one(puri::text(tcx, text, style));
    }
    let mut cuts: Vec<usize> = matches
        .iter()
        .chain(subscripts)
        .flat_map(|span| [span.start, span.end])
        .chain([0, text.len()])
        .collect();
    cuts.sort_unstable();
    cuts.dedup();
    let within = |spans: &[Range<usize>], at| spans.iter().any(|span| span.contains(&at));
    Line::from_segments(
        cuts.windows(2)
            .map(|cut| {
                let subscript = within(subscripts, cut[0]);
                let style = TextStyle {
                    weight: within(matches, cut[0]).then_some(700.0).or(style.weight),
                    brush: if subscript { &dim.brush } else { &style.brush }.clone(),
                    ..style.clone()
                };
                let script = if subscript {
                    puri::Script::Subscript
                } else {
                    puri::Script::Normal
                };
                puri::scripted_text(tcx, &text[cut[0]..cut[1]], &style, script)
            })
            .collect(),
    )
}

/// A row for each of `entries`, all as wide as the widest.
pub fn rows<'a>(
    tcx: &mut TextCtx,
    entries: impl IntoIterator<Item = Entry<'a>>,
    style: Style<'_>,
) -> Vec<Row> {
    let mut rows = entries
        .into_iter()
        .map(|entry| {
            let display = highlighted(
                tcx,
                entry.display,
                entry.matches,
                entry.subscripts,
                entry.style,
                style.detail,
            );
            let kind = entry
                .kind
                .map(|kind| Line::one(puri::text(tcx, kind, style.detail)));
            let detail = entry
                .detail
                .map(|detail| Line::one(puri::text(tcx, detail, style.detail)));
            Row::new(display, kind, detail, &style)
        })
        .collect::<Vec<_>>();
    let width = rows.iter().map(|row| row.metrics.width).fold(0.0, f64::max);
    for row in &mut rows {
        row.metrics.width = width;
    }
    rows
}
