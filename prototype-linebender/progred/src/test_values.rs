use gid::{CellId, Value, new_cell_id};
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

fn sample_relation(name: &str) -> Option<CellId> {
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

pub fn relation(name: &str) -> CellId {
    static RELATIONS: OnceLock<Mutex<BTreeMap<String, CellId>>> = OnceLock::new();
    sample_relation(name).unwrap_or_else(|| {
        *RELATIONS
            .get_or_init(|| Mutex::new(BTreeMap::new()))
            .lock()
            .expect("test relation table")
            .entry(name.to_string())
            .or_insert_with(new_cell_id)
    })
}

pub fn label(name: &str) -> CellId {
    relation(name)
}

pub fn text(text: impl Into<String>) -> Value {
    progred_text::value(text)
}
