use crate::libraries::text;
use gid::{CellId, Step, Value, new_cell_id};
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

fn sample_label(name: &str) -> Option<CellId> {
    use crate::sample::sample_vocabulary as sample;
    match name {
        "at" => Some(sample::AT),
        "row" => Some(sample::ROW),
        "col" => Some(sample::COL),
        "of" => Some(sample::OF),
        "color" => Some(sample::COLOR),
        "swatch" => Some(sample::SWATCH),
        "points" => Some(sample::POINTS),
        "tags" => Some(sample::TAGS),
        "material" => Some(sample::MATERIAL),
        "style" => Some(sample::STYLE),
        "pitch" => Some(sample::PITCH),
        "double pitch" => Some(sample::DOUBLE_PITCH),
        "profile" => Some(sample::PROFILE),
        "shape" => Some(sample::SHAPE),
        "favorite" => Some(sample::FAVORITE),
        _ => None,
    }
}

/// The record label a test calls `name`: the sample vocabulary's, or one
/// minted on first use and kept for the rest of the run.
pub fn label(name: &str) -> CellId {
    static LABELS: OnceLock<Mutex<BTreeMap<String, CellId>>> = OnceLock::new();
    sample_label(name).unwrap_or_else(|| {
        *LABELS
            .get_or_init(|| Mutex::new(BTreeMap::new()))
            .lock()
            .expect("test label table")
            .entry(name.to_string())
            .or_insert_with(new_cell_id)
    })
}

pub fn text(text: impl Into<String>) -> Value {
    text::value(text)
}

pub fn key(s: &str) -> Step {
    Step::Key(crate::test_values::label(s))
}
