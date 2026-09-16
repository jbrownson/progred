//! Compare the old object-major traversal with tile-major scene rendering.
use super::*;
use std::sync::Mutex;

// Deliberately retained only as a diagnostic reference for both pixels and time.
fn object_major(
    scene: &SoftwareScene,
    view: &VolumeView,
    progress: &(dyn Fn(Progress) + Sync),
) -> Vec<u8> {
    let config = VoxelRenderConfig {
        world_to_model: view.world_to_model,
        ..VoxelRenderConfig::from_size(view.size)
    };
    let mut image = vec![
        (GeometryPixel::default(), [255; 3]);
        view.size.width() as usize * view.size.height() as usize
    ];
    for (object, (shape, color)) in scene.objects.iter().enumerate() {
        let report = |completed, total| {
            progress(Progress {
                completed: object * total + completed,
                total: scene.objects.len() * total,
            })
        };
        let eval = fidget_engine::raster::voxel::EvalConfig {
            tile_sizes: software_tiles(),
            progress: Some(&report),
            ..Default::default()
        };
        let geometry =
            fidget_engine::raster::voxel::render(shape.clone().try_into().unwrap(), &config, &eval)
                .unwrap();
        for (dst, src) in image.iter_mut().zip(geometry.iter()) {
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

pub(crate) fn compare_scene_tiles(preview: &VolumePreview) {
    let cancel = incremental::Cancellation::default();
    let scene = SoftwareScene::new(&preview.objects, &cancel).unwrap();
    for (label, multiplier) in [("native", 1), ("depth4", 4)] {
        let view = volume_view(
            preview,
            Camera::default(),
            raster_size(preview.size, 1.0).unwrap(),
        );
        let view = refine_depth(view, multiplier).unwrap();
        let mut reference: Option<Vec<u8>> = None;
        for round in 0..3 {
            for tile_major in if round % 2 == 0 {
                [false, true]
            } else {
                [true, false]
            } {
                let start = Instant::now();
                let milestones = Mutex::new(Vec::new());
                let thresholds = [0.25, 0.5, 0.75, 0.99, 1.0];
                let progress = |p: Progress| {
                    let mut milestones = milestones.lock().unwrap();
                    while milestones.len() < thresholds.len()
                        && p.fraction() >= thresholds[milestones.len()]
                    {
                        milestones.push(start.elapsed());
                    }
                };
                let pixels = if tile_major {
                    scene
                        .render_with_progress(&view, &cancel, Some(&progress))
                        .unwrap()
                } else {
                    object_major(&scene, &view, &progress)
                };
                eprintln!(
                    "scene {label} round {round} {}: {:?}; at 25/50/75/99/100% {:?}",
                    if tile_major { "tiles" } else { "objects" },
                    start.elapsed(),
                    milestones.into_inner().unwrap()
                );
                if let Some(reference) = &reference {
                    assert_eq!(pixels, *reference, "scene pixels differ");
                } else {
                    reference = Some(pixels);
                }
            }
        }
    }
}
