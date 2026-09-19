//! Opt-in, headless native/browser comparison over the same CAM document.
use super::*;
use crate::libraries::{fidget as implicit, tree};
use serde_json::json;
use web_time::Instant;

/// Runs uncached work, not an editor frame. `controls` adds backend comparisons.
pub fn profile_cam(progress: f64, width: u32, height: u32, controls: bool) -> String {
    let start = Instant::now();
    let (doc, names) = crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let stack = crate::stack::load();
    let sources = crate::sources::Sources {
        doc: &doc,
        libraries: &stack.libraries,
    };
    let setup_ms = start.elapsed().as_secs_f64() * 1000.0;
    let start = Instant::now();
    let program = tree::build(&names["program_tree"].into(), &sources, 3_000_000).unwrap();
    let tree_ms = start.elapsed().as_secs_f64() * 1000.0;
    let computations = crate::computations::Computations::from_sources(sources);
    let recording = computation::recording(
        &computations,
        computations.runtime.input(program.items),
        computations.runtime.input(3_000_000),
    );
    let start = Instant::now();
    let recorded = computations.runtime.read(&recording).unwrap();
    let recording_ms = start.elapsed().as_secs_f64() * 1000.0;
    let path = recorded.path().unwrap();
    let settings = playback::Settings::read(&Value::record([
        (PROGRESS, f64::value(progress)),
        (PROFILE_TOLERANCE, f64::value(0.001)),
        (STOCK_MIN, point_value([-0.5; 3])),
        (STOCK_MAX, point_value([0.5; 3])),
        (
            STOCK,
            Value::record([(
                implicit::vocabulary::COLOR,
                Value::record([(
                    crate::libraries::color::vocabulary::RGB,
                    Value::from(vec![184_u8, 155, 109]),
                )]),
            )]),
        ),
    ]))
    .unwrap();
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
        "segments":path.segments().count(), "setup_ms":setup_ms,
        "tree_ms":tree_ms, "recording_ms":recording_ms,
        "stock_ms":stock_ms, "path_mesh_ms":paths_ms,
        "path_triangles":paths.indices.len()/3, "render":render,
    })
    .to_string()
}
