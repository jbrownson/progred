//! The browser backend: a Puri `Canvas` streaming into Canvas2D.

#![cfg(target_arch = "wasm32")]

use js_sys::Array;
use kurbo::{Affine, Cap, Join, PathEl, Shape as KurboShape, Stroke};
use peniko::color::Srgb;
use peniko::{Brush, GradientKind, ImageAlphaType, ImageData, ImageFormat};
use puri::draw::{GlyphRun, Shape};
use skrifa::instance::{LocationRef, NormalizedCoord, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, GlyphId, MetadataProvider};
use std::borrow::Cow;
use wasm_bindgen::{Clamped, JsCast, JsValue};
use web_sys::{CanvasGradient, CanvasRenderingContext2d, HtmlCanvasElement, Path2d};

/// An immediate Canvas2D interpreter for Puri's drawing language.
pub struct WebCanvas(pub CanvasRenderingContext2d);

impl WebCanvas {
    pub fn clear(&self, width: f64, height: f64, color: impl Into<Brush>) {
        let _ = self.0.reset_transform();
        self.0.clear_rect(0.0, 0.0, width, height);
        self.set_fill_style(&color.into());
        self.0.fill_rect(0.0, 0.0, width, height);
    }

    fn set_transform(&self, transform: Affine) {
        let [a, b, c, d, e, f] = transform.as_coeffs();
        let _ = self.0.set_transform(a, b, c, d, e, f);
    }

    fn with_transform(&self, transform: Affine, draw: impl FnOnce(&Self)) {
        self.0.save();
        self.set_transform(transform);
        draw(self);
        self.0.restore();
    }

    fn set_fill_style(&self, brush: &Brush) {
        match self.style(brush) {
            Style::Color(color) => self.0.set_fill_style_str(&color),
            Style::Gradient(gradient) => self.0.set_fill_style_canvas_gradient(&gradient),
        }
    }

    fn set_stroke_style(&self, brush: &Brush) {
        match self.style(brush) {
            Style::Color(color) => self.0.set_stroke_style_str(&color),
            Style::Gradient(gradient) => self.0.set_stroke_style_canvas_gradient(&gradient),
        }
    }

    fn style(&self, brush: &Brush) -> Style {
        match brush {
            Brush::Solid(color) => Style::Color(css(color.components)),
            Brush::Gradient(gradient) => {
                let canvas = match gradient.kind {
                    GradientKind::Linear(linear) => self.0.create_linear_gradient(
                        linear.start.x,
                        linear.start.y,
                        linear.end.x,
                        linear.end.y,
                    ),
                    GradientKind::Radial(radial) => self
                        .0
                        .create_radial_gradient(
                            radial.start_center.x,
                            radial.start_center.y,
                            radial.start_radius.into(),
                            radial.end_center.x,
                            radial.end_center.y,
                            radial.end_radius.into(),
                        )
                        .unwrap(),
                    // Sweep gradients are not yet part of Progred's display
                    // vocabulary. Preserve a deterministic rendering if one
                    // arrives through Rust directly.
                    GradientKind::Sweep(_) => {
                        return Style::Color(
                            gradient
                                .stops
                                .first()
                                .map(|stop| css(stop.color.to_alpha_color::<Srgb>().components))
                                .unwrap_or_else(|| "rgba(0,0,0,0)".to_string()),
                        );
                    }
                };
                for stop in gradient.stops.iter() {
                    let color = css(stop.color.to_alpha_color::<Srgb>().components);
                    let _ = canvas.add_color_stop(stop.offset, &color);
                }
                Style::Gradient(canvas)
            }
            // Images are likewise absent from the current display language.
            Brush::Image(_) => Style::Color("rgba(255,0,255,1)".to_string()),
        }
    }

    fn set_stroke(&self, stroke: &Stroke) {
        self.0.set_line_width(stroke.width);
        self.0.set_line_join(match stroke.join {
            Join::Bevel => "bevel",
            Join::Miter => "miter",
            Join::Round => "round",
        });
        // Canvas has one cap for both ends. Puri's current drawings use the
        // same value; prefer the start cap if a direct Rust caller differs.
        self.0.set_line_cap(match stroke.start_cap {
            Cap::Butt => "butt",
            Cap::Square => "square",
            Cap::Round => "round",
        });
        self.0.set_miter_limit(stroke.miter_limit);
        self.0.set_line_dash_offset(stroke.dash_offset);
        let dashes = Array::new();
        for dash in stroke.dash_pattern.iter() {
            dashes.push(&JsValue::from_f64(*dash));
        }
        let _ = self.0.set_line_dash(&dashes);
    }

    /// Raw clip bracket for wrapper canvases that interleave their own
    /// state around `Canvas::clip`.
    pub fn push_clip(&self, shape: &Shape, transform: Affine) {
        let path = path(shape);
        self.0.save();
        self.set_transform(transform);
        self.0.clip_with_path_2d(&path);
    }

    pub fn pop_clip(&self) {
        self.0.restore();
    }
}

impl puri::draw::CanvasSink for WebCanvas {
    fn draw_image(&mut self, image: ImageData, transform: Affine) {
        if let Some(source) = image_canvas(&image) {
            self.with_transform(transform, |canvas| {
                let _ = canvas
                    .0
                    .draw_image_with_html_canvas_element(&source, 0.0, 0.0);
            });
        }
    }

    fn fill_shape(&mut self, shape: Shape, brush: Brush, transform: Affine) {
        let path = path(&shape);
        self.with_transform(transform, |canvas| {
            canvas.set_fill_style(&brush);
            canvas.0.fill_with_path_2d(&path);
        });
    }

    fn stroke_shape(&mut self, shape: Shape, style: Stroke, brush: Brush, transform: Affine) {
        let path = path(&shape);
        self.with_transform(transform, |canvas| {
            canvas.set_stroke(&style);
            canvas.set_stroke_style(&brush);
            canvas.0.stroke_with_path(&path);
        });
    }

    fn draw_glyphs(&mut self, run: GlyphRun) {
        let Ok(font) = FontRef::from_index(run.font.data.as_ref(), run.font.index) else {
            return;
        };
        let outlines = font.outline_glyphs();
        let coords: Vec<NormalizedCoord> = run
            .normalized_coords
            .iter()
            .map(|bits| NormalizedCoord::from_bits(*bits))
            .collect();
        for glyph in &run.glyphs {
            let Some(outline) = outlines.get(GlyphId::new(glyph.id)) else {
                continue;
            };
            let Ok(glyph_path) = Path2d::new() else {
                continue;
            };
            let mut pen = CanvasPen(&glyph_path);
            let settings = DrawSettings::unhinted(Size::new(run.size), LocationRef::new(&coords));
            if outline.draw(settings, &mut pen).is_err() {
                continue;
            }
            let transform = run.transform
                * Affine::translate((glyph.x as f64, glyph.y as f64))
                * Affine::scale_non_uniform(1.0, -1.0)
                * run.glyph_transform.unwrap_or(Affine::IDENTITY);
            self.with_transform(transform, |canvas| {
                canvas.set_fill_style(&run.brush);
                canvas.0.fill_with_path_2d(&glyph_path);
            });
        }
    }

    fn with_clip(
        &mut self,
        shape: Shape,
        transform: Affine,
        content: Box<dyn FnOnce(&mut dyn puri::draw::CanvasSink) + '_>,
    ) {
        self.push_clip(&shape, transform);
        content(self);
        self.pop_clip();
    }
}

fn image_canvas(image: &ImageData) -> Option<HtmlCanvasElement> {
    let rgba = if image.format == ImageFormat::Rgba8 && image.alpha_type == ImageAlphaType::Alpha {
        Cow::Borrowed(image.data.as_ref())
    } else {
        let mut rgba = image.data.as_ref().to_vec();
        for pixel in rgba.chunks_exact_mut(4) {
            if image.format == ImageFormat::Bgra8 {
                pixel.swap(0, 2);
            }
            if image.alpha_type == ImageAlphaType::AlphaPremultiplied {
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
    };
    let pixels = web_sys::ImageData::new_with_u8_clamped_array_and_sh(
        Clamped(rgba.as_ref()),
        image.width,
        image.height,
    )
    .ok()?;
    let canvas: HtmlCanvasElement = web_sys::window()?
        .document()?
        .create_element("canvas")
        .ok()?
        .dyn_into()
        .ok()?;
    canvas.set_width(image.width);
    canvas.set_height(image.height);
    let context: CanvasRenderingContext2d = canvas.get_context("2d").ok()??.dyn_into().ok()?;
    context.put_image_data(&pixels, 0.0, 0.0).ok()?;
    Some(canvas)
}

enum Style {
    Color(String),
    Gradient(CanvasGradient),
}

fn css([r, g, b, a]: [f32; 4]) -> String {
    format!(
        "rgba({},{},{},{a})",
        (r * 255.0).round(),
        (g * 255.0).round(),
        (b * 255.0).round(),
    )
}

fn path(shape: &Shape) -> Path2d {
    let bezier = match shape {
        Shape::Rect(shape) => shape.to_path(0.1),
        Shape::RoundedRect(shape) => shape.to_path(0.1),
        Shape::Circle(shape) => shape.to_path(0.1),
        Shape::Line(line) => {
            let mut path = kurbo::BezPath::new();
            path.move_to(line.p0);
            path.line_to(line.p1);
            path
        }
        Shape::Path(shape) => shape.clone(),
    };
    let path = Path2d::new().expect("Canvas2D Path2D");
    for element in bezier.elements() {
        match *element {
            PathEl::MoveTo(point) => path.move_to(point.x, point.y),
            PathEl::LineTo(point) => path.line_to(point.x, point.y),
            PathEl::QuadTo(control, point) => {
                path.quadratic_curve_to(control.x, control.y, point.x, point.y)
            }
            PathEl::CurveTo(first, second, point) => {
                path.bezier_curve_to(first.x, first.y, second.x, second.y, point.x, point.y)
            }
            PathEl::ClosePath => path.close_path(),
        }
    }
    path
}

struct CanvasPen<'a>(&'a Path2d);

impl OutlinePen for CanvasPen<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.0.move_to(x.into(), y.into());
    }

    fn line_to(&mut self, x: f32, y: f32) {
        self.0.line_to(x.into(), y.into());
    }

    fn quad_to(&mut self, cx0: f32, cy0: f32, x: f32, y: f32) {
        self.0
            .quadratic_curve_to(cx0.into(), cy0.into(), x.into(), y.into());
    }

    fn curve_to(&mut self, cx0: f32, cy0: f32, cx1: f32, cy1: f32, x: f32, y: f32) {
        self.0.bezier_curve_to(
            cx0.into(),
            cy0.into(),
            cx1.into(),
            cy1.into(),
            x.into(),
            y.into(),
        );
    }

    fn close(&mut self) {
        self.0.close_path();
    }
}
