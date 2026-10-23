//! Opt-in, headless native/browser comparison over the same CAM document: the
//! example's toolpath program, read through the editor's libraries.
use gid::Value;
use web_time::Instant;

/// Runs uncached work, not an editor frame. `controls` adds Fidget's own voxel
/// render of the same view, serial and parallel, for comparison.
pub fn profile_cam(progress: f64, width: u32, height: u32, controls: bool) -> String {
    let start = Instant::now();
    let (doc, names) = crate::gid_text::parse(crate::command::Example::Toolpaths.source()).unwrap();
    let stack = crate::stack::load();
    let sources = crate::sources::Sources {
        doc: &doc,
        libraries: &stack.libraries,
    };
    let setup_ms = start.elapsed().as_secs_f64() * 1000.0;
    let mut timings = crate::libraries::toolpath::performance::profile(
        &sources,
        Value::from(names["program_tree"]),
        progress,
        width,
        height,
        controls,
    );
    timings["setup_ms"] = setup_ms.into();
    timings.to_string()
}
