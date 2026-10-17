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

/// A call of each numeric representation's operations, beside the
/// representation and the function called: binary ones on f64, f32 and u64,
/// and f64's unary sine.
pub fn numeric_calls() -> Vec<(CellId, CellId, Value)> {
    use crate::libraries::{f32, f64, number, u64};
    [
        (f64::vocabulary::F64, f64::vocabulary::SUM, f64::value(1.0)),
        (f32::vocabulary::F32, f32::vocabulary::SUM, f32::value(2.0)),
        (u64::vocabulary::U64, u64::vocabulary::LESS, u64::value(3)),
    ]
    .into_iter()
    .map(|(representation, function, value)| {
        (
            representation,
            function,
            ::grap::call(
                function.into(),
                [
                    (number::vocabulary::LEFT, value.clone()),
                    (number::vocabulary::RIGHT, value),
                ],
            ),
        )
    })
    .chain([(
        f64::vocabulary::F64,
        f64::vocabulary::SIN,
        ::grap::call(
            f64::vocabulary::SIN.into(),
            [(number::vocabulary::OPERAND, f64::value(0.5))],
        ),
    )])
    .collect()
}
