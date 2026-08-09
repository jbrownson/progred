//! Headless SVG bench for tuning drawn delimiters against the system
//! font. Writes target/delimiter_bench.svg: (A) parametric shapes
//! overlaid on the font's own glyph outlines at 8x, (B) a stretch
//! series, (C) mock projection lines with real shaped text. Prints the
//! measured glyph ratios that calibrate `DelimStyle::for_text_size`.

use kurbo::{Affine, BezPath, Rect, Shape};
use parley::layout::PositionedLayoutItem;
use parley::style::GenericFamily;
use parley::{AlignmentOptions, FontContext, Layout, LayoutContext, StyleProperty};
use peniko::Brush;
use puri::delim::{self, Delim, DelimStyle};
use skrifa::instance::{LocationRef, NormalizedCoord, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, GlyphId, MetadataProvider};
use std::fmt::Write as _;

const TEXT_SIZE: f64 = 14.0;
const DIM: &str = "#8C94A3";
const NAME: &str = "#212429";
const LABEL: &str = "#757D8C";
const STRING: &str = "#8C5447";

struct Shaper {
    fonts: FontContext,
    layouts: LayoutContext<Brush>,
}

struct Shaped {
    path: BezPath,
    advance: f64,
}

impl Shaper {
    fn new() -> Self {
        Self {
            fonts: FontContext::new(),
            layouts: LayoutContext::new(),
        }
    }

    /// Outline-fill a single line, baseline at y=0, origin at x=0.
    fn shaped(&mut self, s: &str, family: GenericFamily) -> Shaped {
        let mut builder = self.layouts.ranged_builder(&mut self.fonts, s, 1.0, true);
        builder.push_default(StyleProperty::Brush(Brush::default()));
        builder.push_default(family);
        builder.push_default(StyleProperty::FontSize(TEXT_SIZE as f32));
        let mut layout: Layout<Brush> = builder.build(s);
        layout.break_all_lines(None);
        layout.align(parley::Alignment::Start, AlignmentOptions::default());
        let mut path = BezPath::new();
        let mut advance = 0.0_f64;
        for line in layout.lines() {
            let baseline = line.metrics().baseline;
            advance = advance.max(line.metrics().advance as f64);
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(glyph_run) = item else {
                    continue;
                };
                let run = glyph_run.run();
                let font = run.font().clone();
                let font_ref = FontRef::from_index(font.data.as_ref(), font.index).unwrap();
                let outlines = font_ref.outline_glyphs();
                let coords: Vec<NormalizedCoord> = run
                    .normalized_coords()
                    .iter()
                    .map(|bits| NormalizedCoord::from_bits(*bits))
                    .collect();
                let size = Size::new(run.font_size());
                let mut x = glyph_run.offset();
                for glyph in glyph_run.glyphs() {
                    let mut pen = BezPen {
                        path: &mut path,
                        offset: (
                            (x + glyph.x) as f64,
                            (baseline + glyph.y) as f64 - baseline as f64,
                        ),
                    };
                    if let Some(outline) = outlines.get(GlyphId::new(glyph.id)) {
                        outline
                            .draw(DrawSettings::unhinted(size, LocationRef::new(&coords)), &mut pen)
                            .unwrap();
                    }
                    x += glyph.advance;
                }
            }
        }
        Shaped { path, advance }
    }
}

/// Skrifa outlines are y-up; flip to kurbo's y-down at the baseline.
struct BezPen<'a> {
    path: &'a mut BezPath,
    offset: (f64, f64),
}

impl BezPen<'_> {
    fn pt(&self, x: f32, y: f32) -> (f64, f64) {
        (self.offset.0 + x as f64, self.offset.1 - y as f64)
    }
}

impl OutlinePen for BezPen<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        let p = self.pt(x, y);
        self.path.move_to(p);
    }
    fn line_to(&mut self, x: f32, y: f32) {
        let p = self.pt(x, y);
        self.path.line_to(p);
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        let (c, p) = (self.pt(cx0, cy0), self.pt(x, y));
        self.path.quad_to(c, p);
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        let (c0, c1, p) = (self.pt(cx0, cy0), self.pt(cx1, cy1), self.pt(x, y));
        self.path.curve_to(c0, c1, p);
    }
    fn close(&mut self) {
        self.path.close_path();
    }
}

struct Svg(String);

impl Svg {
    fn fill(&mut self, path: &BezPath, color: &str) {
        writeln!(self.0, r#"<path d="{}" fill="{color}"/>"#, path.to_svg()).unwrap();
    }

    fn stroke(&mut self, path: &BezPath, color: &str, width: f64) {
        writeln!(
            self.0,
            r#"<path d="{}" fill="none" stroke="{color}" stroke-width="{width}" stroke-linecap="round" stroke-linejoin="round"/>"#,
            path.to_svg()
        )
        .unwrap();
    }

    fn rect(&mut self, rect: Rect, color: &str) {
        writeln!(
            self.0,
            r#"<rect x="{}" y="{}" width="{}" height="{}" fill="{color}"/>"#,
            rect.x0,
            rect.y0,
            rect.width(),
            rect.height()
        )
        .unwrap();
    }

    fn caption(&mut self, x: f64, y: f64, text: &str) {
        writeln!(
            self.0,
            r##"<text x="{x}" y="{y}" font-family="system-ui" font-size="13" fill="#666">{text}</text>"##
        )
        .unwrap();
    }

    fn group(&mut self, translate: (f64, f64), scale: f64, content: impl FnOnce(&mut Self)) {
        writeln!(
            self.0,
            r#"<g transform="translate({} {}) scale({scale})">"#,
            translate.0, translate.1
        )
        .unwrap();
        content(self);
        writeln!(self.0, "</g>").unwrap();
    }
}

fn translated(path: &BezPath, x: f64, y: f64) -> BezPath {
    let mut moved = path.clone();
    moved.apply_affine(Affine::translate((x, y)));
    moved
}

fn main() {
    let mut shaper = Shaper::new();
    let style = DelimStyle::for_text_size(TEXT_SIZE);
    let kinds = [
        (Delim::Paren, "(", ")"),
        (Delim::Bracket, "[", "]"),
        (Delim::Brace, "{", "}"),
    ];

    let mut svg = Svg(String::new());
    writeln!(
        svg.0,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="1500" height="1300" viewBox="0 0 1500 1300">"#
    )
    .unwrap();
    svg.rect(Rect::new(0.0, 0.0, 1500.0, 1300.0), "#FFFFFF");

    svg.caption(40.0, 40.0, "A. font glyph (blue fill) vs parametric (red fill) at stem variants, 7x");
    svg.group((60.0, 160.0), 7.0, |svg| {
        for (i, (delim, open_ch, _)) in kinds.iter().enumerate() {
            let glyph = shaper.shaped(open_ch, GenericFamily::SystemUi);
            let bounds = glyph.path.bounding_box();
            println!(
                "{open_ch}  advance {:.3}  ink x [{:.3}, {:.3}] w {:.3}  y [{:.3}, {:.3}] h {:.3}   /size: w {:.3} asc {:.3} desc {:.3} adv {:.3}",
                glyph.advance,
                bounds.x0,
                bounds.x1,
                bounds.width(),
                bounds.y0,
                bounds.y1,
                bounds.height(),
                bounds.width() / TEXT_SIZE,
                -bounds.y0 / TEXT_SIZE,
                bounds.y1 / TEXT_SIZE,
                glyph.advance / TEXT_SIZE,
            );
            for (j, stem_em) in [0.060, 0.0675, 0.075, 0.0825].iter().enumerate() {
                let x = j as f64 * 44.0 + i as f64 * 12.0;
                let fitted = DelimStyle {
                    stem: TEXT_SIZE * stem_em,
                    bow: bounds.width(),
                    brace_bow: bounds.width(),
                    line: TEXT_SIZE * 1.18,
                };
                svg.fill(&translated(&glyph.path, x, 0.0), "#7A9CC6");
                svg.fill(
                    &translated(
                        &delim::open(*delim, &fitted, bounds.y0, bounds.y1),
                        x + bounds.x0,
                         0.0,
                    ),
                    "#D0362CBB",
                );
            }
        }
    });

    svg.caption(40.0, 300.0, "B. stretch series, 2x");
    svg.group((60.0, 340.0), 2.0, |svg| {
        for (i, (delim, _, _)) in kinds.iter().enumerate() {
            let y = i as f64 * 150.0;
            let mut x = 0.0;
            for lines in [1.0, 1.5, 2.0, 3.0, 5.0, 8.0] {
                let height = lines * 17.0;
                let bow = style.bow_for(*delim, height);
                svg.rect(
                    Rect::new(x + bow + 2.0, y, x + bow + 26.0, y + height),
                    "#EEF0F3",
                );
                svg.fill(&translated(&delim::open(*delim, &style, 0.0, height), x, y), DIM);
                svg.fill(
                    &translated(&delim::close(*delim, &style, 0.0, height), x + bow + 28.0, y),
                    DIM,
                );
                x += bow * 2.0 + 40.0;
            }
        }
    });

    svg.caption(620.0, 300.0, "C. mock projection lines, 3.2x");
    svg.group((620.0, 400.0), 3.2, |svg| {
        let pitch = 18.0;
        let gap = 3.0;
        let span = (-TEXT_SIZE * 0.704, TEXT_SIZE * 0.171);
        let flat = |svg: &mut Svg, x: &mut f64, y: f64, delim: Delim, open: bool| {
            let path = if open {
                delim::open(delim, &style, span.0, span.1)
            } else {
                delim::close(delim, &style, span.0, span.1)
            };
            svg.fill(&translated(&path, *x, y), DIM);
            *x += style.bow(delim) + gap;
        };
        let text = |svg: &mut Svg, shaper: &mut Shaper, x: &mut f64, y: f64, s: &str, family: GenericFamily, color: &str| {
            let shaped = shaper.shaped(s, family);
            svg.fill(&translated(&shaped.path, *x, y), color);
            *x += shaped.advance;
        };

        let mut x = 0.0;
        text(svg, &mut shaper, &mut x, 0.0, "points: ", GenericFamily::SystemUi, LABEL);
        flat(svg, &mut x, 0.0, Delim::Bracket, true);
        flat(svg, &mut x, 0.0, Delim::Paren, true);
        text(svg, &mut shaper, &mut x, 0.0, "origin", GenericFamily::SystemUi, NAME);
        flat(svg, &mut x, 0.0, Delim::Paren, false);
        text(svg, &mut shaper, &mut x, 0.0, " ", GenericFamily::SystemUi, NAME);
        flat(svg, &mut x, 0.0, Delim::Paren, true);
        text(svg, &mut shaper, &mut x, 0.0, "corner", GenericFamily::SystemUi, NAME);
        flat(svg, &mut x, 0.0, Delim::Paren, false);
        flat(svg, &mut x, 0.0, Delim::Bracket, false);

        let (row1, row2) = (40.0, 40.0 + pitch);
        let (top, bottom) = (row1 + span.0, row2 + span.1);
        let mut x = 0.0;
        text(svg, &mut shaper, &mut x, row1, "style: ", GenericFamily::SystemUi, LABEL);
        svg.fill(&translated(&delim::open(Delim::Paren, &style, top, bottom), x, 0.0), DIM);
        x += style.bow(Delim::Paren) + gap;
        text(svg, &mut shaper, &mut x, row1, "style ", GenericFamily::SystemUi, NAME);
        svg.fill(&translated(&delim::open(Delim::Brace, &style, top, bottom), x, 0.0), DIM);
        x += style.bow(Delim::Brace) + gap;
        let body = x;
        let mut widest = x;
        for (y, label, value, color) in [
            (row1, "stroke: ", "hairline", NAME),
            (row2, "swatch: ", "0x663399", STRING),
        ] {
            let mut x = body;
            text(svg, &mut shaper, &mut x, y, label, GenericFamily::SystemUi, LABEL);
            if y == row1 {
                flat(svg, &mut x, y, Delim::Paren, true);
            }
            let family = if value.starts_with("0x") {
                GenericFamily::Monospace
            } else {
                GenericFamily::SystemUi
            };
            text(svg, &mut shaper, &mut x, y, value, family, color);
            if y == row1 {
                flat(svg, &mut x, y, Delim::Paren, false);
            }
            widest = widest.max(x);
        }
        let mut x = widest + gap;
        svg.fill(&translated(&delim::close(Delim::Brace, &style, top, bottom), x, 0.0), DIM);
        x += style.bow(Delim::Brace) + gap;
        svg.fill(&translated(&delim::close(Delim::Paren, &style, top, bottom), x, 0.0), DIM);
    });

    writeln!(svg.0, "</svg>").unwrap();
    let out = "target/delimiter_bench.svg";
    std::fs::write(out, svg.0).unwrap();
    println!("wrote {out}");
}
