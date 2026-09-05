//! Text descriptions measured by Parley and placed by their caller.
//! Styles are in logical units; metrics come out in physical pixels
//! via the context's display scale.

use crate::draw::{Canvas, Glyph, GlyphRun};
use crate::geometry::Placement;
use kurbo::{Affine, Line, Point, Stroke};
use parley::layout::{Alignment, Layout, PositionedLayoutItem};
use parley::style::{FontWeight, GenericFamily};
use parley::{AlignmentOptions, Cursor, FontContext, LayoutContext, LineHeight, StyleProperty};
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

    pub fn place(self, canvas: &mut impl Canvas, placement: Placement) {
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
    measured_text(build_layout(ctx, s, style, None, None), true)
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

/// Wrapped to `max_width`; the baseline is the first line's.
pub fn paragraph(
    ctx: &mut TextCtx,
    s: &str,
    style: &TextStyle,
    line_height: f32,
    max_width: f32,
) -> Text {
    measured_text(
        build_layout(ctx, s, style, Some(line_height), Some(max_width)),
        false,
    )
}

/// The single-line layout [`text`] draws for `s`, shared through the
/// cache — for callers that hit-test a text leaf after the fact.
pub fn line_layout(ctx: &mut TextCtx, s: &str, style: &TextStyle) -> Rc<Layout<Brush>> {
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
) -> Rc<Layout<Brush>> {
    let key = (line_height.is_none() && max_width.is_none())
        .then(|| text_key(s, style, ctx.scale))
        .flatten();
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
    if let Some(line_height) = line_height {
        builder.push_default(LineHeight::FontSizeRelative(line_height));
    }
    let mut layout: Layout<Brush> = builder.build(s);
    layout.break_all_lines(max_width);
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

fn measured_text(layout: Rc<Layout<Brush>>, include_trailing_whitespace: bool) -> Text {
    let first = layout.lines().next().map(|line| *line.metrics());
    let baseline = first.map(|m| m.baseline as f64).unwrap_or(0.0);
    let width = if include_trailing_whitespace {
        first.map(|m| m.advance as f64).unwrap_or(0.0)
    } else {
        layout.width() as f64
    };
    let metrics = TextMetrics {
        width,
        ascent: baseline,
        descent: layout.height() as f64 - baseline,
    };
    Text { layout, metrics }
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
    use kurbo::Rect;
    use peniko::Color;

    #[test]
    fn cached_line_layouts_share_the_shaped_result() {
        let mut fonts = FontContext::new();
        let mut layouts = LayoutContext::new();
        let mut cache = TextCache::default();
        let mut ctx = TextCtx {
            fonts: &mut fonts,
            layouts: &mut layouts,
            scale: 1.0,
            cache: &mut cache,
        };
        let style = TextStyle {
            size: 16.0,
            brush: Color::WHITE.into(),
            weight: None,
            family: GenericFamily::SystemUi,
        };

        let first = line_layout(&mut ctx, "shared", &style);
        let second = line_layout(&mut ctx, "shared", &style);

        assert!(Rc::ptr_eq(&first, &second));
    }

    #[test]
    fn subscript_metrics_preserve_the_alignment_baseline_and_cover_the_shaped_line() {
        let mut fonts = FontContext::new();
        let mut layouts = LayoutContext::new();
        let mut cache = TextCache::default();
        for scale in [1.0, 1.5, 2.0] {
            let mut ctx = TextCtx {
                fonts: &mut fonts,
                layouts: &mut layouts,
                scale,
                cache: &mut cache,
            };
            let style = TextStyle {
                size: 13.0,
                brush: Color::WHITE.into(),
                weight: None,
                family: GenericFamily::SystemUi,
            };
            let normal = scripted_text(&mut ctx, "f32", &style, Script::Normal);
            let subscript = scripted_text(&mut ctx, "f32", &style, Script::Subscript);
            let small = text(
                &mut ctx,
                "f32",
                &TextStyle {
                    size: 13.0 * 0.70,
                    ..style
                },
            );
            assert!(Rc::ptr_eq(&subscript.layout, &small.layout));
            let metrics = subscript.metrics();
            assert!(metrics.width < normal.metrics().width);
            let offset = f64::from(13.0 * 0.23 * scale);
            assert!((metrics.ascent + offset - small.metrics().ascent).abs() < 1e-6);
            assert!((metrics.descent - offset - small.metrics().descent).abs() < 1e-6);

            let rect = Rect::new(
                0.0,
                100.0 - metrics.ascent,
                metrics.width,
                100.0 + metrics.descent,
            );
            assert!((rect.height() - subscript.layout.height() as f64).abs() < 1e-6);
            let mut recording = DrawList::new();
            subscript.place(&mut recording, Placement::root(rect));
            let run = recording
                .0
                .iter()
                .find_map(|cmd| match cmd {
                    DrawCmd::GlyphRun(run) => Some(run),
                    _ => None,
                })
                .unwrap();
            let glyph_baseline =
                run.transform.translation().y + f64::from(run.glyphs.first().unwrap().y);
            assert!((glyph_baseline - (100.0 + offset)).abs() < 1e-6);
        }
    }

    #[test]
    fn text_metrics_support_a_shared_baseline() {
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
        let big = text(&mut ctx, "big", &big);
        let small = text(&mut ctx, "small", &small);

        let mut recording = DrawList::new();
        let big_rect = Rect::new(
            0.0,
            100.0 - big.metrics().ascent,
            big.metrics().width,
            100.0 + big.metrics().descent,
        );
        let small_rect = Rect::new(
            big.metrics().width + 4.0,
            100.0 - small.metrics().ascent,
            big.metrics().width + 4.0 + small.metrics().width,
            100.0 + small.metrics().descent,
        );
        big.place(&mut recording, Placement::root(big_rect));
        small.place(&mut recording, Placement::root(small_rect));

        let baselines: Vec<f64> = recording
            .0
            .iter()
            .filter_map(|cmd| match cmd {
                DrawCmd::GlyphRun(run) => {
                    Some(run.transform.translation().y + run.glyphs.first()?.y as f64)
                }
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
