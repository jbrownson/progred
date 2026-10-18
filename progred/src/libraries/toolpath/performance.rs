//! The CAM work a profile times, from the program tree to its render.
use super::*;
use crate::libraries::{fidget as implicit, tree};
use serde_json::json;
use web_time::Instant;

/// Times uncached work, not an editor frame, for the toolpath program
/// `program_tree` names, read through `sources`. `controls` adds Fidget's own
/// voxel render of the same view, serial and parallel, for comparison.
pub(crate) fn profile(
    sources: &crate::sources::Sources<'_>,
    program_tree: Value,
    progress: f64,
    width: u32,
    height: u32,
    controls: bool,
) -> serde_json::Value {
    let start = Instant::now();
    let program = tree::build(&program_tree.into(), sources, 3_000_000).unwrap();
    let program = program.items;
    let tree_ms = start.elapsed().as_secs_f64() * 1000.0;
    let computations = crate::computations::Computations::from_sources(*sources);
    let recording = computation::recording(
        &computations,
        computations.runtime.input(program),
        computations.runtime.input(3_000_000),
    );
    let start = Instant::now();
    let recorded = computations.runtime.read(&recording).unwrap();
    let recording_ms = start.elapsed().as_secs_f64() * 1000.0;
    let path = recorded.path().unwrap();
    let settings = cube_playback(progress);
    let start = Instant::now();
    let stock = settings.remaining_stock(path).unwrap().unwrap();
    let stock_ms = start.elapsed().as_secs_f64() * 1000.0;
    let start = Instant::now();
    let paths =
        mesh::computation::paths(&recorded, 0.005, [240, 174, 80], Some(&settings)).unwrap();
    let paths_ms = start.elapsed().as_secs_f64() * 1000.0;
    let render =
        implicit::raster::performance::profile(stock, paths.clone(), width, height, controls);
    json!({
        "progress":progress, "width":width, "height":height,
        "segments":path.segments().count(),
        "tree_ms":tree_ms, "recording_ms":recording_ms,
        "stock_ms":stock_ms, "path_mesh_ms":paths_ms,
        "path_triangles":paths.indices.len()/3, "render":render,
    })
}
