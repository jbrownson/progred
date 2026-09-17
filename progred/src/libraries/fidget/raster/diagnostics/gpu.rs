//! Headless correctness/timing comparison; never selected by the app.
use super::*;
use fidget_engine::compiler::RegOp;

pub(crate) fn compare_gpu(preview: &VolumePreview) {
    let depth: u32 = std::env::var("CAM_GPU_DEPTH")
        .unwrap_or("1".into())
        .parse()
        .unwrap();
    let view = refine_depth(
        volume_view(
            preview,
            Camera::default(),
            raster_size(preview.size, 1.0).unwrap(),
        ),
        depth,
    )
    .unwrap();
    let settings = VoxelRenderConfig {
        world_to_model: view.world_to_model,
        ..VoxelRenderConfig::from_size(view.size)
    };
    let settings = match std::env::var("CAM_GPU_REGION") {
        Ok(region) => {
            let region: Vec<u32> = region.split(',').map(|v| v.parse().unwrap()).collect();
            assert_eq!(region.len(), 3, "CAM_GPU_REGION is x,y,side");
            voxel::diagnostics::crop(&settings, region[0], region[1], region[2])
        }
        Err(_) => settings,
    };
    let profile = std::env::var_os("CAM_GPU_PROFILE").is_some();
    let batch = std::env::var("CAM_GPU_BATCH")
        .ok()
        .map(|v| v.parse::<std::num::NonZeroU32>().unwrap());
    let gpu = if profile {
        pollster::block_on(Gpu::init()).unwrap()
    } else {
        pollster::block_on(Gpu::init_basic()).unwrap()
    };
    let ctx = voxel::Context::new(&gpu);
    let mut workspace = ctx.workspace();
    let mut read = gpu.read_buffer("CAM GPU diagnostic");
    let mut cpu_total = Duration::ZERO;
    let mut gpu_total = Duration::ZERO;
    for (index, object) in preview.objects.iter().enumerate() {
        let shape = VmShape::from(object.tree.clone());
        let data = shape.inner().data();
        let spills = data
            .iter_asm()
            .filter(|op| matches!(op, RegOp::Load(..) | RegOp::Store(..)))
            .count();
        eprintln!(
            "object {index}: {} ops, {spills} loads/stores, {} slots, {} choices",
            data.len(),
            data.slot_count(),
            data.choice_count()
        );
        let words = std::env::var("CAM_GPU_CHOICE_WORDS")
            .unwrap_or("32".into())
            .parse()
            .unwrap();
        let render_shape = RenderShape::new(&shape)
            .unwrap()
            .with_choice_stack_words(std::num::NonZeroU32::new(words).unwrap());
        eprintln!(
            "object {index}: GPU needs {} spill slots; {words} choice words",
            render_shape.spill_count()
        );
        let cpu_shape = SoftwareShape::from(object.tree.clone());
        let eval = fidget_engine::raster::voxel::EvalConfig {
            tile_sizes: software_tiles(),
            ..Default::default()
        };
        let start = Instant::now();
        let prepared = fidget_engine::raster::voxel::Scene::new(
            vec![cpu_shape.try_into().unwrap()],
            &eval.cancel,
        )
        .unwrap();
        eprintln!("object {index}: CPU root preparation {:?}", start.elapsed());
        let reference = (0..2)
            .map(|attempt| {
                let start = Instant::now();
                let image = prepared.render(&settings, &eval, None).unwrap();
                if attempt == 1 {
                    cpu_total += start.elapsed();
                }
                eprintln!(
                    "object {index}: CPU attempt {attempt} {:?}",
                    start.elapsed()
                );
                image.iter().map(|p| p.geometry).collect::<Vec<_>>()
            })
            .last()
            .unwrap();
        eprintln!(
            "object {index}: submitting GPU {}x{}x{}",
            settings.image_size.width(),
            settings.image_size.height(),
            settings.image_size.depth()
        );
        for attempt in 0..2 {
            let start = Instant::now();
            let image = if let Some(side) = batch.filter(|_| attempt == 1) {
                fidget_wgpu::voxel::diagnostics::batched(
                    &ctx,
                    &render_shape,
                    &mut workspace,
                    &mut read,
                    &settings,
                    side,
                )
                .unwrap()
            } else if profile && attempt == 1 {
                fidget_wgpu::voxel::diagnostics::profile(
                    &ctx,
                    &render_shape,
                    &mut workspace,
                    &mut read,
                    &settings,
                )
                .unwrap()
            } else {
                ctx.run(&render_shape, &mut workspace, &mut read, settings.clone())
                    .unwrap()
            };
            let elapsed = start.elapsed();
            if attempt == 1 {
                gpu_total += elapsed;
            }
            let coverage = image
                .iter()
                .zip(reference.iter())
                .filter(|(a, b)| (a.depth == 0) != (b.depth == 0))
                .count();
            // These backends use different depth encodings for the same sample.
            let depth = |d: u32| if d == 0 { 0 } else { d + 1 };
            let depths = image
                .iter()
                .zip(reference.iter())
                .filter(|(a, b)| depth(a.depth) != b.depth)
                .count();
            let max_depth = image
                .iter()
                .zip(reference.iter())
                .map(|(a, b)| depth(a.depth).abs_diff(b.depth))
                .max()
                .unwrap_or(0);
            let normal = image
                .iter()
                .zip(reference.iter())
                .filter(|(a, b)| {
                    a.depth > 0
                        && b.depth > 0
                        && a.normal
                            .iter()
                            .zip(&b.normal)
                            .any(|(a, b)| !a.is_finite() || (a - b).abs() > 0.0001)
                })
                .count();
            eprintln!(
                "object {index}: GPU attempt {attempt} {elapsed:?}; coverage differences {coverage}, depths {depths}, max delta {max_depth}, normals {normal}; workspace {} MiB",
                workspace.capacity() / 1024 / 1024
            );
            assert_eq!(coverage, 0, "GPU silhouette differs from CPU");
            assert_eq!(depths, 0, "GPU and CPU sampled different depths");
            assert_eq!(normal, 0, "GPU and CPU sampled different normals");
        }
    }
    eprintln!(
        "CAM {} objects {}x{}x{}: CPU {cpu_total:?}, warm GPU {gpu_total:?}",
        preview.objects.len(),
        settings.image_size.width(),
        settings.image_size.height(),
        settings.image_size.depth()
    );
}
