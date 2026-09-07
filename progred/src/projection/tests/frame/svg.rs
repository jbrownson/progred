use super::*;
use kurbo::{BezPath, Shape as KurboShape};
use skrifa::instance::{LocationRef, NormalizedCoord, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, GlyphId, MetadataProvider};
use std::fmt::Write as _;

fn css(brush: &Brush) -> String {
    match brush {
        Brush::Solid(color) => {
            let [r, g, b, a] = color.components;
            format!(
                "rgba({},{},{},{:.3})",
                (r * 255.0).round(),
                (g * 255.0).round(),
                (b * 255.0).round(),
                a
            )
        }
        _ => "magenta".to_string(),
    }
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

pub(super) fn write_cmds(out: &mut String, cmds: &[DrawCmd]) {
    for cmd in cmds {
        match cmd {
                DrawCmd::Image { .. } => panic!("the SVG bench does not encode raster images"),
                DrawCmd::Fill {
                    shape,
                    brush,
                    transform,
                } => writeln!(
                    out,
                    r#"<path d="{}" fill="{}"/>"#,
                    svg_shape(shape, *transform),
                    css(brush)
                )
                .unwrap(),
                DrawCmd::Stroke {
                    shape,
                    style,
                    brush,
                    transform,
                } => writeln!(
                    out,
                    r#"<path d="{}" fill="none" stroke="{}" stroke-width="{}" stroke-linecap="round" stroke-linejoin="round"/>"#,
                    svg_shape(shape, *transform),
                    css(brush),
                    style.width
                )
                .unwrap(),
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
                    writeln!(out, r#"<path d="{}" fill="{}"/>"#, path.to_svg(), css(&run.brush))
                        .unwrap();
                }
                DrawCmd::Clip { children, .. } => write_cmds(out, children),
            }
    }
}

fn render(doc: &Document, selection: Option<&Selection>, width: f64, out_path: &str) {
    let (bench, extent) = place(doc, selection, width);
    let (width, height) = (width.max(extent.width + 48.0), extent.height() + 48.0);
    let mut out = String::new();
    writeln!(
            out,
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width:.0}" height="{height:.0}" viewBox="0 0 {width:.0} {height:.0}">"#
        )
        .unwrap();
    writeln!(
        out,
        r##"<rect width="{width:.0}" height="{height:.0}" fill="#FFFFFF"/>"##
    )
    .unwrap();
    write_cmds(&mut out, &bench.list.0);
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

#[test]
fn svg_bench_renders_the_sample_projection() {
    let doc = sample_document();
    render(&doc, None, 900.0, "raw_projection.svg");
    render(&doc, None, 560.0, "raw_projection_narrow.svg");
    // The deep-fallback regime: hugging fails at most levels, so
    // this render is also the canary against layout cost blowing
    // up when width is scarce.
    render(&doc, None, 320.0, "raw_projection_tight.svg");
}

#[test]
fn svg_bench_renders_the_grap_demo() {
    let (doc, _) = crate::gid_text::parse(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../examples/grap-demo.gid"
    )))
    .expect("the Grap demo parses");
    render(&doc, None, 900.0, "grap_demo.svg");
    render(&doc, None, 560.0, "grap_demo_narrow.svg");
}

#[test]
fn svg_bench_renders_numeric_type_labels() {
    let mut cells = Cells::new();
    for label in ["radius", "offset", "count"] {
        cells.set_value(crate::test_values::label(label), name::record(label, []));
    }
    let doc = Document {
        root: Some(Value::record([
            (
                crate::test_values::label("radius"),
                progred_libraries::f32::value(24.5),
            ),
            (crate::test_values::label("offset"), f64::value(-2.75)),
            (
                crate::test_values::label("count"),
                progred_libraries::u64::value(12),
            ),
        ])),
        cells,
    };
    render(&doc, None, 400.0, "numeric_type_labels.svg");
    let selection = make_projected_selection(&doc, &core_libraries(), vec![key("radius")]);
    render(
        &doc,
        Some(&selection),
        400.0,
        "numeric_type_labels_editing.svg",
    );
}

#[test]
fn svg_bench_renders_fidget_source() {
    use crate::command::Example;
    for (example, file) in [
        (Example::Torus, "fidget_torus.svg"),
        (Example::Tanglecube, "fidget_tanglecube.svg"),
        (Example::Gyroid, "fidget_gyroid.svg"),
        (Example::Cube, "fidget_cube.svg"),
    ] {
        let (doc, _) = crate::gid_text::parse(example.source()).unwrap();
        render(&doc, None, 560.0, file);
    }
}

#[test]
fn svg_bench_renders_the_placeholder_notation() {
    let empty = Document {
        root: None,
        cells: Cells::new(),
    };
    render(&empty, None, 320.0, "raw_placeholder_root.svg");
    // The engaged twin: same slot, same rect, selection blue.
    render(
        &empty,
        Some(&pending_value(
            &crate::workspace::Root::document(),
            Vec::new(),
        )),
        320.0,
        "raw_placeholder_engaged.svg",
    );
    let cells = Cells::new();
    let bare = new_cell_id();
    render(
        &Document {
            root: Some(Value::from(bare)),
            cells,
        },
        None,
        320.0,
        "raw_placeholder_cell.svg",
    );
    // The commit transition pair: the same spelling typed in the
    // slot and committed as the string — glyphs should not move.
    render(
        &empty,
        Some(&crate::selection::pending_with_query(
            &crate::workspace::Root::document(),
            Vec::new(),
            "\"asdf\"",
        )),
        320.0,
        "raw_placeholder_typed.svg",
    );
    render(
        &Document {
            root: Some(crate::test_values::text("asdf")),
            cells: Cells::new(),
        },
        Some(&crate::selection::bare_edge(
            &crate::workspace::Root::document(),
            Vec::new(),
        )),
        320.0,
        "raw_placeholder_committed.svg",
    );
    // The empty string under its write-through editor: quotes
    // stay snug, no slot minimum applies to string literals.
    let empty_string = Document {
        root: Some(crate::test_values::text("")),
        cells: Cells::new(),
    };
    let sel = Selection::edge(&crate::workspace::Root::document(), Vec::new());
    render(
        &empty_string,
        Some(&sel),
        320.0,
        "raw_empty_string_editing.svg",
    );
}

#[test]
fn svg_bench_renders_a_pending_edge() {
    let doc = sample_document();
    let library = crate::stack::load::<()>().libraries;
    let edge = pending_edge(
        &crate::workspace::Root::document(),
        &Sources {
            doc: &doc,
            libraries: &library,
        },
        vec![Step::Key(crate::test_values::label("shape"))],
    )
    .unwrap();
    assert_eq!(
        edge.stage(&src(&doc, &library)),
        crate::selection::Stage::Label
    );
    let typing = edge.with_query("na");
    render(&doc, Some(&typing), 560.0, "raw_pending_edge.svg");

    let doc = Document {
        root: Some(Value::record([])),
        cells: Cells::new(),
    };
    let typing = pending_edge(
        &crate::workspace::Root::document(),
        &Sources {
            doc: &doc,
            libraries: &library,
        },
        Vec::new(),
    )
    .unwrap()
    .with_query("field");
    render(&doc, Some(&typing), 320.0, "pending_field_slot.svg");
}
