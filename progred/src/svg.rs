//! An SVG writer for draw lists, and the captures that use it: documents
//! projected through the frame harness and whole editor frames, written to the
//! build directory.

use crate::projection::test_support::{Bench, place};
use crate::selection::Selection;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use gid::Document;
use kurbo::{Affine, BezPath, Point, Shape as KurboShape};
use measured::Extent;
use peniko::{
    Brush, Color, Extend, GradientKind, ImageAlphaType, ImageData, ImageFormat, color::Srgb,
};
use puri::draw::{DrawCmd, DrawList, Shape};
use skrifa::instance::{LocationRef, NormalizedCoord, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, GlyphId, MetadataProvider};
use std::fmt::Write as _;

mod documents;
mod editor;
mod refined;
mod tests;

fn image_png(image: &ImageData) -> Vec<u8> {
    assert_eq!(
        Some(image.data.as_ref().len()),
        image.format.size_in_bytes(image.width, image.height)
    );
    crate::libraries::png::encode(
        image.width,
        image.height,
        &puri::draw::straight_rgba(image).expect("unsupported SVG image format"),
    )
    .unwrap()
}

fn svg_transform(transform: Affine) -> String {
    let [a, b, c, d, e, f] = transform.as_coeffs();
    format!("matrix({a} {b} {c} {d} {e} {f})")
}

fn css([r, g, b, a]: [f32; 4]) -> String {
    format!(
        "rgba({},{},{},{:.3})",
        (r * 255.0).round(),
        (g * 255.0).round(),
        (b * 255.0).round(),
        a
    )
}

fn svg_brush(out: &mut String, brush: &Brush, transform: Affine, next_id: &mut usize) -> String {
    let gradient = match brush {
        Brush::Solid(color) => return css(color.components),
        Brush::Gradient(gradient) => gradient,
        Brush::Image(_) => panic!("SVG capture does not support image brushes"),
    };
    let GradientKind::Linear(linear) = gradient.kind else {
        panic!("SVG capture currently supports only linear gradients");
    };
    assert_eq!(
        gradient.interpolation_cs,
        peniko::color::ColorSpaceTag::Srgb
    );
    let id = *next_id;
    *next_id += 1;
    let spread = match gradient.extend {
        Extend::Pad => "pad",
        Extend::Repeat => "repeat",
        Extend::Reflect => "reflect",
    };
    writeln!(
        out,
        r#"<defs><linearGradient id="paint{id}" gradientUnits="userSpaceOnUse" x1="{}" y1="{}" x2="{}" y2="{}" gradientTransform="{}" spreadMethod="{spread}" color-interpolation="sRGB">"#,
        linear.start.x, linear.start.y, linear.end.x, linear.end.y, svg_transform(transform),
    ).unwrap();
    for stop in gradient.stops.iter() {
        writeln!(
            out,
            r#"<stop offset="{}" stop-color="{}"/>"#,
            stop.offset,
            css(stop.color.to_alpha_color::<Srgb>().components),
        )
        .unwrap();
    }
    writeln!(out, "</linearGradient></defs>").unwrap();
    format!("url(#paint{id})")
}

struct BezPen {
    path: BezPath,
    offset: Point,
}

impl OutlinePen for BezPen {
    fn move_to(&mut self, x: f32, y: f32) {
        self.path
            .move_to((self.offset.x + x as f64, self.offset.y - y as f64));
    }
    fn line_to(&mut self, x: f32, y: f32) {
        self.path
            .line_to((self.offset.x + x as f64, self.offset.y - y as f64));
    }
    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.path.quad_to(
            (self.offset.x + cx0 as f64, self.offset.y - cy0 as f64),
            (self.offset.x + x as f64, self.offset.y - y as f64),
        );
    }
    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.path.curve_to(
            (self.offset.x + cx0 as f64, self.offset.y - cy0 as f64),
            (self.offset.x + cx1 as f64, self.offset.y - cy1 as f64),
            (self.offset.x + x as f64, self.offset.y - y as f64),
        );
    }
    fn close(&mut self) {
        self.path.close_path();
    }
}

fn svg_shape(shape: &Shape, transform: Affine) -> String {
    let mut path = match shape {
        Shape::Rect(rect) => rect.to_path(0.05),
        Shape::RoundedRect(rect) => rect.to_path(0.05),
        Shape::Circle(circle) => circle.to_path(0.05),
        Shape::Line(line) => {
            let mut p = BezPath::new();
            p.move_to(line.p0);
            p.line_to(line.p1);
            p
        }
        Shape::Path(path) => path.clone(),
    };
    path.apply_affine(transform);
    path.to_svg()
}

pub(crate) fn write_cmds(out: &mut String, cmds: &[DrawCmd]) {
    write_clipped_cmds(out, cmds, &mut 0);
}

fn write_clipped_cmds(out: &mut String, cmds: &[DrawCmd], next_id: &mut usize) {
    for cmd in cmds {
        match cmd {
            DrawCmd::Mesh { scene, transform } => {
                let image = scene.rasterize().expect("valid mesh viewport");
                write_clipped_cmds(
                    out,
                    &[DrawCmd::Image {
                        image,
                        transform: *transform,
                    }],
                    next_id,
                );
            }
            DrawCmd::Image { image, transform } => writeln!(
                out,
                r#"<image width="{}" height="{}" transform="{}" href="data:image/png;base64,{}"/>"#,
                image.width,
                image.height,
                svg_transform(*transform),
                BASE64.encode(image_png(image))
            )
            .unwrap(),
            DrawCmd::Fill {
                shape,
                brush,
                transform,
            } => {
                let paint = svg_brush(out, brush, *transform, next_id);
                writeln!(
                    out,
                    r#"<path d="{}" fill="{}"/>"#,
                    svg_shape(shape, *transform),
                    paint
                )
                .unwrap();
            }
            DrawCmd::Stroke {
                shape,
                style,
                brush,
                transform,
            } => {
                let paint = svg_brush(out, brush, *transform, next_id);
                writeln!(
                    out,
                    r#"<path d="{}" fill="none" stroke="{}" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round"/>"#,
                    svg_shape(shape, *transform),
                    paint,
                    style.width
                )
                .unwrap();
            }
            DrawCmd::GlyphRun(run) => {
                let Ok(font_ref) = FontRef::from_index(run.font.data.as_ref(), run.font.index)
                else {
                    continue;
                };
                let outlines = font_ref.outline_glyphs();
                let coords: Vec<NormalizedCoord> = run
                    .normalized_coords
                    .iter()
                    .map(|bits| NormalizedCoord::from_bits(*bits))
                    .collect();
                let size = Size::new(run.size);
                let mut path = BezPath::new();
                for glyph in &run.glyphs {
                    let mut pen = BezPen {
                        path: std::mem::take(&mut path),
                        offset: run.transform * Point::new(glyph.x as f64, glyph.y as f64),
                    };
                    if let Some(outline) = outlines.get(GlyphId::new(glyph.id)) {
                        let settings = DrawSettings::unhinted(size, LocationRef::new(&coords));
                        let _ = outline.draw(settings, &mut pen);
                    }
                    path = pen.path;
                }
                let paint = svg_brush(out, &run.brush, run.transform, next_id);
                writeln!(out, r#"<path d="{}" fill="{}"/>"#, path.to_svg(), paint).unwrap();
            }
            DrawCmd::Clip {
                shape,
                transform,
                children,
            } => {
                let id = *next_id;
                *next_id += 1;
                writeln!(out,
                        r##"<defs><clipPath id="clip{id}" clipPathUnits="userSpaceOnUse"><path d="{}"/></clipPath></defs><g clip-path="url(#clip{id})">"##,
                        svg_shape(shape, *transform)
                    ).unwrap();
                write_clipped_cmds(out, children, next_id);
                writeln!(out, "</g>").unwrap();
            }
        }
    }
}

/// `doc` placed as the frame harness places it and written to `out_path`;
/// what was placed, for checks of the drawing.
fn render(
    doc: &Document,
    selection: Option<&Selection>,
    width: f64,
    out_path: &str,
) -> (Bench, Extent) {
    let (bench, extent) = place(doc, selection, width);
    let (width, height) = (width.max(extent.width + 48.0), extent.height() + 48.0);
    write_svg(&bench.list, width, height, out_path);
    (bench, extent)
}

fn write_svg(list: &DrawList, width: f64, height: f64, out_path: &str) {
    write_svg_on(
        list,
        width,
        height,
        crate::display::widget::style::Theme::Light.palette().paper,
        out_path,
    );
}

fn write_svg_on(list: &DrawList, width: f64, height: f64, paper: Color, out_path: &str) {
    let mut out = String::new();
    let background = css(paper.components);
    writeln!(
            out,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width:.0}" height="{height:.0}" viewBox="0 0 {width:.0} {height:.0}">"#
        )
        .unwrap();
    writeln!(
        out,
        r#"<rect width="{width:.0}" height="{height:.0}" fill="{background}"/>"#
    )
    .unwrap();
    write_cmds(&mut out, &list.0);
    writeln!(out, "</svg>").unwrap();
    std::fs::write(
        std::env::var_os("CARGO_TARGET_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("../target"))
            .join(out_path),
        out,
    )
    .unwrap();
}

fn render_editor(editor: crate::Editor, size: kurbo::Size, out_path: &str) {
    let paper = editor.palette.paper;
    let mut runner = crate::EditorRunner::new(editor);
    let paint = runner.prepare_paint(1.0, size);
    let mut list = DrawList::new();
    puri::frame::render(paint.renders, &mut list);
    write_svg_on(&list, size.width, size.height, paper, out_path);
}
