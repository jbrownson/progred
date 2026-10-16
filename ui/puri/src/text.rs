//! Text descriptions measured by Parley and placed by their caller.
//! Styles are in logical units; metrics come out in physical pixels
//! via the context's display scale.

use crate::draw::{CanvasSink, Glyph, GlyphRun};
use crate::geometry::Placement;
use kurbo::{Affine, Line, Stroke};
use parley::layout::{Alignment, Layout, PositionedLayoutItem};
use parley::style::FontWeight;
pub use parley::style::GenericFamily;
use parley::{AlignmentOptions, StyleProperty};
pub use parley::{FontContext, LayoutContext};
use peniko::Brush;
use std::collections::HashMap;
use std::rc::Rc;

pub struct TextCtx<'a> {
    pub fonts: &'a mut FontContext,
    pub layouts: &'a mut LayoutContext<Brush>,
    pub scale: f32,
    pub cache: &'a mut TextCache,
}

/// A shaping memo, caller-owned and caller-swept — the anticipated
/// memo table for a pure function, never hidden state. Keys carry
/// the full style identity, so an entry can never be WRONG, only
/// unused; retention is mark-and-sweep by frame: [`TextCache::sweep`]
/// at the top of each pass drops entries the previous pass never
/// touched and resets the marks, so the steady state is exactly the
/// text on screen, shaped once.
#[derive(Default)]
pub struct TextCache(HashMap<TextKey, CacheEntry>);

struct CacheEntry {
    layout: Rc<Layout<Brush>>,
    used: bool,
}

impl TextCache {
    pub fn sweep(&mut self) {
        self.0
            .retain(|_, entry| std::mem::replace(&mut entry.used, false));
    }
}

#[derive(PartialEq, Eq, Hash)]
pub struct TextKey {
    text: String,
    size: u32,
    weight: Option<u32>,
    family: GenericFamily,
    color: [u32; 4],
    scale: u32,
}

/// Only solid brushes key — anything fancier shapes uncached.
fn text_key(s: &str, style: &TextStyle, scale: f32) -> Option<TextKey> {
    let Brush::Solid(color) = &style.brush else {
        return None;
    };
    Some(TextKey {
        text: s.to_owned(),
        size: style.size.to_bits(),
        weight: style.weight.map(f32::to_bits),
        family: style.family,
        color: color.components.map(f32::to_bits),
        scale: scale.to_bits(),
    })
}

#[derive(Debug, Clone)]
pub struct TextStyle {
    pub size: f32,
    pub brush: Brush,
    pub weight: Option<f32>,
    pub family: GenericFamily,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Script {
    #[default]
    Normal,
    Subscript,
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct TextMetrics {
    pub width: f64,
    pub ascent: f64,
    pub descent: f64,
}

pub struct Text {
    layout: Rc<Layout<Brush>>,
    metrics: TextMetrics,
}

impl Text {
    pub fn metrics(&self) -> TextMetrics {
        self.metrics
    }

    pub fn place(self, canvas: &mut (impl CanvasSink + ?Sized), placement: Placement) {
        draw_layout(
            canvas,
            &self.layout,
            Affine::translate((placement.rect.x0, placement.rect.y0)),
        );
    }
}

/// Single line, no wrapping; width includes trailing whitespace so
/// inline fragments compose without collapsing.
pub fn text(ctx: &mut TextCtx, s: &str, style: &TextStyle) -> Text {
    measured_text(build_layout(ctx, s, style))
}

/// Script glyphs retain the surrounding baseline for alignment; their
/// metrics account for the smaller font and displaced glyph baseline.
pub fn scripted_text(ctx: &mut TextCtx, s: &str, style: &TextStyle, script: Script) -> Text {
    match script {
        Script::Normal => text(ctx, s, style),
        Script::Subscript => {
            let text = text(
                ctx,
                s,
                &TextStyle {
                    size: style.size * 0.70,
                    ..style.clone()
                },
            );
            let offset = f64::from(style.size * 0.23 * ctx.scale);
            Text {
                metrics: TextMetrics {
                    ascent: text.metrics.ascent - offset,
                    descent: text.metrics.descent + offset,
                    ..text.metrics
                },
                ..text
            }
        }
    }
}

/// The single-line layout [`text`] draws for `s`, shared through the
/// cache — for callers that hit-test a text leaf after the fact.
pub fn line_layout(ctx: &mut TextCtx, s: &str, style: &TextStyle) -> Rc<Layout<Brush>> {
    build_layout(ctx, s, style)
}

pub(crate) fn build_layout(ctx: &mut TextCtx, s: &str, style: &TextStyle) -> Rc<Layout<Brush>> {
    let key = text_key(s, style, ctx.scale);
    if let Some(key) = &key
        && let Some(hit) = ctx.cache.0.get_mut(key)
    {
        hit.used = true;
        return Rc::clone(&hit.layout);
    }
    let mut builder = ctx.layouts.ranged_builder(ctx.fonts, s, ctx.scale, true);
    builder.push_default(StyleProperty::Brush(style.brush.clone()));
    builder.push_default(style.family);
    builder.push_default(StyleProperty::FontSize(style.size));
    if let Some(weight) = style.weight {
        builder.push_default(StyleProperty::FontWeight(FontWeight::new(weight)));
    }
    let mut layout: Layout<Brush> = builder.build(s);
    layout.break_all_lines(None);
    layout.align(Alignment::Start, AlignmentOptions::default());
    let layout = Rc::new(layout);
    if let Some(key) = key {
        ctx.cache.0.insert(
            key,
            CacheEntry {
                layout: Rc::clone(&layout),
                used: true,
            },
        );
    }
    layout
}

fn measured_text(layout: Rc<Layout<Brush>>) -> Text {
    let first = layout.lines().next().map(|line| *line.metrics());
    let baseline = first.map(|m| m.baseline as f64).unwrap_or(0.0);
    let metrics = TextMetrics {
        width: first.map(|m| m.advance as f64).unwrap_or(0.0),
        ascent: baseline,
        descent: layout.height() as f64 - baseline,
    };
    Text { layout, metrics }
}

pub fn draw_layout(
    canvas: &mut (impl CanvasSink + ?Sized),
    layout: &Layout<Brush>,
    transform: Affine,
) {
    for line in layout.lines() {
        for item in line.items() {
            let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                continue;
            };
            let style = glyph_run.style();
            if let Some(underline) = &style.underline {
                let run_metrics = glyph_run.run().metrics();
                let offset = underline.offset.unwrap_or(run_metrics.underline_offset);
                let width = underline.size.unwrap_or(run_metrics.underline_size);
                let y = glyph_run.baseline() - offset + width / 2.0;
                canvas.stroke_shape(
                    Line::new(
                        (glyph_run.offset() as f64, y as f64),
                        ((glyph_run.offset() + glyph_run.advance()) as f64, y as f64),
                    )
                    .into(),
                    Stroke::new(width.into()),
                    underline.brush.clone(),
                    transform,
                );
            }
            let mut x = glyph_run.offset();
            let y = glyph_run.baseline();
            let run = glyph_run.run();
            let glyph_xform = run
                .synthesis()
                .skew()
                .map(|angle| Affine::skew(angle.to_radians().tan() as f64, 0.0));
            canvas.draw_glyphs(GlyphRun {
                font: run.font().clone(),
                size: run.font_size(),
                glyphs: glyph_run
                    .glyphs()
                    .map(|glyph| {
                        let gx = x + glyph.x;
                        let gy = y + glyph.y;
                        x += glyph.advance;
                        Glyph {
                            id: glyph.id,
                            x: gx,
                            y: gy,
                        }
                    })
                    .collect(),
                normalized_coords: run.normalized_coords().to_vec(),
                brush: style.brush.clone(),
                hint: true,
                transform,
                glyph_transform: glyph_xform,
            });
        }
    }
}

#[cfg(test)]
mod tests;
