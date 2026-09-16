//! Owned image requests for background implicit rendering; no editor state crosses threads.

use super::*;

#[cfg(test)]
pub(crate) mod diagnostics;

// Meshing keeps VmShape independently; JIT benefits the much denser raster work.
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
use fidget_engine::jit::JitShape as SoftwareShape;
#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
use fidget_engine::vm::VmShape as SoftwareShape;

fn software_tiles() -> Option<fidget_engine::render::TileSizes> {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    // Use JIT's own recommendation; smaller VM tiles waste time generating code.
    return None;
    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    Some(fidget_engine::render::TileSizes::new(&[32, 16, 8]).unwrap())
}

#[derive(Clone, PartialEq)]
pub(crate) struct Request {
    pub preview: VolumePreview,
    camera: Camera,
    pixels: PixelRenderSize,
}

/// A worker-local compiled scene, shared by that request's resolution passes.
pub(super) struct SoftwareScene {
    objects: Vec<(SoftwareShape, [u8; 3])>,
}

impl SoftwareScene {
    pub fn new(objects: &[SceneObject], cancel: &incremental::Cancellation) -> Option<Self> {
        let mut compiled = Vec::with_capacity(objects.len());
        for object in objects {
            cancel.check().ok()?;
            compiled.push((SoftwareShape::from(object.tree.clone()), object.color));
        }
        cancel.check().ok()?;
        Some(Self { objects: compiled })
    }

    pub fn render(
        &self,
        view: &VolumeView,
        cancellation: &incremental::Cancellation,
    ) -> Option<Vec<u8>> {
        let cancel = fidget_engine::render::CancelToken::new();
        cancellation.on_cancel({
            let cancel = cancel.clone();
            move || cancel.cancel()
        });
        let eval = fidget_engine::raster::voxel::EvalConfig {
            cancel,
            tile_sizes: software_tiles(),
            ..Default::default()
        };
        let config = VoxelRenderConfig {
            world_to_model: view.world_to_model,
            ..VoxelRenderConfig::from_size(view.size)
        };
        let mut image = vec![
            (GeometryPixel::default(), [255; 3]);
            view.size.width() as usize * view.size.height() as usize
        ];
        for (shape, color) in &self.objects {
            cancellation.check().ok()?;
            let geometry = fidget_engine::raster::voxel::render(
                shape.clone().try_into().ok()?,
                &config,
                &eval,
            )?;
            for (output, pixel) in image.iter_mut().zip(geometry.iter()) {
                if pixel.depth > output.0.depth {
                    *output = (*pixel, *color);
                }
            }
        }
        let shade = shading(&config);
        Some(
            image
                .iter()
                .flat_map(|(pixel, color)| shade(*pixel, *color))
                .collect(),
        )
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn request(width: f64, height: f64) -> Request {
        Request::new(
            VolumePreview {
                objects: vec![SceneObject {
                    tree: Tree::z(),
                    color: [200, 100, 50],
                }],
                size: Size::new(width, height),
                min: Vector3::repeat(-1.0),
                max: Vector3::repeat(1.0),
            },
            None,
            1.0,
        )
        .unwrap()
    }

    #[test]
    fn refinement_reaches_native_pixels_and_keeps_the_same_model_space_view() {
        for (width, height) in [(333.0, 751.0), (1024.0, 1.0), (1.0, 1.0)] {
            let request = request(width, height);
            let sizes = request.resolutions(128);
            assert_eq!(sizes.last(), Some(&request.pixels));
            assert!(sizes[0].width().max(sizes[0].height()) <= 128);
            for pair in sizes.windows(2) {
                assert!(pair[1].width() >= pair[0].width());
                assert!(pair[1].height() >= pair[0].height());
                assert_ne!(pair[0], pair[1]);
            }
            let full = request.view_at(request.pixels);
            for coarse in request.refinements(128, 4).unwrap() {
                for [u, v, z] in [[0.0, 0.0, 0.0], [0.5, 0.5, 0.5], [1.0, 1.0, 1.0]] {
                    let model = |view: &VolumeView| {
                        let screen = nalgebra::Point3::new(
                            u * view.size.width() as f32,
                            v * view.size.height() as f32 - 1.0,
                            z * view.size.depth() as f32,
                        );
                        (view.world_to_model * view.size.screen_to_world()).transform_point(&screen)
                    };
                    assert!((model(&full) - model(&coarse)).norm() < 0.001);
                }
            }
            let views = request.refinements(128, 4).unwrap();
            assert_eq!(views.last().unwrap().size.depth(), full.size.depth() * 4);
            assert!(request.refinements(128, 0).is_none());
            assert!(request.refinements(128, u32::MAX).is_none());
        }
    }

    #[test]
    fn doubling_first_resolution_skips_only_the_coarsest_level() {
        for (width, height) in [(333.0, 751.0), (257.0, 129.0), (128.0, 64.0), (1.0, 1.0)] {
            let request = request(width, height);
            for first_max_edge in [128, 256] {
                let sizes = request.resolutions(first_max_edge);
                let skip = usize::from(sizes.len() > 1);
                assert_eq!(request.resolutions(first_max_edge * 2), sizes[skip..]);
            }
        }
    }

    #[test]
    fn progressive_images_match_final_quality_and_stop_on_cancellation() {
        let request = request(41.0, 27.0);
        let cancel = incremental::Cancellation::default();
        let mut stages = Vec::new();
        let final_image = request
            .render_software_progressive(12, 1, &cancel, &mut |image| {
                stages.push((image.width, image.height));
                Ok(())
            })
            .unwrap()
            .unwrap();
        assert_eq!(stages, [(11, 7), (21, 14)]);
        assert_eq!((final_image.width, final_image.height), (41, 27));
        assert_eq!(
            final_image.data.data(),
            cpu_volume(
                &request.preview.objects,
                &request.view_at(request.pixels),
                &cancel
            )
            .unwrap()
        );
        let mut published = 0;
        let cancelled = request.render_software_progressive(12, 4, &cancel, &mut |_| {
            published += 1;
            cancel.cancel();
            Ok(())
        });
        assert!(matches!(cancelled, Err(incremental::Error::Cancelled)));
        assert_eq!(published, 1);
    }

    #[test]
    fn native_image_is_published_before_depth_refinement_and_can_cancel_it() {
        let request = request(41.0, 27.0);
        let cancel = incremental::Cancellation::default();
        let mut stages = Vec::new();
        let final_image = request
            .render_software_progressive(12, 4, &cancel, &mut |image| {
                stages.push((image.width, image.height));
                Ok(())
            })
            .unwrap()
            .unwrap();
        assert_eq!(stages, [(11, 7), (21, 14), (41, 27)]);
        let refined = refine_depth(request.view_at(request.pixels), 4).unwrap();
        assert_eq!(
            final_image.data.data(),
            cpu_volume(&request.preview.objects, &refined, &cancel).unwrap()
        );

        let cancelled = request.render_software_progressive(128, 4, &cancel, &mut |image| {
            assert_eq!((image.width, image.height), (41, 27));
            cancel.cancel();
            Ok(())
        });
        assert!(matches!(cancelled, Err(incremental::Error::Cancelled)));
    }

    #[test]
    fn software_lighting_is_independent_of_sampling_density() {
        let mut request = request(333.0, 751.0);
        request.preview.objects[0].tree = Tree::x() * 0.3 + Tree::y() * 0.2 + Tree::z();
        let shape = VmShape::from(request.preview.objects[0].tree.clone());
        for camera in [
            Camera {
                yaw: 0.0,
                pitch: 0.0,
                zoom: 1.0,
            },
            Camera {
                yaw: 30.0,
                pitch: 40.0,
                zoom: 2.0,
            },
        ] {
            request.camera = camera;
            let rotation = Rotation3::from_axis_angle(&Vector3::z_axis(), camera.yaw.to_radians())
                * Rotation3::from_axis_angle(&Vector3::x_axis(), camera.pitch.to_radians());
            let normal = rotation.inverse() * Vector3::new(0.3, 0.2, 1.0).normalize();
            let light = Vector3::new(0.35, -0.45, 1.0).normalize();
            let expected = (200.0 * (0.22 + 0.78 * normal.dot(&light).max(0.0))) as u8;
            for view in request.refinements(128, 4).unwrap() {
                let config = VoxelRenderConfig {
                    image_size: view.size,
                    world_to_model: view.world_to_model,
                };
                let image = config.run(shape.clone().try_into().unwrap());
                let index =
                    (view.size.height() / 2 * view.size.width() + view.size.width() / 2) as usize;
                let pixel = image.as_slice()[index];
                assert!(pixel.depth > 0 && pixel.depth < view.size.depth());
                let shade = shading(&config);
                let rgba = shade(pixel, [200; 3]);
                assert_eq!(rgba[3], 255);
                assert!(rgba[0].abs_diff(expected) <= 1, "{rgba:?} vs {expected}");
                assert_eq!(shade(GeometryPixel::default(), [200; 3]), [0; 4]);
            }
        }
    }

    #[test]
    fn software_backend_preserves_the_vm_shaded_result() {
        let mut request = request(96.0, 72.0);
        request.preview.objects[0].tree =
            (Tree::x().square() + Tree::y().square() + Tree::z().square() - 0.25).max(Tree::z());
        let view = request.view_at(request.pixels);
        let config = VoxelRenderConfig {
            image_size: view.size,
            world_to_model: view.world_to_model,
        };
        let geometry = config.run(
            VmShape::from(request.preview.objects[0].tree.clone())
                .try_into()
                .unwrap(),
        );
        let shade = shading(&config);
        let expected: Vec<_> = geometry
            .iter()
            .flat_map(|pixel| shade(*pixel, request.preview.objects[0].color))
            .collect();
        assert_eq!(
            cpu_volume(&request.preview.objects, &view, &Default::default()).unwrap(),
            expected
        );
    }

    #[test]
    fn software_request_renders_on_a_worker_and_honors_cancellation() {
        let request = Request::new(
            VolumePreview {
                objects: vec![SceneObject {
                    tree: Tree::z(),
                    color: [200, 100, 50],
                }],
                size: Size::new(24.0, 16.0),
                min: Vector3::repeat(-1.0),
                max: Vector3::repeat(1.0),
            },
            None,
            2.0,
        )
        .unwrap();
        std::thread::spawn(move || {
            let cancel = incremental::Cancellation::default();
            let render = |request: &Request| {
                request.render_software_progressive(u32::MAX, 1, &cancel, &mut |_| {
                    panic!("no intermediate resolution requested")
                })
            };
            let image = render(&request).unwrap().unwrap();
            assert_eq!((image.width, image.height), (48, 32));
            assert!(
                image
                    .data
                    .data()
                    .chunks_exact(4)
                    .any(|p| { p[3] == 255 && p[0] > p[1] && p[1] > p[2] })
            );

            let mut empty = request.clone();
            empty.preview.objects.clear();
            let image = render(&empty).unwrap().unwrap();
            assert!(image.data.data().iter().all(|&b| b == 0));

            cancel.cancel();
            assert!(matches!(
                render(&request),
                Err(incremental::Error::Cancelled)
            ));
        })
        .join()
        .unwrap();
    }
}

impl Request {
    pub fn new(preview: VolumePreview, state: Option<&Value>, scale: f64) -> Option<Self> {
        Some(Self {
            pixels: raster_size(preview.size, scale)?,
            preview,
            camera: camera(state),
        })
    }

    pub fn size(&self) -> Size {
        self.preview.size
    }

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
        let mut view = volume_view(&self.preview, self.camera, pixels);
        // Pixel rounding must not change the camera's aspect ratio between passes.
        if pixels != self.pixels {
            let ratio = pixels.height() as f32 / self.pixels.height() as f32;
            view.world_to_model *= Scale3::new(
                self.pixels.width() as f32 / pixels.width() as f32 * ratio,
                1.0,
                1.0,
            )
            .to_homogeneous();
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

    /// Publish up to native image resolution, then optionally refine depth in one final pass.
    /// Completed rasters are independent; only scene compilation is shared within the job.
    /// Uses software evaluation explicitly: the GPU VM cannot execute spilling tapes.
    pub fn render_software_progressive(
        &self,
        first_max_edge: u32,
        final_depth_multiplier: u32,
        cancel: &incremental::Cancellation,
        publish: &mut dyn FnMut(ImageData) -> Result<(), incremental::Error>,
    ) -> Result<Option<ImageData>, incremental::Error> {
        cancel.check()?;
        assert!(first_max_edge > 0);
        let Some(views) = self.refinements(first_max_edge, final_depth_multiplier) else {
            return Ok(None);
        };
        let scene = SoftwareScene::new(&self.preview.objects, cancel);
        cancel.check()?;
        let Some(scene) = scene else { return Ok(None) };
        let mut views = views.into_iter().peekable();
        while let Some(view) = views.next() {
            cancel.check()?;
            let rgba = scene.render(&view, cancel);
            cancel.check()?;
            let Some(rgba) = rgba else { return Ok(None) };
            let image = ImageData {
                data: rgba.into(),
                format: ImageFormat::Rgba8,
                alpha_type: ImageAlphaType::Alpha,
                width: view.size.width(),
                height: view.size.height(),
            };
            if views.peek().is_none() {
                return Ok(Some(image));
            }
            publish(image)?;
        }
        unreachable!("the native resolution is always present")
    }
}
