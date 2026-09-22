use super::*;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use kurbo::{BezPath, Shape as KurboShape};
use peniko::{Extend, GradientKind, ImageAlphaType, ImageFormat, color::Srgb};
use skrifa::instance::{LocationRef, NormalizedCoord, Size};
use skrifa::outline::{DrawSettings, OutlinePen};
use skrifa::{FontRef, GlyphId, MetadataProvider};
use std::fmt::Write as _;

mod refined;

/// Computed content owns annotations below the declaration that produced it.
pub(super) fn result_path(path: &[Step]) -> Path {
    path.iter()
        .cloned()
        .chain([Step::Key(
            crate::libraries::presentation::vocabulary::RESULT,
        )])
        .collect()
}

// Preparation feeds the CAM viewport, which returns with-controls directly.
pub(super) fn cam_controls_path(path: &[Step]) -> Path {
    result_path(path)
}

pub(super) fn cam_position(progress: f64) -> Value {
    use crate::libraries::controls::tree_range;
    let (doc, names) = crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let libraries = crate::stack::load().libraries;
    let sources = crate::sources::Sources {
        doc: &doc,
        libraries: &libraries,
    };
    let tree =
        crate::libraries::tree::build(&names["program_tree"].into(), &sources, 3_000_000).unwrap();
    let selection = tree_range::Selection::new(&tree.items, None);
    Value::record([
        (
            names["focus"],
            tree_range::cursor_state(&selection, progress * selection.leaves.end as f64),
        ),
        (names["preview_mode"], names["stock"].into()),
    ])
}

fn image_png(image: &ImageData) -> Vec<u8> {
    let mut rgba = image.data.as_ref().to_vec();
    assert_eq!(
        Some(rgba.len()),
        image.format.size_in_bytes(image.width, image.height)
    );
    for pixel in rgba.chunks_exact_mut(4) {
        match image.format {
            ImageFormat::Rgba8 => {}
            ImageFormat::Bgra8 => pixel.swap(0, 2),
            _ => panic!("unsupported SVG image format"),
        }
        if image.alpha_type == ImageAlphaType::AlphaPremultiplied {
            let alpha = u32::from(pixel[3]);
            for channel in &mut pixel[..3] {
                *channel = if alpha == 0 {
                    0
                } else {
                    ((u32::from(*channel) * 255 + alpha / 2) / alpha).min(255) as u8
                };
            }
        }
    }
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, image.width, image.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(&rgba).unwrap();
    writer.finish().unwrap();
    bytes
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

pub(super) fn write_cmds(out: &mut String, cmds: &[DrawCmd]) {
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

fn render(doc: &Document, selection: Option<&Selection>, width: f64, out_path: &str) {
    let (bench, extent) = place(doc, selection, width);
    let (width, height) = (width.max(extent.width + 48.0), extent.height() + 48.0);
    write_svg(&bench.list, width, height, out_path);
}

fn write_svg(list: &DrawList, width: f64, height: f64, out_path: &str) {
    write_svg_on(
        list,
        width,
        height,
        crate::styles::Theme::Light.palette().paper,
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

#[test]
#[ignore = "writes website exercise captures without launching the app"]
fn website_lesson_svg_captures() {
    use crate::libraries::{absent, blob, color, control, f64, grap, layout, name, number, text};
    for (name, source, libraries) in [
        (
            "forest",
            include_str!("../../../../../website/public/lessons/forest.gid"),
            &[
                name::ID,
                text::ID,
                blob::ID,
                absent::ID,
                color::ID,
                control::ID,
                number::ID,
                f64::ID,
                grap::ID,
                layout::ID,
            ][..],
        ),
        (
            "create",
            include_str!("../../../../../website/public/lessons/create.gid"),
            &[name::ID, text::ID, blob::ID, number::ID, f64::ID][..],
        ),
        (
            "values",
            include_str!("../../../../../website/public/lessons/values.gid"),
            &[name::ID, text::ID, blob::ID, number::ID, f64::ID][..],
        ),
        (
            "lists",
            include_str!("../../../../../website/public/lessons/lists.gid"),
            &[name::ID, text::ID, blob::ID][..],
        ),
        (
            "cells",
            include_str!("../../../../../website/public/lessons/cells.gid"),
            &[name::ID, text::ID, blob::ID, number::ID, f64::ID][..],
        ),
        (
            "grap",
            include_str!("../../../../../website/public/lessons/grap.gid"),
            &[
                name::ID,
                text::ID,
                blob::ID,
                absent::ID,
                number::ID,
                f64::ID,
                grap::ID,
            ][..],
        ),
        (
            "functions",
            include_str!("../../../../../website/public/lessons/functions.gid"),
            &[
                name::ID,
                text::ID,
                blob::ID,
                absent::ID,
                number::ID,
                f64::ID,
                grap::ID,
            ][..],
        ),
        (
            "drawing",
            include_str!("../../../../../website/public/lessons/drawing.gid"),
            &[
                name::ID,
                text::ID,
                blob::ID,
                absent::ID,
                color::ID,
                control::ID,
                number::ID,
                f64::ID,
                grap::ID,
                layout::ID,
            ][..],
        ),
    ] {
        let (doc, fields) = crate::gid_text::parse(source).unwrap();
        for (theme_name, theme) in [
            ("light", crate::styles::Theme::Light),
            ("dark", crate::styles::Theme::Dark),
        ] {
            for width in [320.0, 620.0] {
                let mut editor = crate::test_editor_with_stack(
                    doc.clone(),
                    crate::stack::load_selected(libraries).unwrap(),
                );
                if matches!(name, "grap" | "functions" | "drawing" | "forest" | "create") {
                    editor.stack.projection = crate::web_embed::tutorial_slots(
                        Some(
                            &if matches!(name, "drawing" | "forest") {
                                ["third", "first", "second"]
                            } else {
                                ["first", "second", "third"]
                            }
                            .map(|key| fields[key].simple().to_string())
                            .join(","),
                        ),
                        editor.stack.projection,
                    )
                    .unwrap();
                }
                editor.font_cx = crate::bundled_font_context();
                editor.palette = theme.palette();
                editor.drawn_menu = false;
                render_editor(
                    editor,
                    kurbo::Size::new(
                        width,
                        if name == "forest" {
                            768.0
                        } else if name == "drawing" {
                            384.0
                        } else {
                            304.0
                        },
                    ),
                    &format!("website_{name}_{width}_{theme_name}.svg"),
                );
            }
        }
    }
}

fn website_shape_editor() -> (crate::Editor, crate::gid_text::Binders) {
    use crate::libraries::{
        absent, blob, color, control, controls, f32, f64, fidget, geometry, grap, layout, list,
        logic, name, number, presentation, text, toolpath, tree, u64,
    };
    let (doc, fields) = crate::gid_text::parse(include_str!(
        "../../../../../website/public/lessons/shape.gid"
    ))
    .unwrap();
    let mut editor = crate::test_editor_with_stack(
        doc,
        crate::stack::load_selected(&[
            name::ID,
            text::ID,
            blob::ID,
            absent::ID,
            color::ID,
            control::ID,
            number::ID,
            f64::ID,
            grap::ID,
            layout::ID,
            f32::ID,
            u64::ID,
            fidget::ID,
            controls::ID,
            presentation::ID,
            geometry::ID,
            logic::ID,
            list::ID,
            tree::ID,
            toolpath::ID,
        ])
        .unwrap(),
    );
    editor.stack.projection = crate::web_embed::tutorial_slots(
        Some(&format!(
            "{},{}",
            fields["second"].simple(),
            fields["first"].simple()
        )),
        editor.stack.projection,
    )
    .unwrap();
    editor.font_cx = crate::bundled_font_context();
    editor.drawn_menu = false;
    (editor, fields)
}

#[test]
fn website_shape_edits_and_playback_change_the_rendered_mesh() {
    use crate::libraries::{controls, presentation};
    fn meshes(commands: &[DrawCmd]) -> Vec<puri::mesh::Scene> {
        commands
            .iter()
            .flat_map(|command| match command {
                DrawCmd::Mesh { scene, .. } => vec![scene.clone()],
                DrawCmd::Clip { children, .. } => meshes(children),
                _ => Vec::new(),
            })
            .collect()
    }
    let (editor, names) = website_shape_editor();
    assert!(crate::workspace::declarations(editor.model.doc.root.as_ref()).is_empty());
    let mut runner = crate::EditorRunner::new(editor);
    let size = kurbo::Size::new(720.0, 520.0);
    let render = |runner: &mut crate::EditorRunner| {
        runner.refresh_frame(1.0, size);
        let mut list = DrawList::new();
        puri::frame::render(runner.prepare_paint(1.0, size).renders, &mut list);
        runner.frame_presented();
        let scenes = meshes(&list.0);
        assert_eq!(
            scenes.len(),
            1,
            "the CAM call must produce geometry, not an absent fallback"
        );
        assert!(scenes[0].geometry.indices.len() > 100);
        scenes.into_iter().next().unwrap()
    };
    let initial = render(&mut runner);
    let mut doc = (*runner.editor.model.doc).clone();
    let mut call = doc
        .cells
        .value(names["sample"])
        .unwrap()
        .as_record()
        .unwrap()
        .clone();
    call.insert(names["rows"], f64::value(5.0));
    doc.cells.set_value(names["sample"], Value::Record(call));
    runner.editor.model.doc = Rc::new(doc);
    let edited = render(&mut runner);
    assert_ne!(
        initial.geometry.indices.len(),
        edited.geometry.indices.len()
    );

    runner
        .editor
        .model
        .workspace
        .document
        .annotations
        .set_field(
            &[
                Step::Key(names["second"]),
                Step::Key(presentation::vocabulary::RESULT),
            ],
            controls::vocabulary::STATE,
            Some(Value::record([(
                names["preview_mode"],
                names["model"].into(),
            )])),
        );
    let model = render(&mut runner);
    assert_ne!(edited.geometry.indices.len(), model.geometry.indices.len());

    // These are arguments of the same call, not disconnected preview controls.
    let positions = |scene: &puri::mesh::Scene| {
        scene
            .geometry
            .vertices
            .iter()
            .map(|vertex| vertex.position)
            .collect::<Vec<_>>()
    };
    let mut previous = positions(&model);
    for (parameter, value) in [("control_depth", 0.25), ("tilt", 20.0)] {
        let mut doc = (*runner.editor.model.doc).clone();
        let mut call = doc
            .cells
            .value(names["sample"])
            .unwrap()
            .as_record()
            .unwrap()
            .clone();
        call.insert(names[parameter], f64::value(value));
        doc.cells.set_value(names["sample"], Value::Record(call));
        runner.editor.model.doc = Rc::new(doc);
        let current = positions(&render(&mut runner));
        assert!(previous != current, "{parameter} must change the geometry");
        previous = current;
    }
}

#[test]
#[ignore = "writes the machining website showcase without launching the app"]
fn website_shape_svg_capture() {
    render_editor(
        website_shape_editor().0,
        kurbo::Size::new(720.0, 520.0),
        "website_shape.svg",
    );
}

#[test]
#[ignore = "writes drawn-menu layout captures without launching the app"]
fn drawn_menu_svg_captures() {
    for (menu, file, width) in [
        (0, "menu_file.svg", 640.0),
        (1, "menu_examples.svg", 640.0),
        (3, "menu_view.svg", 350.0),
    ] {
        let mut editor = crate::test_editor(Document {
            root: None,
            cells: Cells::new(),
        });
        editor.drawn_menu = true;
        editor.model.workspace.toggle_projection(None);
        editor.menu.toggle(menu);
        render_editor(editor, kurbo::Size::new(width, 420.0), file);
    }
}

#[test]
#[ignore = "writes full-editor captures, including Fidget rasterization"]
fn editor_svg_captures() {
    use crate::command::Example;
    for (example, file) in [
        (Example::Fidget, "editor_fidget.svg"),
        (Example::Cube, "editor_fidget_cube.svg"),
        (Example::Toolpaths, "editor_toolpaths.svg"),
    ] {
        let (doc, _) = crate::gid_text::parse(example.source()).unwrap();
        render_editor(
            crate::test_editor(doc),
            kurbo::Size::new(1200.0, 900.0),
            file,
        );
    }
}

#[test]
#[ignore = "writes a full-editor capture of the mesh viewport"]
fn editor_mesh_svg_capture() {
    let (doc, _) = crate::gid_text::parse(crate::command::Example::Cube.source()).unwrap();
    render_editor(
        crate::test_editor(doc),
        kurbo::Size::new(1200.0, 900.0),
        "editor_fidget_mesh.svg",
    );
}

#[test]
#[ignore = "writes a full-editor capture of the meshed model and streamed toolpaths"]
fn editor_toolpath_mesh_svg_capture() {
    render_editor(
        cam_editor(crate::libraries::toolpath::vocabulary::PREVIEW_MESH),
        kurbo::Size::new(1500.0, 1050.0),
        "editor_toolpath_mesh.svg",
    );
}

#[test]
#[ignore = "captures Model and Stock with identical playback without opening the editor"]
fn editor_toolpath_model_stock_svg_captures() {
    use crate::libraries::controls::vocabulary::STATE;
    let (_, names) = crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let cursor = cam_position(0.35);
    for (mode, file) in [("model", "cam_model.svg"), ("stock", "cam_stock.svg")] {
        let mut editor = cam_editor(crate::libraries::toolpath::vocabulary::PREVIEW_MESH);
        let path = crate::workspace::declarations(editor.model.doc.root.as_ref())[0]
            .path
            .clone();
        let mut state = cursor.as_record().unwrap().clone();
        state.insert(names["preview_mode"], names[mode].into());
        editor.model.workspace.left.panes[0]
            .view
            .annotations
            .set_field(&cam_controls_path(&path), STATE, Some(Value::Record(state)));
        render_editor(editor, kurbo::Size::new(1200.0, 850.0), file);
    }
}

#[test]
#[ignore = "captures focused CAM groups without opening the editor"]
fn editor_toolpath_focus_svg_captures() {
    use crate::libraries::controls::{tree_range, vocabulary::STATE};
    let (doc, names) = crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let libraries = crate::stack::load().libraries;
    let sources = crate::sources::Sources {
        doc: &doc,
        libraries: &libraries,
    };
    let tree = ::grap::apply(&names["program_tree"].into(), [], &sources, 300_000).result;
    for (ranges, file) in [
        (vec![(4, 0..1), (3, 0..1), (2, 0..1)], "cam_focus_top.svg"),
        (vec![(4, 1..2)], "cam_focus_op2.svg"),
        (
            vec![(4, 0..1), (3, 0..1), (2, 0..1), (1, 0..1), (0, 10..11)],
            "cam_focus_line.svg",
        ),
    ] {
        let selection = ranges.into_iter().fold(
            tree_range::Selection::new(&tree, None),
            |selection, (level, range)| selection.select(level, range),
        );
        let mut editor = cam_editor(crate::libraries::toolpath::vocabulary::PREVIEW_MESH);
        let path = crate::workspace::declarations(editor.model.doc.root.as_ref())[0]
            .path
            .clone();
        editor.model.workspace.left.panes[0]
            .view
            .annotations
            .set_field(
                &cam_controls_path(&path),
                STATE,
                Some(Value::record([(
                    names["focus"],
                    tree_range::cursor_state(&selection, selection.leaves.start as f64),
                )])),
            );
        render_editor(editor, kurbo::Size::new(1200.0, 850.0), file);
    }
}

#[test]
#[ignore = "captures mesh CAM's first pending frame and retained stock during an async update"]
fn editor_toolpath_async_svg_captures() {
    capture_cam_async(crate::libraries::toolpath::vocabulary::PREVIEW_MESH);
}

#[test]
#[ignore = "captures async implicit CAM using the software renderer"]
fn editor_toolpath_implicit_async_svg_captures() {
    capture_cam_async(crate::libraries::toolpath::vocabulary::PREVIEW_3D);
}

#[test]
#[ignore = "captures successive implicit refinements through the full editor, without a window"]
fn editor_toolpath_progressive_svg_captures() {
    use incremental::background::Executor;
    use std::{
        sync::mpsc,
        time::{Duration, Instant},
    };

    fn image_sizes(commands: &[DrawCmd], sizes: &mut Vec<(u32, u32)>) {
        for command in commands {
            match command {
                DrawCmd::Image { image, .. } => sizes.push((image.width, image.height)),
                DrawCmd::Mesh { scene, .. } => sizes.push((scene.view.width, scene.view.height)),
                DrawCmd::Clip { children, .. } => image_sizes(children, sizes),
                _ => {}
            }
        }
    }

    let mut editor = cam_editor(crate::libraries::toolpath::vocabulary::PREVIEW_3D);
    let (send, receive) = mpsc::channel();
    editor.computations = crate::computations::Computations::new(
        Executor::new({
            let send = send.clone();
            move |job| {
                let send = send.clone();
                std::thread::spawn(move || {
                    job();
                    send.send(true).unwrap();
                });
            }
        }),
        move || {
            send.send(false).unwrap();
        },
    );
    let mut runner = crate::EditorRunner::new(editor);
    let size = kurbo::Size::new(1000.0, 750.0);
    let start = Instant::now();
    runner.refresh_frame(1.0, size);
    let mut seen = Vec::new();
    loop {
        let finished = receive.recv_timeout(Duration::from_secs(60)).unwrap();
        if runner.editor.computations.tasks.poll() {
            runner.refresh_frame(1.0, size);
            let paint = runner.prepare_paint(1.0, size);
            let mut list = DrawList::new();
            puri::frame::render(paint.renders, &mut list);
            let mut pixels = Vec::new();
            image_sizes(&list.0, &mut pixels);
            assert_eq!(pixels.len(), 1);
            seen.push(pixels[0]);
            eprintln!(
                "implicit update {} {:?}: {:.2} ms",
                seen.len(),
                pixels[0],
                start.elapsed().as_secs_f64() * 1000.0
            );
            write_svg(
                &list,
                size.width,
                size.height,
                &format!("cam_progressive_{}.svg", seen.len()),
            );
        }
        if finished {
            break;
        }
    }
    assert!(
        seen.len() >= 2,
        "the editor must display intermediate results"
    );
    for pair in seen.windows(2) {
        assert!(pair[0].0 <= pair[1].0 && pair[0].1 <= pair[1].1);
    }
    assert_eq!(
        seen.last().unwrap().1,
        size.height as u32,
        "the final raster fills the pane, including behind its controls"
    );
}

pub(super) fn cam_editor(mode: CellId) -> crate::Editor {
    // Exercise each public preview using the same document, without adding a
    // renderer-selection control to the production example.
    let source = crate::command::Example::Toolpaths.source().replace(
        &crate::libraries::toolpath::vocabulary::PREVIEW_REFINED
            .simple()
            .to_string(),
        &mode.simple().to_string(),
    );
    let (doc, _) = crate::gid_text::parse(&source).unwrap();
    let declarations = crate::workspace::declarations(doc.root.as_ref());
    let mut editor = crate::test_editor(doc);
    editor.model.workspace.sync_declared(&declarations);
    editor
}

fn capture_cam_async(mode: CellId) {
    use crate::libraries::controls::vocabulary::STATE;
    use incremental::background::{Executor, Job};
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
    };

    let mut editor = cam_editor(mode);
    let queue = Arc::new(Mutex::new(VecDeque::<Job>::new()));
    editor.computations = crate::computations::Computations::new(
        Executor::new({
            let queue = queue.clone();
            move |job| queue.lock().unwrap().push_back(job)
        }),
        || {},
    );
    let mut runner = crate::EditorRunner::new(editor);
    let size = kurbo::Size::new(1000.0, 750.0);
    let capture = |runner: &mut crate::EditorRunner, file| {
        let start = std::time::Instant::now();
        runner.refresh_frame(1.0, size);
        let paint = runner.prepare_paint(1.0, size);
        let mut list = DrawList::new();
        puri::frame::render(paint.renders, &mut list);
        eprintln!(
            "{file}: frame {:.2} ms",
            start.elapsed().as_secs_f64() * 1000.0
        );
        write_svg(&list, size.width, size.height, file);
    };
    let complete = |runner: &mut crate::EditorRunner| {
        let job = queue.lock().unwrap().pop_front().unwrap();
        let start = std::time::Instant::now();
        std::thread::spawn(job).join().unwrap();
        eprintln!(
            "preview worker {:.2} ms",
            start.elapsed().as_secs_f64() * 1000.0
        );
        assert!(runner.editor.computations.tasks.poll());
    };
    capture(&mut runner, "cam_async_first.svg");
    complete(&mut runner);
    capture(&mut runner, "cam_async_ready.svg");
    let path = crate::workspace::declarations(runner.editor.model.doc.root.as_ref())[0]
        .path
        .clone();
    let annotations = &mut runner.editor.model.workspace.left.panes[0].view.annotations;
    annotations.set_field(&cam_controls_path(&path), STATE, Some(cam_position(0.7)));
    capture(&mut runner, "cam_async_updating.svg");
    assert_eq!(queue.lock().unwrap().len(), 1);
    complete(&mut runner);
    capture(&mut runner, "cam_async_updated.svg");
    assert!(queue.lock().unwrap().is_empty());
}

#[test]
#[ignore = "writes remote-review captures at three playback positions"]
fn editor_toolpath_playback_svg_captures() {
    use crate::libraries::controls::vocabulary::STATE;
    let (doc, _) = crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    for (progress, file) in [
        (0.0, "playback_start.svg"),
        (0.35, "playback_middle.svg"),
        (1.0, "playback_end.svg"),
    ] {
        let mut editor = crate::test_editor(doc.clone());
        editor
            .model
            .workspace
            .sync_declared(&crate::workspace::declarations(doc.root.as_ref()));
        let pane = &mut editor.model.workspace.left.panes[0];
        let path = crate::workspace::declarations(doc.root.as_ref())[0]
            .path
            .clone();
        pane.view.annotations.set_field(
            &cam_controls_path(&path),
            STATE,
            Some(cam_position(progress)),
        );
        render_editor(editor, kurbo::Size::new(1500.0, 1050.0), file);
    }
}

#[test]
fn svg_images_preserve_pixels_transform_and_nested_clips() {
    let image = ImageData {
        data: vec![12, 34, 56, 128, 78, 90, 123, 255].into(),
        format: ImageFormat::Rgba8,
        alpha_type: ImageAlphaType::Alpha,
        width: 2,
        height: 1,
    };
    let command = DrawCmd::Image {
        image: image.clone(),
        transform: Affine::new([2.0, 0.0, 0.0, 3.0, 10.0, 20.0]),
    };
    let clip = |children| DrawCmd::Clip {
        shape: Shape::Rect(kurbo::Rect::new(0.0, 0.0, 4.0, 5.0)),
        transform: Affine::translate((10.0, 20.0)),
        children,
    };
    let mut svg = String::new();
    write_cmds(&mut svg, &[clip(vec![clip(vec![command])]), clip(vec![])]);
    assert!(svg.contains(r#"width="2" height="1" transform="matrix(2 0 0 3 10 20)""#));
    for id in 0..3 {
        assert_eq!(svg.matches(&format!(r#"id="clip{id}""#)).count(), 1);
        assert!(svg.contains(&format!("clip-path=\"url(#clip{id})\"")));
    }
    assert!(svg.contains(&svg_shape(
        &Shape::Rect(kurbo::Rect::new(10.0, 20.0, 14.0, 25.0)),
        Affine::IDENTITY
    )));
    assert!(svg.contains("</g>\n</g>"));
    let encoded = svg
        .split_once("data:image/png;base64,")
        .unwrap()
        .1
        .split('"')
        .next()
        .unwrap();
    let png = BASE64.decode(encoded).unwrap();
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png))
        .read_info()
        .unwrap();
    let mut pixels = vec![0; decoder.output_buffer_size().unwrap()];
    let info = decoder.next_frame(&mut pixels).unwrap();
    assert_eq!((info.width, info.height), (2, 1));
    assert_eq!(pixels.as_slice(), image.data.as_ref());
}

#[test]
fn svg_images_convert_bgra_and_premultiplied_alpha() {
    let image = ImageData {
        data: vec![16, 32, 64, 128, 0, 0, 0, 0].into(),
        format: ImageFormat::Bgra8,
        alpha_type: ImageAlphaType::AlphaPremultiplied,
        width: 2,
        height: 1,
    };
    let mut decoder = png::Decoder::new(std::io::Cursor::new(image_png(&image)))
        .read_info()
        .unwrap();
    let mut pixels = vec![0; decoder.output_buffer_size().unwrap()];
    decoder.next_frame(&mut pixels).unwrap();
    assert_eq!(pixels, [128, 64, 32, 128, 0, 0, 0, 0]);
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
                crate::libraries::f32::value(24.5),
            ),
            (crate::test_values::label("offset"), f64::value(-2.75)),
            (
                crate::test_values::label("count"),
                crate::libraries::u64::value(12),
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
fn svg_bench_renders_numeric_operation_labels() {
    let doc = Document {
        root: Some(Value::list(
            super::numbers::calls().into_iter().map(|(_, _, call)| call),
        )),
        cells: Cells::new(),
    };
    render(&doc, None, 440.0, "numeric_operation_labels.svg");
}

#[test]
fn svg_bench_renders_toolpath_source_and_preview() {
    use crate::libraries::presentation;
    let (doc, names) = crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    render(&doc, None, 760.0, "toolpaths_source.svg");
    let libraries = core_libraries();
    let sources = src(&doc, &libraries);
    let pane = crate::workspace::declarations(doc.root.as_ref()).remove(0);
    let declaration = sources.resolve_path(&pane.path).unwrap();
    let (value, _) = presentation::viewport(declaration).unwrap();
    assert_eq!(value.as_cell(), Some(names["program_tree"]));
    let preview = presentation::viewport_output(declaration, &sources, 700.0, 500.0).unwrap();
    assert!(
        preview
            .as_record()
            .unwrap()
            .contains_key(&crate::libraries::controls::vocabulary::WITH_CONTROLS)
    );
    let doc = Document {
        root: Some(preview),
        cells: doc.cells,
    };
    let (bench, _) = place(&doc, None, 760.0);
    let image = bench
        .list
        .0
        .iter()
        .find_map(|command| match command {
            DrawCmd::Image { image, .. } => Some(image.clone()),
            DrawCmd::Mesh { scene, .. } => scene.rasterize(),
            _ => None,
        })
        .expect("the combined toolpath viewport renders an image");
    let mut model = 0;
    let mut paths = 0;
    for pixel in image.data.as_ref().chunks_exact(4).filter(|p| p[3] > 0) {
        model += usize::from(pixel[2] > pixel[1] && pixel[1] > pixel[0]);
        paths += usize::from(u16::from(pixel[0]) > 2 * u16::from(pixel[2]));
    }
    assert!(model > 100 && paths > 100, "model {model}, paths {paths}");
    write_svg(&bench.list, 760.0, 548.0, "toolpaths.svg");
}

#[test]
fn failed_toolpath_preview_discards_the_model_and_partial_paths() {
    use crate::libraries::{absent, color, control, f32, fidget, presentation, toolpath};
    use toolpath::vocabulary as t;
    let point = |function: CellId, x| {
        grap::call(
            function.into(),
            [
                (t::X, f64::value(x)),
                (t::Y, f64::value(0.0)),
                (t::Z, f64::value(0.0)),
            ],
        )
    };
    for preview in [t::PREVIEW_3D, t::PREVIEW_MESH] {
        for (result, fuel, should_draw) in [
            (Value::record([]), 1000, true),
            (absent::with_reason(t::INVALID_INPUT), 1000, false),
            (Value::record([]), 0, false),
        ] {
            let program = Value::record([
                (grap::vocabulary::PARAMS, Value::list([])),
                (
                    grap::vocabulary::BODY,
                    grap::call(
                        control::vocabulary::DO.into(),
                        [(
                            control::vocabulary::EXPRESSIONS,
                            Value::list([point(t::START_AT, 0.0), point(t::LINE_TO, 1.0), result]),
                        )],
                    ),
                ),
            ]);
            let expression = grap::call(
                preview.into(),
                [
                    (presentation::vocabulary::VALUE, f32::value(-1.0)),
                    (t::PROGRAM, program),
                    (t::LINE_RADIUS, f64::value(0.01)),
                    (
                        fidget::vocabulary::COLOR,
                        Value::record([(color::vocabulary::RGB, vec![255, 128, 0].into())]),
                    ),
                    (layout_data::vocabulary::WIDTH, f64::value(32.0)),
                    (layout_data::vocabulary::HEIGHT, f64::value(32.0)),
                    (layout_data::vocabulary::FUEL, f64::value(fuel as f64)),
                ],
            );
            let stack = crate::stack::load();
            let result = grap::evaluate(&expression, &stack.libraries, 1000);
            assert!(result.completed && !absent::is_absent(&result.result));
            let doc = Document {
                root: Some(result.result),
                cells: Cells::new(),
            };
            let (bench, _) = place(&doc, None, 200.0);
            assert_eq!(
                bench
                    .list
                    .0
                    .iter()
                    .any(|cmd| matches!(cmd, DrawCmd::Image { .. } | DrawCmd::Mesh { .. })),
                should_draw
            );
        }
    }
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
        Some(&pending_value(&crate::test_root(), Vec::new())),
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
            &crate::test_root(),
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
            &crate::test_root(),
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
    let sel = Selection::edge(&crate::test_root(), Vec::new());
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
    let library = crate::stack::load().libraries;
    let edge = pending_edge(
        &crate::test_root(),
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
        &crate::test_root(),
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
