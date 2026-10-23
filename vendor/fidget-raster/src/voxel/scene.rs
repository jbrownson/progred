//! Tile-major scene rendering using the same per-object voxel evaluator.
use super::*;

/// A scene's nearest surface and its index in the supplied object slice.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ScenePixel {
    /// Surface geometry, identical to rendering that object separately.
    pub geometry: GeometryPixel,
    /// Winning object; empty pixels have no object. Equal depths keep the first.
    pub object: Option<usize>,
}

/// One fully rendered scene tile, borrowed for the duration of a callback.
/// Callbacks may run concurrently and in any order. Edge tiles exclude padding.
pub struct SceneTile<'a> {
    /// Top-left image coordinates.
    pub origin: [usize; 2],
    /// Valid pixel dimensions, excluding padding.
    pub size: [usize; 2],
    /// Row stride in the padded pixel buffer.
    pub stride: usize,
    /// Tile pixels; each valid row starts at `y * stride`.
    pub pixels: &'a [ScenePixel],
}

/// Bound objects and their prepared root interval tapes, reusable across views.
/// Worker-local spatial simplifications are deliberately not retained here.
pub struct Scene<'a, F: Function> {
    objects: Vec<BoundShape<'a, F, f32>>,
    handles: Vec<RenderHandle<F>>,
}

impl<'a, F: Function + RenderHints> Scene<'a, F> {
    /// Prepare each root tape once, checking cancellation between objects.
    pub fn new(
        objects: Vec<BoundShape<'a, F, f32>>,
        cancel: &CancelToken,
    ) -> Option<Self> {
        let mut handles = Vec::with_capacity(objects.len());
        for object in &objects {
            if cancel.is_cancelled() {
                return None;
            }
            let mut handle = RenderHandle::new(object.shape().clone());
            handle.i_tape(&mut vec![]);
            handles.push(handle);
        }
        (!cancel.is_cancelled()).then_some(Self { objects, handles })
    }

    /// The bound objects in depth-tie precedence order.
    pub fn objects(&self) -> &[BoundShape<'a, F, f32>] {
        &self.objects
    }

    /// Render a view, optionally observing completed tiles before assembly.
    /// A tile callback runs before its pixels are counted as finished. It must
    /// not retain borrowed pixels; cancelling from it aborts the render.
    pub fn render(
        &self,
        config: &RenderConfig,
        eval: &EvalConfig,
        tile_ready: Option<&(dyn Fn(SceneTile<'_>) + Sync)>,
    ) -> Option<GenericImage<ScenePixel, RenderSize>> {
        let image = std::sync::Mutex::new(GenericImage::new(config.image_size));
        let collect = |tile: SceneTile<'_>| {
            {
                let mut image = image.lock().unwrap();
                for y in 0..tile.size[1] {
                    let start = (tile.origin[1] + y) * config.width() as usize
                        + tile.origin[0];
                    image.data[start..start + tile.size[0]].copy_from_slice(
                        &tile.pixels
                            [y * tile.stride..y * tile.stride + tile.size[0]],
                    );
                }
            }
            if let Some(ready) = tile_ready {
                ready(tile);
            }
        };
        self.render_tiles(config, eval, &collect)?;
        Some(image.into_inner().unwrap())
    }

    /// Consume completed tiles without constructing or retaining a whole image.
    pub fn render_tiles(
        &self,
        config: &RenderConfig,
        eval: &EvalConfig,
        tile_ready: &(dyn Fn(SceneTile<'_>) + Sync),
    ) -> Option<()> {
        render_prepared(&self.objects, &self.handles, config, eval, tile_ready)
    }
}

/// Render all objects within each image tile before reporting it complete.
///
/// Progress counts actual image pixels (excluding padding in edge tiles), not
/// objects or elapsed time. Expressions remain separate; depth ties keep the
/// earlier object, exactly as an ordered depth-composition of single renders.
/// Root interval tapes are compiled once before parallel work, and each worker
/// retains per-object render handles and shared evaluator scratch across tiles.
pub fn render_scene<F: Function + RenderHints>(
    objects: &[BoundShape<'_, F, f32>],
    config: &RenderConfig,
    eval: &EvalConfig,
) -> Option<GenericImage<ScenePixel, RenderSize>> {
    Scene::new(objects.to_vec(), &eval.cancel)?.render(config, eval, None)
}

fn render_prepared<F: Function + RenderHints>(
    objects: &[BoundShape<'_, F, f32>],
    handles: &[RenderHandle<F>],
    config: &RenderConfig,
    eval: &EvalConfig,
    tile_ready: &(dyn Fn(SceneTile<'_>) + Sync),
) -> Option<()> {
    let cancelled = || eval.cancel.is_cancelled();
    if cancelled() {
        return None;
    }
    let width = config.width() as usize;
    let height = config.height() as usize;
    let progress = crate::TileProgress::new(width * height, eval.progress);
    if cancelled() {
        return None;
    }
    let default_sizes;
    let sizes = if let Some(sizes) = &eval.tile_sizes {
        sizes
    } else {
        default_sizes = F::tile_sizes_3d();
        &default_sizes
    };
    let sizes = TileSizesRef::new(sizes, width.max(height));
    let size = sizes[0];
    crate::run_tiles(
        crate::tiles(width, height, size),
        eval,
        || {
            (
                objects.first().map(|object| {
                    Worker::<F>::new(config, sizes, object.vars())
                }),
                handles.to_vec(),
            )
        },
        |(worker, handles), tile| {
            let mut out = GenericImage::<ScenePixel, _>::new(RenderSize::from(
                size as u32,
            ));
            for (index, (object, handle)) in
                objects.iter().zip(handles).enumerate()
            {
                let worker = worker.as_mut().unwrap(); // A worker exists iff objects do.
                worker.vars = object.vars();
                worker.render_tile_in_place(handle, tile, &cancelled)?;
                for (dst, src) in out.data.iter_mut().zip(worker.out.iter()) {
                    let src = clamp_depth(*src, config.image_size.depth());
                    if src.depth > dst.geometry.depth {
                        *dst = ScenePixel {
                            geometry: src,
                            object: Some(index),
                        };
                    }
                }
            }
            if cancelled() {
                return None;
            }
            tile_ready(SceneTile {
                origin: [tile.corner.x, tile.corner.y],
                size: [
                    (width - tile.corner.x).min(size),
                    (height - tile.corner.y).min(size),
                ],
                stride: size,
                pixels: &out.data,
            });
            if cancelled() {
                return None;
            }
            progress.complete(
                (width - tile.corner.x).min(size)
                    * (height - tile.corner.y).min(size),
            );
            Some(())
        },
    )?;
    (!cancelled()).then_some(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fidget_core::{
        context::Tree,
        vm::{VmFunction, VmShape},
    };
    use std::sync::Mutex;

    fn objects() -> Vec<BoundShape<'static, VmFunction, f32>> {
        let sphere = (Tree::x() - 0.2).square()
            + Tree::y().square()
            + Tree::z().square()
            - 0.5;
        [sphere.clone(), sphere, Tree::y(), Tree::from(1.0)]
            .into_iter()
            .map(|tree| VmShape::from(tree).try_into().unwrap())
            .collect()
    }

    #[test]
    fn scene_matches_ordered_separate_renders_including_ties_and_clipping() {
        let objects = objects();
        let cfg = RenderConfig::from_size(RenderSize::new(41, 27, 33));
        for threads in [None, Some(&ThreadPool::Global)] {
            let eval = EvalConfig {
                threads,
                tile_sizes: Some(TileSizes::new(&[16, 8]).unwrap()),
                ..Default::default()
            };
            let mut expected =
                GenericImage::<ScenePixel, _>::new(cfg.image_size);
            for (index, object) in objects.iter().enumerate() {
                let image = render(object.clone(), &cfg, &eval).unwrap();
                for (dst, src) in expected.data.iter_mut().zip(image.iter()) {
                    if src.depth > dst.geometry.depth {
                        *dst = ScenePixel {
                            geometry: *src,
                            object: Some(index),
                        };
                    }
                }
            }
            let actual = render_scene(&objects, &cfg, &eval).unwrap();
            assert_eq!(actual.data, expected.data);
            assert!(actual.iter().any(|p| p.object == Some(0)));
            assert!(
                !actual.iter().any(|p| p.object == Some(1)),
                "ties keep the first object"
            );
            assert!(
                actual
                    .iter()
                    .any(|p| p.geometry.depth == cfg.image_size.depth())
            );
        }
    }

    #[test]
    fn scene_progress_counts_finished_image_pixels_not_objects_or_tile_padding()
    {
        let cfg = RenderConfig::from_size(RenderSize::new(41, 27, 33));
        for threads in [None, Some(&ThreadPool::Global)] {
            for count in [1, 4] {
                let reports = Mutex::new(Vec::new());
                let report =
                    |n, total| reports.lock().unwrap().push((n, total));
                let eval = EvalConfig {
                    threads,
                    tile_sizes: Some(TileSizes::new(&[16, 8]).unwrap()),
                    progress: Some(&report),
                    ..Default::default()
                };
                render_scene(&objects()[..count], &cfg, &eval).unwrap();
                let reports = reports.into_inner().unwrap();
                assert_eq!(reports.first(), Some(&(0, 41 * 27)));
                assert_eq!(reports.last(), Some(&(41 * 27, 41 * 27)));
                assert_eq!(
                    reports.len(),
                    7,
                    "one completion per whole-scene tile"
                );
                let mut increments = reports
                    .windows(2)
                    .map(|w| {
                        assert_eq!(w[1].1, 41 * 27);
                        assert!(w[1].0 > w[0].0);
                        w[1].0 - w[0].0
                    })
                    .collect::<Vec<_>>();
                increments.sort();
                assert_eq!(increments, [99, 144, 176, 176, 256, 256]);
            }
        }
    }

    #[test]
    fn cancellation_does_not_finish_the_scene() {
        let cfg = RenderConfig::from_size(RenderSize::new(41, 27, 33));
        for stop_at in [0, 256] {
            let cancel = CancelToken::new();
            let reports = Mutex::new(Vec::new());
            let report = |n, total| {
                reports.lock().unwrap().push((n, total));
                if n == stop_at {
                    cancel.cancel();
                }
            };
            let eval = EvalConfig {
                threads: None,
                cancel: cancel.clone(),
                tile_sizes: Some(TileSizes::new(&[16, 8]).unwrap()),
                progress: Some(&report),
            };
            assert!(render_scene(&objects(), &cfg, &eval).is_none());
            assert_eq!(
                reports.lock().unwrap().last(),
                Some(&(stop_at, 41 * 27))
            );
        }
    }

    #[test]
    fn empty_scene_finishes_as_transparent() {
        let reports = Mutex::new(Vec::new());
        let report = |n, total| reports.lock().unwrap().push((n, total));
        let cfg = RenderConfig::from_size(RenderSize::new(13, 7, 16));
        let image = render_scene::<VmFunction>(
            &[],
            &cfg,
            &EvalConfig {
                progress: Some(&report),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(image.iter().all(|p| *p == ScenePixel::default()));
        assert_eq!(*reports.lock().unwrap(), [(0, 91), (91, 91)]);
    }

    #[test]
    fn prepared_scene_tiles_reconstruct_each_view_and_cancel_cleanly() {
        let scene = Scene::new(objects(), &CancelToken::new()).unwrap();
        for dimensions in [(41, 27, 33), (23, 51, 110)] {
            let cfg = RenderConfig::from_size(RenderSize::new(
                dimensions.0,
                dimensions.1,
                dimensions.2,
            ));
            let assembled =
                Mutex::new(GenericImage::<ScenePixel, _>::new(cfg.image_size));
            let seen =
                Mutex::new(vec![0; (cfg.width() * cfg.height()) as usize]);
            let callback = |tile: SceneTile<'_>| {
                let mut image = assembled.lock().unwrap();
                let mut seen = seen.lock().unwrap();
                for y in 0..tile.size[1] {
                    for x in 0..tile.size[0] {
                        let dst = (tile.origin[1] + y) * cfg.width() as usize
                            + tile.origin[0]
                            + x;
                        image.data[dst] = tile.pixels[y * tile.stride + x];
                        seen[dst] += 1;
                    }
                }
            };
            let eval = EvalConfig {
                tile_sizes: Some(TileSizes::new(&[16, 8]).unwrap()),
                ..Default::default()
            };
            let output = scene.render(&cfg, &eval, Some(&callback)).unwrap();
            assert_eq!(output.data, assembled.into_inner().unwrap().data);
            assert!(seen.into_inner().unwrap().iter().all(|n| *n == 1));
            assert_eq!(
                output.data,
                render_scene(scene.objects(), &cfg, &eval).unwrap().data
            );
            let eval = EvalConfig {
                threads: None,
                ..eval
            };
            let callback = |_: SceneTile<'_>| eval.cancel.cancel();
            assert!(scene.render(&cfg, &eval, Some(&callback)).is_none());
        }
    }

    #[test]
    fn scene_worker_reuses_and_clears_the_object_tile_buffer() {
        let cfg = RenderConfig::from_size(RenderSize::new(17, 19, 24));
        let sizes = TileSizes::new(&[16, 8]).unwrap();
        let vars = ShapeVars::new();
        let mut worker = Worker::<VmFunction>::new(
            &cfg,
            TileSizesRef::new(&sizes, 19),
            &vars,
        );
        let tile = Tile::new(nalgebra::Point2::new(0, 0));
        let mut filled = RenderHandle::new(VmShape::from(Tree::from(-1.0)));
        let mut empty = RenderHandle::new(VmShape::from(Tree::from(1.0)));
        worker
            .render_tile_in_place(&mut filled, tile, &|| false)
            .unwrap();
        let address = worker.out.data.as_ptr();
        assert!(worker.out.iter().all(|p| p.depth > 0));
        worker
            .render_tile_in_place(&mut empty, tile, &|| false)
            .unwrap();
        assert_eq!(worker.out.data.as_ptr(), address);
        assert!(worker.out.iter().all(|p| *p == GeometryPixel::default()));
    }

    #[test]
    fn each_object_keeps_its_bound_variables() {
        use fidget_core::{Context, var::Var};
        let mut ctx = Context::new();
        let v = Var::new();
        let z = ctx.z();
        let offset = ctx.var(v);
        let node = ctx.sub(z, offset).unwrap();
        let shape = VmShape::new(&ctx, node).unwrap();
        let mut a = ShapeVars::new();
        let mut b = ShapeVars::new();
        a.insert(v.index().unwrap(), -0.3);
        b.insert(v.index().unwrap(), 0.3);
        let objects =
            [shape.clone().bind(&a).unwrap(), shape.bind(&b).unwrap()];
        let cfg = RenderConfig::from_size(32.into());
        let eval = EvalConfig::default();
        let expected = render(objects[1].clone(), &cfg, &eval).unwrap();
        let actual = render_scene(&objects, &cfg, &eval).unwrap();
        for (a, b) in actual.iter().zip(expected.iter()) {
            assert_eq!(a.geometry, *b);
            assert_eq!(a.object, Some(1));
        }
    }
}
