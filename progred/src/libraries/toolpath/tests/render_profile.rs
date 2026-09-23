//! Opt-in measurements of the real multi-operation CAM program.
use super::*;
use crate::libraries::fidget as implicit;
use fidget_engine::{compiler::RegOp, context::Tree, vm::VmShape};
use std::time::Instant;

fn balanced_union(trees: &[Tree]) -> Tree {
    if trees.len() == 1 {
        trees[0].clone()
    } else {
        let mid = trees.len() / 2;
        balanced_union(&trees[..mid]).min(balanced_union(&trees[mid..]))
    }
}

#[test]
#[ignore = "profiles current CAM scene layers and same-color path grouping"]
fn cam_render_profile() {
    use super::super::playback::Draw;
    let start = Instant::now();
    let (doc, names) = crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let stack = crate::stack::load();
    let sources = crate::sources::Sources {
        doc: &doc,
        libraries: &stack.libraries,
    };
    let mut recording = Recording::default();
    let eval = run(&mut recording, |scope| {
        ::grap::apply_value_scoped(
            &names["preview_operations"].into(),
            [],
            &sources,
            scope,
            3_000_000,
        )
    });
    assert!(
        eval.completed && !absent::is_absent(&eval.result),
        "{:?}",
        eval.result
    );
    eprintln!(
        "CAM record {:?}, {} segments",
        start.elapsed(),
        recording.segments().count()
    );
    // Ignore tool identity here: no repeated geometry even in this broader
    // comparison rules out exact duplicate cuts with an identical tool.
    let mut segments = std::collections::BTreeSet::new();
    let mut duplicates = 0;
    let mut collapsed = 0;
    for (a, b, axis) in recording.segments() {
        let bits = |p: Point3| p.map(|n| if n == 0.0 { 0 } else { n.to_bits() });
        let mut endpoints = [bits(a), bits(b)];
        endpoints.sort();
        duplicates += usize::from(!segments.insert((endpoints, bits(axis.vector()))));
        collapsed += usize::from(a.map(|n| n as f32) == b.map(|n| n as f32));
    }
    eprintln!("exact duplicate segments {duplicates}; segments collapsed in f32 {collapsed}");
    let progress: f64 = std::env::var("CAM_PROGRESS")
        .unwrap_or("0.5".into())
        .parse()
        .unwrap();
    let height: f64 = std::env::var("CAM_HEIGHT")
        .unwrap_or("750".into())
        .parse()
        .unwrap();
    let settings = super::super::playback::Settings::read(&Value::record([
        (PROGRESS, f64::value(progress)),
        (PROFILE_TOLERANCE, f64::value(0.001)),
        (STOCK_MIN, point_value([-0.5; 3])),
        (STOCK_MAX, point_value([0.5; 3])),
        (
            STOCK,
            Value::record([(
                implicit::vocabulary::COLOR,
                crate::libraries::color::value(peniko::Color::from_rgb8(184, 155, 109)),
            )]),
        ),
    ]))
    .unwrap();
    let start = Instant::now();
    let stock = settings.remaining_stock(&recording).unwrap().unwrap();
    eprintln!("stock construction {:?}", start.elapsed());
    let start = Instant::now();
    let mut tubes = super::super::fidget::Tubes::new(0.005).unwrap();
    tubes.style(0.005, [240, 174, 80]).unwrap();
    settings
        .draw(&recording, &mut tubes, 0.005, [240, 174, 80])
        .unwrap();
    let objects = tubes.scene();
    eprintln!(
        "paths/tool construction {:?}; {} objects",
        start.elapsed(),
        objects.len()
    );
    let (paths, tool): (Vec<_>, Vec<_>) =
        objects.into_iter().partition(|o| o.color == [240, 174, 80]);
    let preview = |objects| {
        use implicit::vocabulary as f;
        let mut preview = implicit::volume_preview(&Value::record([(
            f::PREVIEW_3D,
            Value::record([
                (f::FIELD, crate::libraries::f32::value(1.0)),
                (
                    crate::libraries::layout::vocabulary::WIDTH,
                    f64::value(height * 4.0 / 9.0),
                ),
                (
                    crate::libraries::layout::vocabulary::HEIGHT,
                    f64::value(height),
                ),
                (f::MIN_X, crate::libraries::f32::value(-0.85)),
                (f::MIN_Y, crate::libraries::f32::value(-0.85)),
                (f::MIN_Z, crate::libraries::f32::value(-0.85)),
                (f::MAX_X, crate::libraries::f32::value(0.85)),
                (f::MAX_Y, crate::libraries::f32::value(0.85)),
                (f::MAX_Z, crate::libraries::f32::value(0.85)),
            ]),
        )]))
        .unwrap();
        preview.objects = objects;
        preview
    };
    let cancel = incremental::Cancellation::default();
    if std::env::var("CAM_HYBRID").is_ok() {
        let model = preview(vec![stock.clone()]);
        let start = Instant::now();
        let geometry = super::super::mesh::computation::paths(
            &super::super::computation::Recorded {
                path: std::sync::Arc::new(recording),
                evaluation: eval,
            },
            0.005,
            [240, 174, 80],
            Some(&settings),
        )
        .unwrap();
        eprintln!(
            "mesh paths/tool {:?}, {} triangles",
            start.elapsed(),
            geometry.indices.len() / 3
        );
        let legacy = paths
            .iter()
            .chain(tool.iter())
            .chain(std::iter::once(&stock))
            .cloned()
            .collect();
        let mut renderer = implicit::mesh::Renderer::default();
        // Warm the triangle backend separately from steady-state comparison.
        implicit::mesh::raster(&geometry, &model, None, 1.0, &mut renderer).unwrap();
        if std::env::var("CAM_MESH_UPLOADS").is_ok() {
            let mut geometry = geometry;
            let mut timings = [Vec::new(), Vec::new()];
            let mut expected = None;
            for trial in 0..12 {
                for index in [trial % 2, 1 - trial % 2] {
                    if index == 0 {
                        // Keep identical geometry but detach the previous upload's weak identity.
                        std::sync::Arc::make_mut(&mut geometry);
                    }
                    let start = Instant::now();
                    let image = implicit::mesh::raster(&geometry, &model, None, 1.0, &mut renderer)
                        .unwrap();
                    timings[index].push(start.elapsed());
                    if let Some(expected) = &expected {
                        assert_eq!(image.data.data(), expected);
                    } else {
                        expected = Some(image.data.data().to_vec());
                    }
                }
            }
            for (label, mut times) in ["reupload", "retained"].into_iter().zip(timings) {
                times.sort();
                eprintln!(
                    "{label} mesh upload/render/readback: median {:?}, min {:?}, max {:?}",
                    (times[5] + times[6]) / 2,
                    times[0],
                    times[11]
                );
            }
            return;
        }
        if std::env::var("CAM_MESH_SHADING").is_ok() {
            let mut flat = geometry.clone();
            for vertex in &mut std::sync::Arc::make_mut(&mut flat).vertices {
                vertex.normal = implicit::mesh::Normal::default();
            }
            eprintln!(
                "{} vertices, {} extra normal bytes",
                geometry.vertices.len(),
                geometry.vertices.len() * 4
            );
            let mut timings = [Vec::new(), Vec::new()];
            for trial in 0..12 {
                for index in [trial % 2, 1 - trial % 2] {
                    let start = Instant::now();
                    implicit::mesh::raster(
                        if index == 0 { &flat } else { &geometry },
                        &model,
                        None,
                        1.0,
                        &mut renderer,
                    )
                    .unwrap();
                    timings[index].push(start.elapsed());
                }
            }
            for (label, mut times) in ["flat", "smooth"].into_iter().zip(timings) {
                times.sort();
                eprintln!(
                    "{label} mesh upload/render/readback: median {:?}, min {:?}, max {:?}",
                    (times[5] + times[6]) / 2,
                    times[0],
                    times[11]
                );
            }
            return;
        }
        for (label, objects) in [("all implicit", legacy), ("hybrid", vec![stock])] {
            let request = implicit::raster::Request::new(preview(objects), None, 1.0).unwrap();
            for trial in 0..3 {
                let start = Instant::now();
                let frame = request
                    .render_software_tiles(
                        implicit::raster::Passes::Final,
                        4,
                        &cancel,
                        &mut |_| Ok(()),
                        None,
                    )
                    .unwrap()
                    .unwrap();
                let implicit_time = start.elapsed();
                let composition = Instant::now();
                if label == "hybrid" {
                    implicit::mesh::raster_surface(
                        &geometry,
                        Some(implicit::mesh::Surface {
                            frame: frame.clone(),
                            mesh_start: geometry.indices.len(),
                        }),
                        &model,
                        None,
                        1.0,
                        &mut renderer,
                    )
                    .unwrap();
                }
                eprintln!(
                    "{label} trial {trial}, progress {progress}, {}x{}: implicit {implicit_time:?}, compose {:?}, total {:?}",
                    frame.image.width,
                    frame.image.height,
                    composition.elapsed(),
                    start.elapsed()
                );
            }
        }
        return;
    }
    #[cfg(all(not(target_arch = "wasm32"), feature = "gpu-experiment"))]
    if std::env::var("CAM_GPU").is_ok() {
        let objects = if std::env::var("CAM_GPU_STOCK").is_ok() {
            vec![stock]
        } else {
            paths
                .iter()
                .chain(tool.iter())
                .chain(std::iter::once(&stock))
                .cloned()
                .collect()
        };
        implicit::raster::diagnostics::compare_gpu(&preview(objects));
        return;
    }
    if std::env::var("CAM_SCENE_TILES").is_ok()
        || std::env::var("CAM_SCENE_PREPARE").is_ok()
        || std::env::var("CAM_TILE_PUBLICATION").is_ok()
        || std::env::var("CAM_RETAINED_REGIONS").is_ok()
    {
        let objects: Vec<_> = paths
            .iter()
            .chain(tool.iter())
            .chain(std::iter::once(&stock))
            .cloned()
            .collect();
        if std::env::var("CAM_RETAINED_REGIONS").is_ok() {
            let stock_index = objects.len() - 1;
            implicit::raster::diagnostics::compare_retained_regions(&preview(objects), stock_index);
        } else if std::env::var("CAM_TILE_PUBLICATION").is_ok() {
            implicit::raster::diagnostics::compare_tile_publication(&preview(objects));
        } else if std::env::var("CAM_SCENE_PREPARE").is_ok() {
            implicit::raster::diagnostics::compare_scene_preparation(&preview(objects));
        } else {
            implicit::raster::diagnostics::compare_scene_tiles(&preview(objects));
        }
        return;
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    if std::env::var("CAM_JIT").is_ok() {
        implicit::raster::diagnostics::compare_jit(&preview(vec![stock]));
        return;
    }
    if std::env::var("CAM_TILES").is_ok() {
        implicit::raster::diagnostics::compare_tiles(&preview(vec![stock]));
        return;
    }
    if std::env::var("CAM_STOCK_AB").is_ok() {
        let mut cuts = Vec::new();
        recording
            .playback(progress, |a, b, axis, tool, complete| {
                if complete {
                    if let Some(field) = tool.unwrap().sweep(a, b, axis, 0.001)? {
                        cuts.push((
                            std::array::from_fn::<_, 3, _>(|i| (a[i] + b[i]) * 0.5),
                            field,
                        ));
                    }
                }
                Ok::<_, InvalidPath>(())
            })
            .unwrap();
        fn spatial(cuts: &mut [([f64; 3], Tree)]) -> Tree {
            if cuts.len() == 1 {
                return cuts[0].1.clone();
            }
            let axis = (0..3)
                .max_by(|&a, &b| {
                    let spread = |axis: usize| {
                        cuts.iter()
                            .map(|p| p.0[axis])
                            .fold(f64::NEG_INFINITY, f64::max)
                            - cuts.iter().map(|p| p.0[axis]).fold(f64::INFINITY, f64::min)
                    };
                    spread(a).total_cmp(&spread(b))
                })
                .unwrap();
            let mid = cuts.len() / 2;
            cuts.select_nth_unstable_by(mid, |a, b| a.0[axis].total_cmp(&b.0[axis]));
            let (a, b) = cuts.split_at_mut(mid);
            spatial(a).min(spatial(b))
        }
        let fields = cuts.iter().map(|c| c.1.clone()).collect::<Vec<_>>();
        assert!(
            !fields.is_empty(),
            "stock grouping requires positive playback progress"
        );
        let block = super::super::stock::Stock::block([-0.5; 3], [0.5; 3])
            .unwrap()
            .into_field();
        let variants = [
            ("left-fold", stock.tree.clone()),
            ("balanced", block.clone().max(-balanced_union(&fields))),
            ("spatial-balanced", block.max(-spatial(&mut cuts))),
        ];
        let mut reference: Option<Vec<u8>> = None;
        for (name, tree) in variants {
            let start = Instant::now();
            let shape = VmShape::from(tree.clone());
            let n = shape.inner().data().iter_asm().count();
            let spills = shape
                .inner()
                .data()
                .iter_asm()
                .filter(|op| matches!(op, RegOp::Load(..) | RegOp::Store(..)))
                .count();
            eprintln!(
                "stock {name}: compile {:?}, {n} instructions, {spills} spills",
                start.elapsed()
            );
            let object = implicit::SceneObject {
                tree,
                color: stock.color,
            };
            let request =
                implicit::raster::Request::new(preview(vec![object.clone()]), None, 1.0).unwrap();
            let start = Instant::now();
            let image = request
                .render_software_progressive(u32::MAX, 1, &cancel, &mut |_| unreachable!(), None)
                .unwrap()
                .unwrap();
            let differences = reference
                .as_ref()
                .map(|r| {
                    r.chunks_exact(4)
                        .zip(image.data.data().chunks_exact(4))
                        .filter(|(a, b)| a != b)
                        .count()
                })
                .unwrap_or(0);
            eprintln!(
                "stock {name}: native {:?}, {differences} pixel differences",
                start.elapsed()
            );
            if reference.is_none() {
                reference = Some(image.data.data().to_vec());
            }
            if std::env::var("CAM_MESH").is_ok() {
                let start = Instant::now();
                let mut geometry = implicit::mesh::Geometry::default();
                implicit::mesh::Shape::from(&preview(vec![object]))
                    .append(&mut geometry, 6)
                    .unwrap();
                eprintln!(
                    "stock {name}: mesh {:?}, {} vertices",
                    start.elapsed(),
                    geometry.vertices.len()
                );
            }
        }
        return;
    }
    for (label, objects) in [
        ("stock", vec![stock]),
        ("tool", tool),
        ("paths", paths.clone()),
    ] {
        let start = Instant::now();
        let shapes: Vec<_> = objects
            .iter()
            .map(|o| VmShape::from(o.tree.clone()))
            .collect();
        let instructions: usize = shapes
            .iter()
            .map(|s| s.inner().data().iter_asm().count())
            .sum();
        let spills: usize = shapes
            .iter()
            .flat_map(|s| s.inner().data().iter_asm())
            .filter(|op| matches!(op, RegOp::Load(..) | RegOp::Store(..)))
            .count();
        eprintln!(
            "{label}: {} objects; {instructions} instructions, {spills} load/stores; compile {:?}",
            objects.len(),
            start.elapsed()
        );
        drop(shapes);
        let request = implicit::raster::Request::new(preview(objects), None, 1.0).unwrap();
        let start = Instant::now();
        let mut prior = start;
        let result = request
            .render_software_progressive(
                512,
                4,
                &cancel,
                &mut |image| {
                    eprintln!(
                        "{label}: {}x{} stage {:?}, cumulative {:?}",
                        image.width,
                        image.height,
                        prior.elapsed(),
                        start.elapsed()
                    );
                    prior = Instant::now();
                    Ok(())
                },
                None,
            )
            .unwrap()
            .unwrap();
        eprintln!(
            "{label}: {}x{} final stage {:?}, total {:?}",
            result.width,
            result.height,
            prior.elapsed(),
            start.elapsed()
        );
    }
    let mesh = std::env::var("CAM_MESH").is_ok();
    if mesh {
        let shape = implicit::mesh::Shape::from(&preview(vec![
            settings.remaining_stock(&recording).unwrap().unwrap(),
        ]));
        let start = Instant::now();
        let mut geometry = implicit::mesh::Geometry::default();
        shape.append(&mut geometry, 6).unwrap();
        eprintln!(
            "stock mesh d6 {:?}, {} vertices",
            start.elapsed(),
            geometry.vertices.len()
        );
    }
    if std::env::var("CAM_GROUPS").is_ok() && !paths.is_empty() {
        let mut original: Option<Vec<u8>> = None;
        for group in [1, 8, 32, usize::MAX] {
            let objects = paths
                .chunks(group)
                .map(|group| implicit::SceneObject {
                    tree: balanced_union(&group.iter().map(|o| o.tree.clone()).collect::<Vec<_>>()),
                    color: group[0].color,
                })
                .collect::<Vec<_>>();
            let count = objects.len();
            let request = implicit::raster::Request::new(preview(objects), None, 1.0).unwrap();
            let start = Instant::now();
            let result = request
                .render_software_progressive(u32::MAX, 1, &cancel, &mut |_| unreachable!(), None)
                .unwrap()
                .unwrap();
            let bytes = result.data.data();
            let coverage = original
                .as_ref()
                .map(|old| {
                    old.chunks_exact(4)
                        .zip(bytes.chunks_exact(4))
                        .filter(|(a, b)| a[3] != b[3])
                        .count()
                })
                .unwrap_or(0);
            let colors = original
                .as_ref()
                .map(|old| {
                    old.chunks_exact(4)
                        .zip(bytes.chunks_exact(4))
                        .filter(|(a, b)| a != b)
                        .count()
                })
                .unwrap_or(0);
            eprintln!(
                "path grouping {group}: {count} objects, {:?}, coverage differences {coverage}, color differences {colors}",
                start.elapsed()
            );
            if original.is_none() {
                original = Some(bytes.to_vec());
            }
        }
    }
}
