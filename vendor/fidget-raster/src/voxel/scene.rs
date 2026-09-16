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
    if objects.is_empty() {
        let image = GenericImage::new(config.image_size);
        progress.complete(width * height);
        return (!cancelled()).then_some(image);
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
    let mut handles = Vec::with_capacity(objects.len());
    for object in objects {
        if cancelled() {
            return None;
        }
        let mut handle = RenderHandle::new(object.shape().clone());
        handle.i_tape(&mut vec![]);
        handles.push(handle);
    }
    let tiles = crate::run_tiles(
        crate::tiles(width, height, size),
        eval,
        || {
            (
                Worker::<F>::new(config, sizes, objects[0].vars()),
                handles.clone(),
            )
        },
        |(worker, handles), tile| {
            let mut out = GenericImage::<ScenePixel, _>::new(RenderSize::from(
                size as u32,
            ));
            for (index, (object, handle)) in
                objects.iter().zip(handles).enumerate()
            {
                worker.vars = object.vars();
                let geometry = worker.render_tile(handle, tile, &cancelled)?;
                for (dst, src) in out.data.iter_mut().zip(geometry.iter()) {
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
            progress.complete(
                (width - tile.corner.x).min(size)
                    * (height - tile.corner.y).min(size),
            );
            Some(out)
        },
    )?;
    if cancelled() {
        return None;
    }
    let mut image = GenericImage::new(config.image_size);
    for (tile, out) in tiles {
        let w = (width - tile.corner.x).min(size);
        let h = (height - tile.corner.y).min(size);
        for y in 0..h {
            let start = (tile.corner.y + y) * width + tile.corner.x;
            image.data[start..start + w]
                .copy_from_slice(&out.data[y * size..y * size + w]);
        }
    }
    (!cancelled()).then_some(image)
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
