//! Opt-in diagnostics over real stock geometry, without editor or window instrumentation.
use super::*;
use crate::libraries::f64;
use crate::libraries::toolpath::{
    self,
    paths::Recording,
    stock::{BallEnd, Stock},
};
use std::time::{Duration, Instant};

fn paths() -> (Recording, f64) {
    let (doc, names) = crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let stack = crate::stack::load();
    let sources = crate::sources::Sources {
        doc: &doc,
        libraries: &stack.libraries,
    };
    let mut path = Recording::default();
    let evaluation = toolpath::run(&mut path, |scope| {
        ::grap::apply_scoped(&names["ball_path"].into(), [], &sources, scope, 500_000)
    });
    assert!(evaluation.completed && !absent::is_absent(&evaluation.result));
    let radius = f64::read(doc.cells.value(names["tool_diameter"]).unwrap()).unwrap() / 2.0;
    (path, radius)
}

fn stock(path: &Recording, radius: f64, progress: f64) -> Tree {
    let tool = BallEnd::new(radius, 0.22).unwrap();
    let mut stock = Stock::block([-0.5; 3], [0.5; 3]).unwrap();
    path.playback(progress, |a, b, complete| {
        if complete {
            stock.cut(&tool, a, b)?;
        }
        Ok::<_, toolpath::paths::InvalidPath>(())
    })
    .unwrap();
    stock.into_field()
}

fn write_png(name: &str, width: u32, height: u32, bytes: &[u8]) {
    let dir = std::path::PathBuf::from(std::env::var_os("CARGO_TARGET_DIR").unwrap());
    let file = std::fs::File::create(dir.join(name)).unwrap();
    let mut encoder = png::Encoder::new(file, width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(bytes)
        .unwrap();
}

#[test]
#[ignore = "checks whether lighting needs to account for sampling-axis scale"]
fn implicit_normal_sampling_diagnostic() {
    let preview = VolumePreview {
        objects: vec![SceneObject {
            tree: Tree::x() + Tree::z(),
            color: [200; 3],
        }],
        size: Size::new(600.0, 600.0),
        min: Vector3::repeat(-1.0),
        max: Vector3::repeat(1.0),
    };
    let shape = VmShape::from(preview.objects[0].tree.clone());
    let camera = Camera {
        yaw: 0.0,
        pitch: 0.0,
        zoom: 1.0,
    };
    for size in [42, 94, 188, 375, 600] {
        let view = volume_view(&preview, camera, PixelRenderSize::from(size));
        let config = VoxelRenderConfig {
            world_to_model: view.world_to_model,
            ..VoxelRenderConfig::from_size(view.size)
        };
        let image = config.run(shape.clone().try_into().unwrap());
        let pixel = image.as_slice()[(size / 2 * size + size / 2) as usize];
        assert!(pixel.depth > 0);
        let raw = Vector3::from(pixel.normal);
        let transform = config.mat();
        let corrected =
            Vector3::from_fn(|i, _| raw[i] / transform.fixed_view::<3, 1>(0, i).norm()).normalize();
        assert!((corrected - Vector3::new(1.0, 0.0, 1.0).normalize()).norm() < 0.0001);
        eprintln!(
            "plane {size}x{size} depth {}: raw normal {:?}, corrected {:?}, shade {:?}",
            view.size.depth(),
            raw.normalize(),
            corrected,
            shading(&config)(pixel, [200; 3])
        );
    }
}

#[test]
#[ignore = "distinguishes missing cap pixels from side normals at the tool rim"]
fn implicit_tool_rim_diagnostic() {
    let radius = 0.0625_f32;
    let length = 0.22_f32;
    let height = length - radius;
    let preview = VolumePreview {
        objects: vec![SceneObject {
            tree: BallEnd::new(f64::from(radius), f64::from(length))
                .unwrap()
                .sweep([0.0; 3], [0.0; 3])
                .unwrap(),
            color: [225, 94, 58],
        }],
        size: Size::new(300.0, 300.0),
        min: Vector3::new(-radius, -radius, -radius),
        max: Vector3::new(radius, radius, height),
    };
    let shape = VmShape::from(preview.objects[0].tree.clone());
    for z_scale in [1, 4, 16] {
        let view = volume_view(
            &preview,
            Camera {
                yaw: 0.0,
                pitch: 40.0,
                zoom: 1.0,
            },
            PixelRenderSize::from(300),
        );
        let view = refine_depth(view, z_scale).unwrap();
        let config = VoxelRenderConfig {
            world_to_model: view.world_to_model,
            ..VoxelRenderConfig::from_size(view.size)
        };
        let image = config.run(shape.clone().try_into().unwrap());
        let matrix = config.mat();
        let dz = matrix.fixed_view::<3, 1>(0, 2).into_owned();
        let cap_normal = Vector3::new(matrix[(2, 0)], matrix[(2, 1)], matrix[(2, 2)]).normalize();
        let mut cap_pixels = 0;
        let mut missing = 0;
        let mut side_normal = 0;
        let mut halves = [[0; 3]; 2]; // near/far: cap pixels, missing pixels, wrong normals
        for y in 0..300 {
            for x in 0..300 {
                let start = matrix.transform_point(&nalgebra::Point3::new(x as f32, y as f32, 0.0));
                let z = (height - start.z) / dz.z;
                let hit = start + dz * z;
                // The ray intersects the exact flat cap, strictly inside its rim.
                if z > 0.0
                    && z < view.size.depth() as f32 - 1.0
                    && hit.x * hit.x + hit.y * hit.y < radius * radius * 0.9999
                {
                    cap_pixels += 1;
                    let half = usize::from(hit.x * dz.x + hit.y * dz.y < 0.0);
                    halves[half][0] += 1;
                    let pixel = image.as_slice()[y * 300 + x];
                    if pixel.depth == 0 {
                        missing += 1;
                        halves[half][1] += 1;
                    } else if Vector3::from(pixel.normal).normalize().dot(&cap_normal) < 0.999 {
                        side_normal += 1;
                        halves[half][2] += 1;
                    }
                }
            }
        }
        eprintln!(
            "tool Zx{z_scale}: exact cap {cap_pixels}, missing {missing}, wrong normal {side_normal}"
        );
        eprintln!(
            "near {:?}, far {:?} (cap pixels, missing pixels, wrong normals)",
            halves[0], halves[1]
        );
        let shade = shading(&config);
        let rgba: Vec<_> = image
            .iter()
            .flat_map(|p| shade(*p, [225, 94, 58]))
            .collect();
        write_png(&format!("tool_rim_z{z_scale}.png"), 300, 300, &rgba);
    }
}

#[test]
#[ignore = "profiles compilation, cancellation and pixel quality on the CAM stock"]
fn implicit_stock_diagnostics() {
    let start = Instant::now();
    let (path, radius) = paths();
    eprintln!("Grap path recording {:?}", start.elapsed());
    let cancel = incremental::Cancellation::default();
    for progress in [0.35, 0.7, 1.0] {
        let start = Instant::now();
        let tree = stock(&path, radius, progress);
        eprintln!("{progress}: stock expression {:?}", start.elapsed());
        let objects = vec![SceneObject {
            tree,
            color: [190, 165, 110],
        }];
        let start = Instant::now();
        let compiled = SoftwareScene::new(&objects, &cancel).unwrap();
        eprintln!("{progress}: compilation {:?}", start.elapsed());
        let preview = VolumePreview {
            objects,
            size: Size::new(600.0, 600.0),
            min: Vector3::repeat(-0.5),
            max: Vector3::repeat(0.5),
        };
        let camera = Camera {
            zoom: 1.2,
            ..Default::default()
        };
        for (label, xy, z) in [("native", 1, 1), ("z4", 1, 4), ("ss2", 2, 2)] {
            let mut view = volume_view(&preview, camera, PixelRenderSize::from(600 * xy));
            // Hold the physical volume fixed while refining depth separately.
            let factor = z as f32 / xy as f32;
            view.size = VoxelRenderSize::new(
                view.size.width(),
                view.size.height(),
                (view.size.depth() as f32 * factor) as u32,
            );
            view.world_to_model *= Scale3::new(1.0, 1.0, 1.0 / factor).to_homogeneous();
            let config = VoxelRenderConfig {
                world_to_model: view.world_to_model,
                ..VoxelRenderConfig::from_size(view.size)
            };
            let shade = shading(&config);
            let start = Instant::now();
            let eval = fidget_engine::raster::voxel::EvalConfig {
                tile_sizes: Some(
                    fidget_engine::render::TileSizes::new(SOFTWARE_TILE_SIZES).unwrap(),
                ),
                ..Default::default()
            };
            let image = fidget_engine::raster::voxel::render(
                compiled.objects[0].0.clone().try_into().unwrap(),
                &config,
                &eval,
            )
            .unwrap();
            let mut zero = 0;
            let mut nan = 0;
            let mut interior_empty = 0;
            let mut rgba = Vec::new();
            for row in image.as_slice().chunks(view.size.width() as usize) {
                if let (Some(a), Some(b)) = (
                    row.iter().position(|p| p.depth > 0),
                    row.iter().rposition(|p| p.depth > 0),
                ) {
                    interior_empty += row[a..=b].iter().filter(|p| p.depth == 0).count();
                }
                for pixel in row {
                    let pixel = *pixel;
                    if pixel.depth > 0 {
                        zero += usize::from(pixel.normal == [0.0; 3]);
                        nan += usize::from(pixel.normal.iter().any(|n| !n.is_finite()));
                    }
                    rgba.extend(shade(pixel, [190, 165, 110]));
                }
            }
            eprintln!(
                "{progress} {label}: raster+shade {:?}, zero normals {zero}, nonfinite normals {nan}, interior empty pixels {interior_empty}",
                start.elapsed()
            );
            write_png(
                &format!("stock_{progress}_{label}.png"),
                view.size.width(),
                view.size.height(),
                &rgba,
            );
            if xy == 2 {
                let mut averaged = Vec::with_capacity(600 * 600 * 4);
                for y in 0..600usize {
                    for x in 0..600usize {
                        let mut sum = [0u32; 4];
                        for dy in 0..2 {
                            for dx in 0..2 {
                                let i = ((y * 2 + dy) * 1200 + x * 2 + dx) * 4;
                                for c in 0..4 {
                                    sum[c] += u32::from(rgba[i + c]);
                                }
                            }
                        }
                        averaged.extend(sum.map(|n| ((n + 2) / 4) as u8));
                    }
                }
                write_png(
                    &format!("stock_{progress}_ss2_average.png"),
                    600,
                    600,
                    &averaged,
                );
            }
        }
        // Measure how long an in-flight voxel call takes to notice cancellation.
        let view = volume_view(&preview, camera, PixelRenderSize::from(600));
        for multiplier in [1, 4] {
            let view = refine_depth(
                volume_view(&preview, camera, PixelRenderSize::from(600)),
                multiplier,
            )
            .unwrap();
            for _ in 0..3 {
                let cancel = incremental::Cancellation::default();
                let token = cancel.clone();
                let trigger = std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(20));
                    let when = Instant::now();
                    token.cancel();
                    when
                });
                let result = compiled.render(&view, &cancel);
                let returned = Instant::now();
                let requested = trigger.join().unwrap();
                eprintln!(
                    "{progress} Zx{multiplier}: cancellation return {:?}, cancelled {}",
                    returned.saturating_duration_since(requested),
                    result.is_none()
                );
            }
        }
        if progress == 1.0 {
            for tile_sizes in [
                &[128, 64, 32, 16, 8][..],
                &[64, 32, 16, 8][..],
                &[32, 16, 8][..],
            ] {
                let config = VoxelRenderConfig {
                    world_to_model: view.world_to_model,
                    ..VoxelRenderConfig::from_size(view.size)
                };
                let eval = fidget_engine::raster::voxel::EvalConfig {
                    tile_sizes: Some(fidget_engine::render::TileSizes::new(tile_sizes).unwrap()),
                    ..Default::default()
                };
                let start = Instant::now();
                let image = fidget_engine::raster::voxel::render(
                    compiled.objects[0].0.clone().try_into().unwrap(),
                    &config,
                    &eval,
                )
                .unwrap();
                eprintln!("tiles {tile_sizes:?}: full render {:?}", start.elapsed());
                let shade = shading(&config);
                let rgba: Vec<_> = image
                    .iter()
                    .flat_map(|p| shade(*p, [190, 165, 110]))
                    .collect();
                write_png(
                    &format!("stock_tiles_{}.png", tile_sizes[0]),
                    600,
                    600,
                    &rgba,
                );
                let token = eval.cancel.clone();
                let trigger = std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(20));
                    let when = Instant::now();
                    token.cancel();
                    when
                });
                let result = fidget_engine::raster::voxel::render(
                    compiled.objects[0].0.clone().try_into().unwrap(),
                    &config,
                    &eval,
                );
                let returned = Instant::now();
                let requested = trigger.join().unwrap();
                eprintln!(
                    "tiles {tile_sizes:?}: cancellation {:?}, cancelled {}",
                    returned.saturating_duration_since(requested),
                    result.is_none()
                );
            }
        }
    }
}
