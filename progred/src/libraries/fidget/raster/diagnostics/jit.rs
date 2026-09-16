//! Opt-in VM/JIT comparison over the real CAM stock (CAM_JIT=1).
use super::*;
use fidget_engine::{eval::Function, render::RenderHints, shape::Shape};

pub(crate) fn compare_jit(preview: &VolumePreview) {
    use fidget_engine::jit::JitShape;
    let jit_tiles = || {
        if std::env::var("CAM_JIT_DEFAULT_TILES").is_ok() {
            None
        } else {
            Some(fidget_engine::render::TileSizes::new(&[32, 16, 8]).unwrap())
        }
    };
    let view = volume_view(
        preview,
        Camera::default(),
        raster_size(preview.size, 1.0).unwrap(),
    );
    let start = Instant::now();
    let vm: Vec<_> = preview
        .objects
        .iter()
        .map(|o| (VmShape::from(o.tree.clone()), o.color))
        .collect();
    eprintln!("VM compile {:?}", start.elapsed());
    let start = Instant::now();
    let shapes: Vec<_> = preview
        .objects
        .iter()
        .map(|o| (JitShape::from(o.tree.clone()), o.color))
        .collect();
    eprintln!("JIT shape construction {:?}", start.elapsed());
    let refined = refine_depth(
        VolumeView {
            size: view.size,
            world_to_model: view.world_to_model,
        },
        4,
    )
    .unwrap();
    for (label, view) in [("native", view), ("z4", refined)] {
        let start = Instant::now();
        let reference = render(
            &vm,
            &view,
            Some(fidget_engine::render::TileSizes::new(&[32, 16, 8]).unwrap()),
        );
        eprintln!("VM {label} {:?}", start.elapsed());
        let start = Instant::now();
        let image = render(&shapes, &view, jit_tiles());
        let elapsed = start.elapsed();
        let differences = reference
            .chunks_exact(4)
            .zip(image.chunks_exact(4))
            .filter(|(a, b)| a != b)
            .count();
        let coverage = reference
            .chunks_exact(4)
            .zip(image.chunks_exact(4))
            .filter(|(a, b)| a[3] != b[3])
            .count();
        eprintln!(
            "JIT {label} {elapsed:?}; differences vs VM same quality: {differences}, coverage {coverage}"
        );
        assert_eq!(differences, 0);
    }
    let settings = fidget_engine::mesh::Settings {
        depth: 6,
        world_to_model: Translation3::from((preview.min + preview.max) / 2.0).to_homogeneous()
            * Scale3::from((preview.max - preview.min) / 2.0).to_homogeneous(),
        ..Default::default()
    };
    for ((shape, _), (reference, _)) in shapes.iter().zip(&vm) {
        let start = Instant::now();
        let reference =
            fidget_engine::mesh::Octree::build(&reference.clone().try_into().unwrap(), &settings)
                .unwrap()
                .walk_dual();
        eprintln!(
            "VM mesh {:?}, {} vertices",
            start.elapsed(),
            reference.vertices.len()
        );
        let start = Instant::now();
        let mesh =
            fidget_engine::mesh::Octree::build(&shape.clone().try_into().unwrap(), &settings)
                .unwrap()
                .walk_dual();
        eprintln!(
            "JIT mesh {:?}, {} vertices",
            start.elapsed(),
            mesh.vertices.len()
        );
        assert_eq!(reference.vertices, mesh.vertices);
        assert_eq!(reference.triangles, mesh.triangles);
    }
    let view = volume_view(
        preview,
        Camera::default(),
        raster_size(preview.size, 1.0).unwrap(),
    );
    let config = VoxelRenderConfig {
        world_to_model: view.world_to_model,
        ..VoxelRenderConfig::from_size(view.size)
    };
    for jit in [false, true] {
        let token = fidget_engine::render::CancelToken::new();
        let eval = fidget_engine::raster::voxel::EvalConfig {
            cancel: token.clone(),
            tile_sizes: if jit {
                jit_tiles()
            } else {
                Some(fidget_engine::render::TileSizes::new(&[32, 16, 8]).unwrap())
            },
            ..Default::default()
        };
        let timer = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            let when = Instant::now();
            token.cancel();
            when
        });
        let output = if jit {
            fidget_engine::raster::voxel::render(
                shapes[0].0.clone().try_into().unwrap(),
                &config,
                &eval,
            )
        } else {
            fidget_engine::raster::voxel::render(
                vm[0].0.clone().try_into().unwrap(),
                &config,
                &eval,
            )
        };
        let stopped = Instant::now();
        let cancelled = timer.join().unwrap();
        assert!(output.is_none());
        eprintln!(
            "{} cancellation response {:?}",
            if jit { "JIT" } else { "VM" },
            stopped.saturating_duration_since(cancelled)
        );
    }
}

fn render<F: Function + RenderHints>(
    shapes: &[(Shape<F>, [u8; 3])],
    view: &VolumeView,
    tile_sizes: Option<fidget_engine::render::TileSizes>,
) -> Vec<u8> {
    let config = VoxelRenderConfig {
        world_to_model: view.world_to_model,
        ..VoxelRenderConfig::from_size(view.size)
    };
    let eval = fidget_engine::raster::voxel::EvalConfig {
        tile_sizes,
        ..Default::default()
    };
    let mut image = vec![
        (GeometryPixel::default(), [0; 3]);
        view.size.width() as usize * view.size.height() as usize
    ];
    for (shape, color) in shapes {
        let pixels =
            fidget_engine::raster::voxel::render(shape.clone().try_into().unwrap(), &config, &eval)
                .unwrap();
        for (dst, src) in image.iter_mut().zip(pixels.iter()) {
            if src.depth > dst.0.depth {
                *dst = (*src, *color);
            }
        }
    }
    let shade = shading(&config);
    image
        .iter()
        .flat_map(|(pixel, color)| shade(*pixel, *color))
        .collect()
}
