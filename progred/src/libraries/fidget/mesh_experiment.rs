//! Opt-in CPU meshing measurements and diagnostic images, not a viewport backend.

use super::*;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use fidget_engine::mesh::{Mesh, Octree, Settings};
use std::{collections::BTreeMap, fmt::Write as _, path::Path, time::Instant};

fn cube_preview() -> VolumePreview {
    let (doc, _) = crate::gid_text::parse(crate::command::Example::Cube.source()).unwrap();
    let stack = crate::stack::load();
    let sources = crate::sources::Sources {
        doc: &doc,
        libraries: &stack.libraries,
    };
    let declaration = crate::workspace::declarations(doc.root.as_ref()).remove(0);
    let entry = crate::projection::viewport::entry(sources, &declaration.path).unwrap();
    let (value, function) = presentation::viewport(entry.value).unwrap();
    let result = ::grap::apply_value(
        function,
        [
            (presentation::vocabulary::VALUE, value.clone()),
            (
                crate::libraries::layout::vocabulary::WIDTH,
                crate::libraries::f64::value(512.0),
            ),
            (
                crate::libraries::layout::vocabulary::HEIGHT,
                crate::libraries::f64::value(512.0),
            ),
        ],
        &sources,
        100_000,
    );
    assert!(result.completed && !absent::is_absent(&result.result));
    volume_preview_fields(
        result
            .result
            .as_record()
            .unwrap()
            .get(&vocabulary::PREVIEW_MESH)
            .unwrap()
            .as_record()
            .unwrap(),
    )
    .unwrap()
}

struct Quality {
    boundary_edges: usize,
    nonmanifold_edges: usize,
    inconsistent_edges: usize,
    degenerate_triangles: usize,
    coincident_triangles: usize,
    volume: f64,
    vertex_residual: f32,
    face_residual: f32,
}

fn quality(mesh: &Mesh, shape: &VmShape) -> Quality {
    let mut edges = BTreeMap::<(usize, usize), (usize, i32)>::new();
    let mut points = mesh.vertices.clone();
    let mut volume = 0.0;
    let mut degenerate_triangles = 0;
    let mut coincident_triangles = 0;
    for triangle in &mesh.triangles {
        let [i, j, k] = [triangle.x, triangle.y, triangle.z];
        let [a, b, c] = [mesh.vertices[i], mesh.vertices[j], mesh.vertices[k]];
        assert!([a, b, c].iter().all(|v| v.iter().all(|x| x.is_finite())));
        degenerate_triangles += usize::from((b - a).cross(&(c - a)).norm_squared() == 0.0);
        coincident_triangles += usize::from(a == b || b == c || c == a);
        volume += f64::from(a.dot(&b.cross(&c))) / 6.0;
        points.push((a + b + c) / 3.0);
        for (a, b) in [(i, j), (j, k), (k, i)] {
            let edge = edges.entry((a.min(b), a.max(b))).or_default();
            edge.0 += 1;
            edge.1 += if a < b { 1 } else { -1 };
        }
    }
    let x: Vec<_> = points.iter().map(|v| v.x).collect();
    let y: Vec<_> = points.iter().map(|v| v.y).collect();
    let z: Vec<_> = points.iter().map(|v| v.z).collect();
    let mut evaluator = VmShape::new_float_slice_eval();
    let tape = shape.ez_float_slice_tape();
    let values = evaluator.eval(&tape, &x, &y, &z).unwrap();
    assert!(values.iter().all(|v| v.is_finite()));
    let max_abs = |values: &[f32]| values.iter().map(|v| v.abs()).fold(0.0, f32::max);
    Quality {
        boundary_edges: edges.values().filter(|(count, _)| *count == 1).count(),
        nonmanifold_edges: edges.values().filter(|(count, _)| *count != 2).count(),
        inconsistent_edges: edges.values().filter(|(_, winding)| *winding != 0).count(),
        degenerate_triangles,
        coincident_triangles,
        volume: volume.abs(),
        vertex_residual: max_abs(&values[..mesh.vertices.len()]),
        face_residual: max_abs(&values[mesh.vertices.len()..]),
    }
}

fn png_data(rgba: &[u8], side: u32) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, side, side);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();
    writer.write_image_data(rgba).unwrap();
    writer.finish().unwrap();
    bytes
}

// A small depth-buffered triangle rasterizer only for inspecting the actual mesh.
// Its timings are not measurements of a GPU mesh renderer.
fn diagnostic_images(mesh: &Mesh, view: &VolumeView, color: [u8; 3], side: u32) -> [Vec<u8>; 2] {
    let model_to_view = view.world_to_model.try_inverse().unwrap();
    let vertices: Vec<_> = mesh
        .vertices
        .iter()
        .map(|v| {
            let p = model_to_view.transform_point(&nalgebra::Point3::from(*v));
            Vector3::new(
                (p.x + 1.0) * side as f32 / 2.0,
                (1.0 - p.y) * side as f32 / 2.0,
                p.z,
            )
        })
        .collect();
    let background = [246, 246, 248, 255];
    let mut shaded = background.repeat((side * side) as usize);
    let mut wire = shaded.clone();
    let mut depth = vec![f32::NEG_INFINITY; (side * side) as usize];
    let light = Vector3::new(0.35, -0.45, 1.0).normalize();
    let edge = |a: Vector3<f32>, b: Vector3<f32>, x: f32, y: f32| {
        (b.x - a.x) * (y - a.y) - (b.y - a.y) * (x - a.x)
    };
    for triangle in &mesh.triangles {
        let [a, b, c] = [
            vertices[triangle.x],
            vertices[triangle.y],
            vertices[triangle.z],
        ];
        let area = edge(a, b, c.x, c.y);
        if area == 0.0 {
            continue;
        }
        let min_x = a.x.min(b.x).min(c.x).floor().max(0.0) as u32;
        let max_x = a.x.max(b.x).max(c.x).ceil().min(side as f32) as u32;
        let min_y = a.y.min(b.y).min(c.y).floor().max(0.0) as u32;
        let max_y = a.y.max(b.y).max(c.y).ceil().min(side as f32) as u32;
        let distances = [
            (b - c).xy().norm(),
            (c - a).xy().norm(),
            (a - b).xy().norm(),
        ];
        let model_vertices = [triangle.x, triangle.y, triangle.z].map(|i| {
            model_to_view
                .transform_point(&nalgebra::Point3::from(mesh.vertices[i]))
                .coords
        });
        let normal = (model_vertices[1] - model_vertices[0])
            .cross(&(model_vertices[2] - model_vertices[0]))
            .normalize();
        let brightness = 0.35 + 0.65 * normal.dot(&light).abs();
        let rgb = color.map(|n| (f32::from(n) * brightness).round() as u8);
        for y in min_y..max_y {
            for x in min_x..max_x {
                let weights = [
                    edge(b, c, x as f32 + 0.5, y as f32 + 0.5),
                    edge(c, a, x as f32 + 0.5, y as f32 + 0.5),
                    edge(a, b, x as f32 + 0.5, y as f32 + 0.5),
                ]
                .map(|w| w / area);
                if weights.iter().any(|w| *w < 0.0) {
                    continue;
                }
                let z = weights[0] * a.z + weights[1] * b.z + weights[2] * c.z;
                let index = (y * side + x) as usize;
                if z <= depth[index] {
                    continue;
                }
                depth[index] = z;
                shaded[4 * index..4 * index + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
                let on_edge = weights
                    .iter()
                    .zip(distances)
                    .any(|(w, length)| w * area.abs() / length < 0.55);
                let rgb = if on_edge { [30, 55, 66] } else { rgb };
                wire[4 * index..4 * index + 4].copy_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
            }
        }
    }
    [png_data(&shaded, side), png_data(&wire, side)]
}

fn add_image(svg: &mut String, png: &[u8], x: u32, y: u32, side: u32) {
    writeln!(svg, r#"<image x="{x}" y="{y}" width="{side}" height="{side}" href="data:image/png;base64,{}"/>"#, BASE64.encode(png)).unwrap();
}

#[test]
#[ignore = "CPU meshing experiment; writes timings, STL files, and a visual comparison"]
fn fidget_cube_mesh_experiment() {
    let start = Instant::now();
    let preview = cube_preview();
    let evaluate = start.elapsed();
    assert_eq!(preview.objects.len(), 1);
    let start = Instant::now();
    let shape = VmShape::from(preview.objects[0].tree.clone());
    let lower = start.elapsed();
    let bound_shape = shape.clone().try_into().unwrap();
    let center = (preview.min + preview.max) / 2.0;
    let half = (preview.max - preview.min) / 2.0;
    let world_to_model =
        Translation3::from(center).to_homogeneous() * Scale3::from(half).to_homogeneous();
    let target = std::env::var_os("CARGO_TARGET_DIR").unwrap();
    let out = Path::new(&target).join("mesh-experiment");
    std::fs::create_dir_all(&out).unwrap();
    let side = 400;
    let view = volume_view(
        &preview,
        Camera {
            yaw: 30.0,
            pitch: 60.0,
            zoom: 1.0,
        },
        PixelRenderSize::new(side, side),
    );
    let mut svg = String::from(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="1640" height="960" viewBox="0 0 1640 960"><rect width="1640" height="960" fill="#f6f6f8"/><g font-family="sans-serif" fill="#182b36"><text x="20" y="30" font-size="23">Fidget cube · CPU-generated triangle meshes</text><text x="20" y="58" font-size="15">Same document and camera at every resolution · flat shading above, mesh edges below · not a GPU speed benchmark</text>"##,
    );
    let mut csv = String::from(
        "depth,cells_per_axis,first_ms,median_octree_ms,median_extract_ms,median_total_ms,vertices,triangles,packed_geometry_bytes,boundary_edges,nonmanifold_edges,inconsistent_edges,degenerate_triangles,coincident_triangles,volume,max_vertex_field_residual,max_centroid_field_residual\n",
    );
    eprintln!(
        "cube viewport evaluation {evaluate:.2?}; VM lowering {lower:.2?}; bounds {:?} to {:?}",
        preview.min, preview.max
    );
    for (column, depth) in [5, 6, 7, 8].into_iter().enumerate() {
        let settings = Settings {
            depth,
            world_to_model,
            ..Default::default()
        };
        let mut timings = Vec::new();
        let mut mesh = Mesh::default();
        for _ in 0..4 {
            let start = Instant::now();
            let octree = Octree::build(&bound_shape, &settings).unwrap();
            let built = start.elapsed();
            let extract = Instant::now();
            mesh = octree.walk_dual();
            timings.push((
                built.as_secs_f64() * 1000.0,
                extract.elapsed().as_secs_f64() * 1000.0,
            ));
        }
        let median = |f: fn(&(f64, f64)) -> f64| {
            let mut samples: Vec<_> = timings[1..].iter().map(f).collect();
            samples.sort_by(f64::total_cmp);
            samples[samples.len() / 2]
        };
        let ms = median(|(a, b)| a + b);
        let q = quality(&mesh, &shape);
        assert!(!mesh.triangles.is_empty());
        assert_eq!(
            (q.boundary_edges, q.nonmanifold_edges, q.inconsistent_edges),
            (0, 0, 0)
        );
        let cells = 1_u32 << depth;
        let bytes = mesh.vertices.len() * 12 + mesh.triangles.len() * 12;
        eprintln!(
            "depth {depth} ({cells}^3): first {:.2}ms; median {ms:.2}ms; {} vertices, {} triangles, {:.1} KiB packed; vertex/centroid field residual {:.6}/{:.6}; volume {:.6}; {} degenerate triangles ({} with coincident vertices)",
            timings[0].0 + timings[0].1,
            mesh.vertices.len(),
            mesh.triangles.len(),
            bytes as f64 / 1024.0,
            q.vertex_residual,
            q.face_residual,
            q.volume,
            q.degenerate_triangles,
            q.coincident_triangles
        );
        writeln!(
            csv,
            "{depth},{cells},{:.3},{:.3},{:.3},{ms:.3},{},{},{bytes},{},{},{},{},{},{:.9},{:.9},{:.9}",
            timings[0].0 + timings[0].1,
            median(|(a, _)| *a),
            median(|(_, b)| *b),
            mesh.vertices.len(),
            mesh.triangles.len(),
            q.boundary_edges,
            q.nonmanifold_edges,
            q.inconsistent_edges,
            q.degenerate_triangles,
            q.coincident_triangles,
            q.volume,
            q.vertex_residual,
            q.face_residual
        )
        .unwrap();
        mesh.write_stl(
            &mut std::fs::File::create(out.join(format!("cube-depth-{depth}.stl"))).unwrap(),
        )
        .unwrap();
        let images = diagnostic_images(&mesh, &view, preview.objects[0].color, side);
        let x = 20 + column as u32 * side;
        writeln!(svg, r#"<text x="{x}" y="94" font-size="19">Depth {depth} · {cells}³ grid</text><text x="{x}" y="120" font-size="15">{} triangles · {ms:.1} ms to mesh</text>"#, mesh.triangles.len()).unwrap();
        for (row, image) in images.iter().enumerate() {
            std::fs::write(
                out.join(format!(
                    "cube-depth-{depth}-{}.png",
                    if row == 0 { "solid" } else { "wire" }
                )),
                image,
            )
            .unwrap();
            add_image(&mut svg, image, x, 135 + row as u32 * side, side);
        }
    }
    svg.push_str("</g></svg>");
    std::fs::write(out.join("cube-comparison.svg"), svg).unwrap();
    std::fs::write(out.join("timings.csv"), csv).unwrap();
    let reference = cpu_volume(&preview.objects, &view).unwrap();
    std::fs::write(
        out.join("cube-voxel-reference.png"),
        png_data(&reference, side),
    )
    .unwrap();
    eprintln!("Artifacts: {}", out.display());
}
