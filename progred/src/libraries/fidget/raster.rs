//! Owned image requests for background implicit rendering; no editor state crosses threads.

use super::*;
use incremental::background::Progress;

mod partial;
#[cfg(feature = "cam-profile")]
pub(crate) mod performance;
#[cfg(test)]
mod reference;
use puri::mesh::DepthImage;

// Software rendering evaluates with Fidget's VM, on every platform. Its JIT
// rendered the CAM stock four times slower on Apple Silicon, and won only on
// shapes that render in a millisecond either way; see
// docs/history/fidget-jit-vm-2026-10-11.md.
use fidget_engine::vm::{VmFunction, VmShape};

fn software_tiles() -> Option<fidget_engine::render::TileSizes> {
    Some(fidget_engine::render::TileSizes::new(&[32, 16, 8]).unwrap())
}

#[derive(Clone, PartialEq)]
pub(crate) struct Request {
    pub preview: VolumePreview,
    camera: Camera,
    pixels: PixelRenderSize,
}

/// Framing and resolution only; changing these does not require recompilation.
#[derive(Clone, PartialEq)]
pub(crate) struct ViewRequest {
    min: Vector3<f32>,
    max: Vector3<f32>,
    camera: Camera,
    pixels: PixelRenderSize,
}

/// Image-quality progression, independent of tile publication and cancellation.
#[derive(Clone, Copy)]
pub(crate) enum Passes {
    Final,
    Progressive { first_max_edge: u32 },
}

/// Immutable compiled objects and root tapes, reusable across camera requests.
/// Spatial specialization and render scratch remain local to each render job.
pub(crate) struct SoftwareScene {
    scene: fidget_engine::raster::voxel::Scene<'static, VmFunction>,
    colors: Vec<[u8; 3]>,
}

impl SoftwareScene {
    pub fn new(objects: &[SceneObject], cancel: &incremental::Cancellation) -> Option<Self> {
        let mut compiled = Vec::with_capacity(objects.len());
        for object in objects {
            cancel.check().ok()?;
            compiled.push(VmShape::from(object.tree.clone()).try_into().ok()?);
        }
        cancel.check().ok()?;
        Some(Self {
            scene: fidget_engine::raster::voxel::Scene::new(compiled, &render_cancel(cancel))?,
            colors: objects.iter().map(|o| o.color).collect(),
        })
    }

    pub(super) fn render(
        &self,
        view: &VolumeView,
        cancellation: &incremental::Cancellation,
    ) -> Option<Vec<u8>> {
        self.render_with_progress(view, cancellation, None)
    }

    fn render_with_progress(
        &self,
        view: &VolumeView,
        cancellation: &incremental::Cancellation,
        progress: Option<&(dyn Fn(Progress) + Sync)>,
    ) -> Option<Vec<u8>> {
        let config = VoxelRenderConfig {
            world_to_model: view.world_to_model,
            ..VoxelRenderConfig::from_size(view.size)
        };
        let report = |completed, total| {
            if let Some(progress) = progress {
                progress(Progress { completed, total });
            }
        };
        let eval = fidget_engine::raster::voxel::EvalConfig {
            cancel: render_cancel(cancellation),
            tile_sizes: software_tiles(),
            progress: progress.map(|_| &report as &(dyn Fn(usize, usize) + Sync)),
            ..Default::default()
        };
        let pixels = self.scene.render(&config, &eval, None)?;
        let shade = shading(&config);
        Some(
            pixels
                .iter()
                .flat_map(|pixel| {
                    shade(
                        pixel.geometry,
                        pixel.object.map_or([255; 3], |i| self.colors[i]),
                    )
                })
                .collect(),
        )
    }

    fn render_tiles(
        &self,
        view: &VolumeView,
        cancellation: &incremental::Cancellation,
        progress: Option<&(dyn Fn(Progress) + Sync)>,
        tile_ready: &(dyn Fn(fidget_engine::raster::voxel::SceneTile<'_>) + Sync),
    ) -> Option<()> {
        let cancel = render_cancel(cancellation);
        let config = VoxelRenderConfig {
            world_to_model: view.world_to_model,
            ..VoxelRenderConfig::from_size(view.size)
        };
        // UI updates are bounded, not one whole editor frame per tile. Always
        // report stage boundaries; no timer or polling loop is needed.
        let last_report = std::sync::Mutex::new(web_time::Instant::now());
        let report = |completed, total| {
            if let Some(progress) = progress {
                let boundary = completed == 0 || completed == total;
                let report = {
                    let mut last = last_report.lock().unwrap();
                    let now = web_time::Instant::now();
                    let report = boundary
                        || now.duration_since(*last) >= std::time::Duration::from_millis(50);
                    if report {
                        *last = now;
                    }
                    report
                };
                if report {
                    progress(Progress { completed, total });
                }
            }
        };
        let eval = fidget_engine::raster::voxel::EvalConfig {
            cancel,
            tile_sizes: software_tiles(),
            progress: progress.map(|_| &report as &(dyn Fn(usize, usize) + Sync)),
            ..Default::default()
        };
        self.scene.render_tiles(&config, &eval, tile_ready)
    }
}

fn render_cancel(cancellation: &incremental::Cancellation) -> fidget_engine::render::CancelToken {
    fidget_engine::render::CancelToken::from_shared_flag(cancellation.shared_flag().clone())
}

fn shading(config: &VoxelRenderConfig) -> impl Fn(GeometryPixel, [u8; 3]) -> [u8; 4] + use<> {
    let transform = config.mat();
    // Gradients are in sample coordinates; lighting is in the camera's orthonormal axes.
    let mut normal_scale =
        Vector3::from_fn(|axis, _| transform.fixed_view::<3, 1>(0, axis).norm().recip());
    // Fidget's sample Y points down the image; the mesh renderer's camera Y
    // points up. Undo that reflection as well as the unequal sample spacing.
    normal_scale.y = -normal_scale.y;
    let light = Vector3::new(0.35, -0.45, 1.0).normalize();
    move |pixel, color| {
        if pixel.depth == 0 {
            [0; 4]
        } else {
            let normal = Vector3::from(pixel.normal)
                .component_mul(&normal_scale)
                .normalize();
            let intensity = 0.22 + 0.78 * normal.dot(&light).max(0.0);
            let [r, g, b] = color.map(|channel| (f32::from(channel) * intensity) as u8);
            [r, g, b, 255]
        }
    }
}

fn refine_depth(mut view: VolumeView, multiplier: u32) -> Option<VolumeView> {
    let depth = view
        .size
        .depth()
        .checked_mul(multiplier)
        .filter(|&d| d > 0)?;
    view.size = VoxelRenderSize::new(view.size.width(), view.size.height(), depth);
    view.world_to_model *= Scale3::new(1.0, 1.0, 1.0 / multiplier as f32).to_homogeneous();
    Some(view)
}

impl Request {
    pub fn new(preview: VolumePreview, state: Option<&Value>, scale: f64) -> Option<Self> {
        Some(Self {
            pixels: raster_size(preview.size, scale)?,
            preview,
            camera: camera(state),
        })
    }

    pub fn view(&self) -> ViewRequest {
        ViewRequest {
            min: self.preview.min,
            max: self.preview.max,
            camera: self.camera,
            pixels: self.pixels,
        }
    }

    /// One-shot rendering for diagnostics without a dependency-tracked scene.
    #[cfg(any(test, feature = "cam-profile"))]
    pub fn render_software_tiles(
        &self,
        passes: Passes,
        final_depth_multiplier: u32,
        cancel: &incremental::Cancellation,
        publish: &mut (dyn FnMut(DepthImage) -> Result<(), incremental::Error> + Send),
        progress: Option<&(dyn Fn(Progress) + Sync)>,
    ) -> Result<Option<DepthImage>, incremental::Error> {
        cancel.check()?;
        let scene = SoftwareScene::new(&self.preview.objects, cancel);
        cancel.check()?;
        let Some(scene) = scene else { return Ok(None) };
        self.view().render(
            &scene,
            passes,
            final_depth_multiplier,
            cancel,
            publish,
            progress,
        )
    }
}

impl ViewRequest {
    fn resolutions(&self, first_max_edge: u32) -> Vec<PixelRenderSize> {
        assert!(first_max_edge > 0);
        let mut sizes = vec![self.pixels];
        let mut size = self.pixels;
        while size.width().max(size.height()) > first_max_edge {
            size = PixelRenderSize::new(size.width().div_ceil(2), size.height().div_ceil(2));
            sizes.push(size);
        }
        sizes.reverse();
        sizes
    }

    fn view_at(&self, pixels: PixelRenderSize) -> VolumeView {
        let mut view = volume_view_bounds(self.min, self.max, self.camera, pixels);
        // Pixel rounding must not change the camera's aspect ratio between passes.
        if pixels != self.pixels {
            let ratio = pixels.height() as f32 / self.pixels.height() as f32;
            let half_pixel = 1.0 / pixels.width().min(pixels.height()) as f32;
            // Adjust the frustum before the pixel-center translation.
            view.world_to_model *= Translation3::new(-half_pixel, 0.0, 0.0).to_homogeneous()
                * Scale3::new(
                    self.pixels.width() as f32 / pixels.width() as f32 * ratio,
                    1.0,
                    1.0,
                )
                .to_homogeneous()
                * Translation3::new(half_pixel, 0.0, 0.0).to_homogeneous();
        }
        view
    }

    fn refinements(
        &self,
        first_max_edge: u32,
        final_depth_multiplier: u32,
    ) -> Option<Vec<VolumeView>> {
        let mut views: Vec<_> = self
            .resolutions(first_max_edge)
            .into_iter()
            .map(|pixels| self.view_at(pixels))
            .collect();
        if final_depth_multiplier != 1 {
            views.push(refine_depth(
                self.view_at(self.pixels),
                final_depth_multiplier,
            )?);
        }
        Some(views)
    }
    /// Publish completed tile batches, retaining the previous level in
    /// unfinished regions. The first level reports coverage for a mesh fallback.
    pub fn render(
        &self,
        scene: &SoftwareScene,
        passes: Passes,
        final_depth_multiplier: u32,
        cancel: &incremental::Cancellation,
        publish: &mut (dyn FnMut(DepthImage) -> Result<(), incremental::Error> + Send),
        progress: Option<&(dyn Fn(Progress) + Sync)>,
    ) -> Result<Option<DepthImage>, incremental::Error> {
        cancel.check()?;
        let views = match passes {
            Passes::Final => refine_depth(self.view_at(self.pixels), final_depth_multiplier)
                .map(|view| vec![view]),
            Passes::Progressive { first_max_edge } => {
                self.refinements(first_max_edge, final_depth_multiplier)
            }
        };
        let Some(views) = views else {
            return Ok(None);
        };
        let mut previous = None;
        let mut views = views.into_iter().peekable();
        while let Some(view) = views.next() {
            cancel.check()?;
            let frame =
                partial::render(scene, &view, previous.as_ref(), cancel, publish, progress)?;
            let Some(frame) = frame else { return Ok(None) };
            if views.peek().is_none() {
                return Ok(Some(frame));
            }
            publish(frame.clone())?;
            previous = Some(frame);
        }
        unreachable!("the native resolution is always present")
    }
}

#[cfg(test)]
mod tests;
