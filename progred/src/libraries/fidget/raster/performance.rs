//! Computation timings only: no window, editor, GPU upload or presentation.
use super::*;
use serde_json::{Value as Json, json};
use web_time::Instant;

pub(crate) fn profile(
    stock: SceneObject,
    paths: mesh::Mesh,
    width: u32,
    height: u32,
    controls: bool,
) -> Json {
    let preview = VolumePreview {
        objects: vec![stock],
        size: Size::new(width.into(), height.into()),
        min: Vector3::repeat(-0.85),
        max: Vector3::repeat(0.85),
    };
    let cancel = incremental::Cancellation::default();
    let mut geometry = mesh::Geometry::default();
    let start = Instant::now();
    mesh::Shape::from(&preview)
        .append_cancellable(&mut geometry, 6, &cancel)
        .unwrap()
        .unwrap();
    let mesh_ms = start.elapsed().as_secs_f64() * 1000.0;
    let request = Request::new(preview.clone(), None, 1.0).unwrap();
    let mut publications = 0;
    let start = Instant::now();
    let frame = request
        .render_software_tiles(
            Passes::Final,
            4,
            &cancel,
            &mut |_| {
                publications += 1;
                Ok(())
            },
            Some(&|_| {}),
        )
        .unwrap()
        .unwrap();
    let implicit_ms = start.elapsed().as_secs_f64() * 1000.0;
    let rgba = frame.image.data.data();
    let occupied = rgba.chunks_exact(4).filter(|p| p[3] != 0).count();
    // Matching coverage/depth checks guard against comparing different work.
    let depth_sum: f64 = frame.depth.iter().map(|&x| f64::from(x)).sum();
    let rgba_sum: u64 = rgba.iter().map(|&x| u64::from(x)).sum();
    let rgba_hash = fingerprint(rgba.iter().copied());
    let depth_hash = fingerprint(frame.depth.iter().flat_map(|x| x.to_le_bytes()));
    let mut combined = (*paths).clone();
    combined.append_colored(&geometry, |color| color).unwrap();
    let drawing = mesh::performance::draw(&preview, combined.into());
    let mut control_results = Vec::new();
    if controls {
        let view = refine_depth(request.view().view_at(request.pixels), 4).unwrap();
        control_results.push(control(&preview, &view, false));
        #[cfg(not(target_arch = "wasm32"))]
        control_results.push(control(&preview, &view, true));
    }
    json!({"mesh_ms":mesh_ms, "stock_vertices":geometry.vertices.len(),
        "stock_triangles":geometry.indices.len()/3, "implicit_ms":implicit_ms,
        "partial_publications":publications, "occupied_pixels":occupied,
        "depth_sum":depth_sum, "rgba_sum":rgba_sum, "rgba_hash":rgba_hash,
        "depth_hash":depth_hash, "controls":control_results, "drawing":drawing})
}

fn fingerprint(bytes: impl Iterator<Item = u8>) -> String {
    format!(
        "{:016x}",
        bytes.fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        })
    )
}

/// Fidget's own voxel render of the same view, serially or on the global
/// thread pool, without Progred's tiling and publication.
fn control(preview: &VolumePreview, view: &VolumeView, parallel: bool) -> Json {
    use fidget_engine::{
        raster::voxel::{EvalConfig, Scene},
        render::{CancelToken, ThreadPool},
    };
    let start = Instant::now();
    let shapes = preview
        .objects
        .iter()
        .map(|o| VmShape::from(o.tree.clone()).try_into().ok().unwrap())
        .collect();
    let scene = Scene::new(shapes, &CancelToken::new()).unwrap();
    let compile_ms = start.elapsed().as_secs_f64() * 1000.0;
    let config = VoxelRenderConfig {
        world_to_model: view.world_to_model,
        ..VoxelRenderConfig::from_size(view.size)
    };
    let eval = EvalConfig {
        threads: parallel.then_some(&ThreadPool::Global),
        tile_sizes: software_tiles(),
        ..Default::default()
    };
    let start = Instant::now();
    let pixels = scene.render(&config, &eval, None).unwrap();
    let render_ms = start.elapsed().as_secs_f64() * 1000.0;
    let occupied = pixels.iter().filter(|p| p.geometry.depth != 0).count();
    let depth_sum: u64 = pixels.iter().map(|p| u64::from(p.geometry.depth)).sum();
    json!({"threads":if parallel {ThreadPool::Global.thread_count()} else {1},
        "compile_ms":compile_ms, "render_ms":render_ms,
        "occupied_pixels":occupied, "raw_depth_sum":depth_sum})
}
