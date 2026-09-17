//! Worker-local assembly of finished tiles, with bounded snapshot publication.
use super::*;
use fidget_engine::raster::voxel::SceneTile;
use std::sync::Mutex;

#[derive(Clone, Copy, Debug)]
struct Area {
    origin: [usize; 2],
    size: [usize; 2],
}

/// A raster and, only during its first pass, the regions actually computed.
/// Transparent pixels in those regions are final pixels, not missing data.
#[derive(Clone)]
pub(crate) struct Frame {
    pub image: ImageData,
    coverage: Option<Vec<Area>>,
}

impl From<ImageData> for Frame {
    fn from(image: ImageData) -> Self {
        Self {
            image,
            coverage: None,
        }
    }
}

impl Frame {
    pub fn is_partial(&self) -> bool {
        self.coverage.is_some()
    }

    /// Replace covered regions of a fallback, including transparent pixels.
    /// The fallback may have a different resolution, but the same camera view.
    pub fn over(&self, fallback: &ImageData) -> ImageData {
        let Some(coverage) = &self.coverage else {
            return self.image.clone();
        };
        let mut pixels = fallback.data.data().to_vec();
        let width = fallback.width as usize;
        let height = fallback.height as usize;
        let source_width = self.image.width as usize;
        let source_height = self.image.height as usize;
        for area in coverage {
            let [x0, y0] = area.origin;
            let [w, h] = area.size;
            // Destination samples use floor(x * source_width / width).
            for y in
                (y0 * height).div_ceil(source_height)..((y0 + h) * height).div_ceil(source_height)
            {
                for x in
                    (x0 * width).div_ceil(source_width)..((x0 + w) * width).div_ceil(source_width)
                {
                    let src = ((y * source_height / height) * source_width
                        + x * source_width / width)
                        * 4;
                    let dst = (y * width + x) * 4;
                    pixels[dst..dst + 4].copy_from_slice(&self.image.data.data()[src..src + 4]);
                }
            }
        }
        image(width as u32, height as u32, pixels)
    }
}

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
    coverage: Option<Vec<Area>>,
    completed: usize,
}

struct Publication<'a> {
    assembly: Assembly,
    publish: &'a mut (dyn FnMut(Frame) -> Result<(), incremental::Error> + Send),
    error: Option<incremental::Error>,
    #[cfg(not(target_arch = "wasm32"))]
    last: std::time::Instant,
}

impl Publication<'_> {
    fn maybe_publish(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        let due = self.last.elapsed() >= std::time::Duration::from_millis(100);
        #[cfg(target_arch = "wasm32")]
        let due = false; // Browser rendering is inline; no mid-pass presentation.
        if due && self.assembly.completed < self.assembly.width * self.assembly.height {
            self.error = (self.publish)(self.assembly.snapshot()).err();
            #[cfg(not(target_arch = "wasm32"))]
            {
                self.last = std::time::Instant::now();
            }
        }
    }
}

impl Assembly {
    fn new(width: usize, height: usize, previous: Option<&ImageData>) -> Self {
        let mut pixels = vec![0; width * height * 4];
        if let Some(previous) = previous {
            for y in 0..height {
                for x in 0..width {
                    let src = ((y * previous.height as usize / height) * previous.width as usize
                        + x * previous.width as usize / width)
                        * 4;
                    let dst = (y * width + x) * 4;
                    pixels[dst..dst + 4].copy_from_slice(&previous.data.data()[src..src + 4]);
                }
            }
        }
        Self {
            width,
            height,
            pixels,
            coverage: previous.is_none().then(Vec::new),
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
            }
        }
        self.completed += tile.size[0] * tile.size[1];
        if let Some(coverage) = &mut self.coverage {
            coverage.push(Area {
                origin: tile.origin,
                size: tile.size,
            });
        }
    }

    fn snapshot(&self) -> Frame {
        Frame {
            image: image(self.width as u32, self.height as u32, self.pixels.clone()),
            coverage: self.coverage.clone(),
        }
    }
}

pub(super) fn render(
    scene: &SoftwareScene,
    view: &VolumeView,
    previous: Option<&ImageData>,
    cancel: &incremental::Cancellation,
    publish: &mut (dyn FnMut(Frame) -> Result<(), incremental::Error> + Send),
    progress: Option<&(dyn Fn(Progress) + Sync)>,
) -> Result<Option<Frame>, incremental::Error> {
    let assembly = Assembly::new(
        view.size.width() as usize,
        view.size.height() as usize,
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
        #[cfg(not(target_arch = "wasm32"))]
        last: std::time::Instant::now(),
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
    Ok(output.map(|()| image(view.size.width(), view.size.height(), state.assembly.pixels).into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fidget_engine::raster::voxel::ScenePixel;

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn publication_preserves_callback_failure() {
        let mut count = 0;
        let mut publish = |_: Frame| {
            count += 1;
            Err(incremental::Error::Cancelled)
        };
        let mut state = Publication {
            assembly: Assembly::new(2, 1, None),
            publish: &mut publish,
            error: None,
            last: std::time::Instant::now() - std::time::Duration::from_secs(1),
        };
        state.assembly.completed = 1;
        state.maybe_publish();
        assert!(matches!(state.error, Some(incremental::Error::Cancelled)));
        drop(state);
        assert_eq!(count, 1);
    }

    #[test]
    fn finished_transparent_tiles_erase_fallback_but_unfinished_tiles_do_not() {
        let fallback = image(7, 3, [200, 40, 20, 255].repeat(21));
        let mut assembly = Assembly::new(3, 2, None);
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
        let combined = frame.over(&fallback);
        for y in 0..3 {
            for x in 0..7 {
                let offset = (y * 7 + x) * 4;
                let expected = if x * 3 / 7 == 1 {
                    [0; 4]
                } else {
                    [200, 40, 20, 255]
                };
                assert_eq!(&combined.data.data()[offset..offset + 4], &expected);
            }
        }
    }

    #[test]
    fn finer_pass_preserves_previous_pixels_until_replaced_including_empty_pixels() {
        let previous = image(2, 1, vec![10, 20, 30, 255, 40, 50, 60, 255]);
        let mut assembly = Assembly::new(4, 2, Some(&previous));
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
        let assembly = Mutex::new(Assembly::new(41, 27, None));
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
