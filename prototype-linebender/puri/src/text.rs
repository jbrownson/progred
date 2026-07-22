//! Text leaves measured by parley. Styles are in logical units; extents
//! come out in physical pixels via the context's display scale.

use crate::draw::{Canvas, Glyph, GlyphRun};
use crate::layout::{Extent, Node, leaf};
use kurbo::{Affine, Line, Point, Stroke};
use parley::layout::{Alignment, Layout, PositionedLayoutItem};
use parley::style::{FontWeight, GenericFamily};
use parley::{AlignmentOptions, Cursor, FontContext, LayoutContext, LineHeight, StyleProperty};
use peniko::Brush;
use std::collections::HashMap;

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
    layout: Layout<Brush>,
    used: bool,
}

impl TextCache {
    pub fn sweep(&mut self) {
        self.0.retain(|_, entry| std::mem::replace(&mut entry.used, false));
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

/// Single line, no wrapping; width includes trailing whitespace so
/// inline fragments compose without collapsing.
pub fn text<P: Canvas>(ctx: &mut TextCtx, s: &str, style: &TextStyle) -> Node<P> {
    layout_node(build_layout(ctx, s, style, None, None), true)
}

/// Wrapped to `max_width`; the baseline is the first line's.
pub fn paragraph<P: Canvas>(
    ctx: &mut TextCtx,
    s: &str,
    style: &TextStyle,
    line_height: f32,
    max_width: f32,
) -> Node<P> {
    layout_node(
        build_layout(ctx, s, style, Some(line_height), Some(max_width)),
        false,
    )
}

/// The single-line layout [`text`] draws for `s`, shared through the
/// cache — for callers that hit-test a text leaf after the fact.
pub fn line_layout(ctx: &mut TextCtx, s: &str, style: &TextStyle) -> Layout<Brush> {
    build_layout(ctx, s, style, None, None)
}

/// The caret boundary nearest `point` (leaf-local, physical pixels),
/// as a byte index into the laid-out text.
pub fn caret_index(layout: &Layout<Brush>, point: Point) -> usize {
    Cursor::from_point(layout, point.x as f32, point.y as f32).index()
}

pub(crate) fn build_layout(
    ctx: &mut TextCtx,
    s: &str,
    style: &TextStyle,
    line_height: Option<f32>,
    max_width: Option<f32>,
) -> Layout<Brush> {
    let key = (line_height.is_none() && max_width.is_none())
        .then(|| text_key(s, style, ctx.scale))
        .flatten();
    if let Some(key) = &key
        && let Some(hit) = ctx.cache.0.get_mut(key)
    {
        hit.used = true;
        return hit.layout.clone();
    }
    let mut builder = ctx.layouts.ranged_builder(ctx.fonts, s, ctx.scale, true);
    builder.push_default(StyleProperty::Brush(style.brush.clone()));
    builder.push_default(style.family);
    builder.push_default(StyleProperty::FontSize(style.size));
    if let Some(weight) = style.weight {
        builder.push_default(StyleProperty::FontWeight(FontWeight::new(weight)));
    }
    if let Some(line_height) = line_height {
        builder.push_default(LineHeight::FontSizeRelative(line_height));
    }
    let mut layout: Layout<Brush> = builder.build(s);
    layout.break_all_lines(max_width);
    layout.align(Alignment::Start, AlignmentOptions::default());
    if let Some(key) = key {
        ctx.cache.0.insert(
            key,
            CacheEntry {
                layout: layout.clone(),
                used: true,
            },
        );
    }
    layout
}

fn layout_node<P: Canvas>(layout: Layout<Brush>, include_trailing_whitespace: bool) -> Node<P> {
    let first = layout.lines().next().map(|line| *line.metrics());
    let baseline = first.map(|m| m.baseline as f64).unwrap_or(0.0);
    let width = if include_trailing_whitespace {
        first.map(|m| m.advance as f64).unwrap_or(0.0)
    } else {
        layout.width() as f64
    };
    let extent = Extent {
        width,
        ascent: baseline,
        descent: layout.height() as f64 - baseline,
    };
    leaf(extent, move |canvas: &mut P, at: Point| {
        draw_layout(
            canvas,
            &layout,
            Affine::translate((at.x, at.y - baseline)),
        );
    })
}

pub fn draw_layout(canvas: &mut impl Canvas, layout: &Layout<Brush>, transform: Affine) {
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
                canvas.stroke(
                    Line::new(
                        (glyph_run.offset() as f64, y as f64),
                        ((glyph_run.offset() + glyph_run.advance()) as f64, y as f64),
                    ),
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
            canvas.glyph_run(GlyphRun {
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
mod tests {
    use super::*;
    use crate::draw::{DrawCmd, DrawList};
    use crate::layout::{place, row};
    use peniko::Color;

    #[test]
    fn text_of_different_sizes_shares_a_baseline_in_a_row() {
        let mut fonts = FontContext::new();
        let mut layouts = LayoutContext::new();
        let mut cache = TextCache::default();
        let mut ctx = TextCtx {
            fonts: &mut fonts,
            layouts: &mut layouts,
            scale: 1.0,
            cache: &mut cache,
        };
        let big = TextStyle {
            size: 28.0,
            brush: Color::WHITE.into(),
            weight: None,
            family: GenericFamily::SystemUi,
        };
        let small = TextStyle {
            size: 12.0,
            brush: Color::WHITE.into(),
            weight: None,
            family: GenericFamily::SystemUi,
        };
        let r = row(4.0, vec![text(&mut ctx, "big", &big), text(&mut ctx, "small", &small)]);

        let mut recording = DrawList::new();
        place(r, &mut recording, Point::new(0.0, 100.0));

        let baselines: Vec<f64> = recording
            .0
            .iter()
            .filter_map(|cmd| match cmd {
                DrawCmd::GlyphRun(run) => Some(
                    run.transform.translation().y + run.glyphs.first()?.y as f64,
                ),
                _ => None,
            })
            .collect();
        assert!(baselines.len() >= 2);
        assert!(
            baselines.iter().all(|y| (y - 100.0).abs() < 1.0),
            "baselines: {baselines:?}"
        );
    }
}
