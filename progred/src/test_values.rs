use crate::libraries::{name, text};
use gid::{CellId, Step, Value, new_cell_id};
use std::collections::BTreeMap;
use std::sync::{Mutex, OnceLock};

/// The record label a test calls `name`: the sample document's label of
/// that name, or one minted on first use and kept for the rest of the run.
pub fn label(name: &str) -> CellId {
    static LABELS: OnceLock<Mutex<BTreeMap<String, CellId>>> = OnceLock::new();
    *LABELS
        .get_or_init(|| Mutex::new(sample_labels()))
        .lock()
        .expect("test label table")
        .entry(name.to_string())
        .or_insert_with(new_cell_id)
}

/// The sample document's labels: its cells that hold nothing but a name.
fn sample_labels() -> BTreeMap<String, CellId> {
    crate::sample::sample_document()
        .cells
        .iter()
        .filter_map(|(cell, value)| {
            name::read(value)
                .filter(|spelling| *value == name::record(*spelling, []))
                .map(|spelling| (spelling.to_string(), *cell))
        })
        .collect()
}

pub fn text(text: impl Into<String>) -> Value {
    text::value(text)
}

pub fn key(s: &str) -> Step {
    Step::Key(crate::test_values::label(s))
}

/// A call of the binary numeric operation `function` on `left` and `right`.
pub fn binary_call(function: CellId, left: Value, right: Value) -> Value {
    use crate::libraries::number::vocabulary::{LEFT, RIGHT};
    ::grap::call(function.into(), [(LEFT, left), (RIGHT, right)])
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
            binary_call(function, value.clone(), value),
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
