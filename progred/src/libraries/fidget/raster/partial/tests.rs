use super::*;
use fidget_engine::raster::voxel::ScenePixel;

#[test]
fn publication_preserves_callback_failure() {
    let mut count = 0;
    let mut publish = |_: DepthImage| {
        count += 1;
        Err(incremental::Error::Cancelled)
    };
    let mut state = Publication {
        assembly: Assembly::new(2, 1, 64, None),
        publish: &mut publish,
        error: None,
        last: web_time::Instant::now() - std::time::Duration::from_secs(1),
    };
    state.assembly.completed = 1;
    state.maybe_publish();
    assert!(matches!(state.error, Some(incremental::Error::Cancelled)));
    drop(state);
    assert_eq!(count, 1);
}

#[test]
fn finished_empty_pixels_are_distinct_from_unfinished_pixels() {
    let mut assembly = Assembly::new(3, 2, 64, None);
    assembly.put(
        SceneTile {
            origin: [1, 0],
            size: [1, 2],
            stride: 1,
            pixels: &[ScenePixel::default(); 2],
        },
        &|_, _| [0; 4],
        &[],
    );
    let frame = assembly.snapshot();
    assert!(frame.partial);
    assert_eq!(&*frame.depth, &[-1.0, 1.0, -1.0, -1.0, 1.0, -1.0]);
    assert!(frame.image.data.data().iter().all(|v| *v == 0));
}

#[test]
fn finer_pass_preserves_previous_pixels_until_replaced_including_empty_pixels() {
    let previous = DepthImage {
        image: rgba_image(2, 1, vec![10, 20, 30, 255, 40, 50, 60, 255]),
        depth: vec![1.0; 2].into(),
        partial: false,
    };
    let mut assembly = Assembly::new(4, 2, 64, Some(&previous));
    assert!(!assembly.snapshot().partial);
    assembly.put(
        SceneTile {
            origin: [1, 0],
            size: [2, 1],
            stride: 2,
            pixels: &[ScenePixel::default(); 2],
        },
        &|_, _| [0; 4],
        &[],
    );
    assert_eq!(
        assembly.pixels,
        [
            [10, 20, 30, 255],
            [0; 4],
            [0; 4],
            [40, 50, 60, 255],
            [10, 20, 30, 255],
            [10, 20, 30, 255],
            [40, 50, 60, 255],
            [40, 50, 60, 255],
        ]
        .concat()
    );
}

#[test]
fn tiled_progression_matches_whole_pass_reference_and_propagates_cancellation() {
    let request = super::super::tests::request(41.0, 27.0);
    let cancel = incremental::Cancellation::default();
    let reference = request
        .render_software_progressive(12, 4, &cancel, &mut |_| Ok(()))
        .unwrap()
        .unwrap();
    let result = request
        .render_software_tiles(
            Passes::Progressive { first_max_edge: 12 },
            4,
            &cancel,
            &mut |_| Ok(()),
            None,
        )
        .unwrap()
        .unwrap();
    assert!(!result.partial);
    assert_eq!(result.image.data.data(), reference.data.data());
    let mut count = 0;
    let result = request.render_software_tiles(
        Passes::Progressive { first_max_edge: 12 },
        4,
        &cancel,
        &mut |_| {
            count += 1;
            cancel.cancel();
            Ok(())
        },
        None,
    );
    assert!(matches!(result, Err(incremental::Error::Cancelled)));
    assert_eq!(count, 1);
}

#[test]
fn final_only_tiles_keep_final_quality_with_one_progress_interval() {
    let mut request = super::super::tests::request(81.0, 53.0);
    request.preview.objects = vec![
        SceneObject {
            tree: Tree::x().square() + Tree::y().square() + Tree::z().square() - 0.6,
            color: [200, 100, 50],
        },
        SceneObject {
            tree: Tree::x() + Tree::z(),
            color: [50, 100, 200],
        },
    ];
    let cancel = incremental::Cancellation::default();
    let reference = request
        .render_software_progressive(12, 4, &cancel, &mut |_| Ok(()))
        .unwrap()
        .unwrap();
    let reports = Mutex::new(Vec::new());
    let result = request
        .render_software_tiles(
            Passes::Final,
            4,
            &cancel,
            &mut |frame| {
                assert!(frame.partial, "no intermediate whole-image passes");
                assert_eq!((frame.image.width, frame.image.height), (81, 53));
                Ok(())
            },
            Some(&|progress| reports.lock().unwrap().push(progress)),
        )
        .unwrap()
        .unwrap();
    assert!(!result.partial);
    assert_eq!((result.image.width, result.image.height), (81, 53));
    assert_eq!(result.image.data.data(), reference.data.data());
    let reports = reports.into_inner().unwrap();
    assert_eq!(reports.iter().filter(|p| p.completed == 0).count(), 1);
    assert_eq!(reports.iter().filter(|p| p.completed == p.total).count(), 1);
    assert!(reports.iter().all(|p| p.total == 81 * 53));
    assert!(reports.windows(2).all(|p| p[0].completed <= p[1].completed));
}

#[test]
fn final_only_tiles_validate_depth_and_propagate_cancellation() {
    let request = super::super::tests::request(41.0, 27.0);
    let cancel = incremental::Cancellation::default();
    for depth in [0, u32::MAX] {
        assert!(
            request
                .render_software_tiles(
                    Passes::Final,
                    depth,
                    &cancel,
                    &mut |_| panic!("invalid depth must not publish"),
                    None,
                )
                .unwrap()
                .is_none()
        );
    }
    let result = request.render_software_tiles(
        Passes::Final,
        4,
        &cancel,
        &mut |_| panic!("cancelled pass must not publish"),
        Some(&|progress| {
            if progress.completed == 0 {
                cancel.cancel();
            }
        }),
    );
    assert!(matches!(result, Err(incremental::Error::Cancelled)));
}

#[test]
fn assembly_matches_renderer_at_every_pixel_with_parallel_out_of_order_tiles() {
    let request = super::super::tests::request(41.0, 27.0);
    let cancel = incremental::Cancellation::default();
    let scene = SoftwareScene::new(&request.preview.objects, &cancel)
        .unwrap()
        .unwrap();
    let view = request.view().view_at(request.pixels);
    let assembly = Mutex::new(Assembly::new(41, 27, view.size.depth(), None));
    let shade = shading(&VoxelRenderConfig {
        world_to_model: view.world_to_model,
        ..VoxelRenderConfig::from_size(view.size)
    });
    let ready = |tile: SceneTile<'_>| assembly.lock().unwrap().put(tile, &shade, &scene.colors);
    let reference = scene.render(&view, &cancel).unwrap();
    scene.render_tiles(&view, &cancel, None, &ready).unwrap();
    let assembly = assembly.into_inner().unwrap();
    assert_eq!(assembly.completed, 41 * 27);
    assert_eq!(assembly.pixels, reference);
}
