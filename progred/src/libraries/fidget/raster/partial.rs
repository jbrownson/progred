//! Worker-local assembly of finished tiles, with bounded snapshot publication.
use super::*;
use fidget_engine::raster::voxel::SceneTile;
use std::sync::Mutex;

fn image(width: u32, height: u32, pixels: Vec<u8>) -> ImageData {
    ImageData {
        data: pixels.into(),
        format: ImageFormat::Rgba8,
        alpha_type: ImageAlphaType::Alpha,
        width,
        height,
    }
}

struct Assembly {
    width: usize,
    height: usize,
    pixels: Vec<u8>,
    depth: Vec<f32>,
    depth_count: u32,
    partial: bool,
    completed: usize,
}

struct Publication<'a> {
    assembly: Assembly,
    publish: &'a mut (dyn FnMut(Frame) -> Result<(), incremental::Error> + Send),
    error: Option<incremental::Error>,
    last: web_time::Instant,
}

impl Publication<'_> {
    fn maybe_publish(&mut self) {
        let due = self.last.elapsed() >= std::time::Duration::from_millis(100);
        if due && self.assembly.completed < self.assembly.width * self.assembly.height {
            self.error = (self.publish)(self.assembly.snapshot()).err();
            self.last = web_time::Instant::now();
        }
    }
}

impl Assembly {
    fn new(width: usize, height: usize, depth_count: u32, previous: Option<&Frame>) -> Self {
        let mut pixels = vec![0; width * height * 4];
        let mut depth = vec![-1.0; width * height];
        if let Some(previous) = previous {
            let image = &previous.image;
            for y in 0..height {
                for x in 0..width {
                    let src = ((y * image.height as usize / height) * image.width as usize
                        + x * image.width as usize / width)
                        * 4;
                    let dst = (y * width + x) * 4;
                    pixels[dst..dst + 4].copy_from_slice(&image.data.data()[src..src + 4]);
                    depth[dst / 4] = previous.depth[src / 4];
                }
            }
        }
        Self {
            width,
            height,
            pixels,
            depth,
            depth_count,
            partial: previous.is_none(),
            completed: 0,
        }
    }

    fn put(
        &mut self,
        tile: SceneTile<'_>,
        shade: &impl Fn(GeometryPixel, [u8; 3]) -> [u8; 4],
        colors: &[[u8; 3]],
    ) {
        for y in 0..tile.size[1] {
            for x in 0..tile.size[0] {
                let pixel = tile.pixels[y * tile.stride + x];
                let color = pixel.object.map_or([255; 3], |i| colors[i]);
                let dst = ((tile.origin[1] + y) * self.width + tile.origin[0] + x) * 4;
                self.pixels[dst..dst + 4].copy_from_slice(&shade(pixel.geometry, color));
                // Fidget stores the front boundary of the occupied voxel, with
                // larger Z nearer the camera; zero denotes an empty ray.
                self.depth[dst / 4] = 1.0 - pixel.geometry.depth as f32 / self.depth_count as f32;
            }
        }
        self.completed += tile.size[0] * tile.size[1];
    }

    fn snapshot(&self) -> Frame {
        Frame {
            image: image(self.width as u32, self.height as u32, self.pixels.clone()),
            depth: self.depth.clone().into(),
            partial: self.partial,
        }
    }
}

pub(super) fn render(
    scene: &SoftwareScene,
    view: &VolumeView,
    previous: Option<&Frame>,
    cancel: &incremental::Cancellation,
    publish: &mut (dyn FnMut(Frame) -> Result<(), incremental::Error> + Send),
    progress: Option<&(dyn Fn(Progress) + Sync)>,
) -> Result<Option<Frame>, incremental::Error> {
    let assembly = Assembly::new(
        view.size.width() as usize,
        view.size.height() as usize,
        view.size.depth(),
        previous,
    );
    let shade = shading(&VoxelRenderConfig {
        world_to_model: view.world_to_model,
        ..VoxelRenderConfig::from_size(view.size)
    });
    // Publication is serialized with assembly: parallel workers cannot publish
    // an older snapshot after a newer one. No UI or editor state crosses here.
    let state = Mutex::new(Publication {
        assembly,
        publish,
        error: None,
        last: web_time::Instant::now(),
    });
    let tile_ready = |tile: SceneTile<'_>| {
        let mut state = state.lock().unwrap();
        if cancel.check().is_err() || state.error.is_some() {
            return;
        }
        state.assembly.put(tile, &shade, &scene.colors);
        state.maybe_publish();
        if state.error.is_some() {
            cancel.cancel();
        }
    };
    let output = scene.render_tiles(view, cancel, progress, &tile_ready);
    let state = state.into_inner().unwrap();
    if let Some(error) = state.error {
        return Err(error);
    }
    cancel.check()?;
    Ok(output.map(|()| Frame {
        image: image(view.size.width(), view.size.height(), state.assembly.pixels),
        depth: state.assembly.depth.into(),
        partial: false,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fidget_engine::raster::voxel::ScenePixel;

    #[test]
    fn publication_preserves_callback_failure() {
        let mut count = 0;
        let mut publish = |_: Frame| {
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
        assert!(frame.is_partial());
        assert_eq!(&*frame.depth, &[-1.0, 1.0, -1.0, -1.0, 1.0, -1.0]);
        assert!(frame.image.data.data().iter().all(|v| *v == 0));
    }

    #[test]
    fn finer_pass_preserves_previous_pixels_until_replaced_including_empty_pixels() {
        let previous = Frame {
            image: image(2, 1, vec![10, 20, 30, 255, 40, 50, 60, 255]),
            depth: vec![1.0; 2].into(),
            partial: false,
        };
        let mut assembly = Assembly::new(4, 2, 64, Some(&previous));
        assert!(!assembly.snapshot().is_partial());
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
            .render_software_progressive(12, 4, &cancel, &mut |_| Ok(()), None)
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
        assert!(!result.is_partial());
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
            .render_software_progressive(12, 4, &cancel, &mut |_| Ok(()), None)
            .unwrap()
            .unwrap();
        let reports = Mutex::new(Vec::new());
        let result = request
            .render_software_tiles(
                Passes::Final,
                4,
                &cancel,
                &mut |frame| {
                    assert!(frame.is_partial(), "no intermediate whole-image passes");
                    assert_eq!((frame.image.width, frame.image.height), (81, 53));
                    Ok(())
                },
                Some(&|progress| reports.lock().unwrap().push(progress)),
            )
            .unwrap()
            .unwrap();
        assert!(!result.is_partial());
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
        let scene = SoftwareScene::new(&request.preview.objects, &cancel).unwrap();
        let view = request.view_at(request.pixels);
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
}
