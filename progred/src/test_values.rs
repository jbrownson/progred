use crate::libraries::name;
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

pub fn key(s: &str) -> Step {
    Step::Key(crate::test_values::label(s))
}

/// A call of `function` with `fields` as its arguments.
pub fn call(function: CellId, fields: impl IntoIterator<Item = (CellId, Value)>) -> Value {
    ::grap::call(function.into(), fields)
}

/// A call of the binary numeric operation `function` on `left` and `right`.
pub fn binary_call(function: CellId, left: Value, right: Value) -> Value {
    use crate::libraries::grap::numbers::vocabulary::{LEFT, RIGHT};
    call(function, [(LEFT, left), (RIGHT, right)])
}

/// A function's arguments it couldn't use: "invalid arguments" for a call
/// that's the evaluation's input, carrying why under each label.
pub fn invalid_arguments(why: impl IntoIterator<Item = (CellId, Value)>) -> Value {
    Value::record(
        [
            (
                ::grap::absent::ABSENT,
                ::grap::absent::INVALID_ARGUMENTS.into(),
            ),
            (::grap::absent::CALL, input_call()),
        ]
        .into_iter()
        .chain(why),
    )
}

/// Where a call that's the evaluation's input is.
fn input_call() -> Value {
    ::grap::source::value(&::grap::SourceOrigin::Input(Vec::new()))
}

/// Why `value` isn't a type: its `reason`, carrying it.
pub fn not(reason: CellId, value: Value) -> Value {
    ::grap::absent::with_detail(reason, ::grap::absent::VALUE, value)
}

/// A missing argument's absent, naming its label and the call it's missing
/// from, the evaluation's input.
pub fn missing(label: CellId) -> Value {
    ::grap::absent::missing_argument(label, Some(&::grap::SourceOrigin::Input(Vec::new())))
}

/// A call of each numeric representation's operations, beside the
/// representation and the function called: binary ones on f64, f32 and u64,
/// and f64's unary sine.
pub fn numeric_calls() -> Vec<(CellId, CellId, Value)> {
    use crate::libraries::{f32, f64, u64};
    [
        (
            f64::vocabulary::F64,
            crate::libraries::grap::numbers::f64::SUM,
            f64::value(1.0),
        ),
        (
            f32::vocabulary::F32,
            crate::libraries::grap::numbers::f32::SUM,
            f32::value(2.0),
        ),
        (
            u64::vocabulary::U64,
            crate::libraries::grap::numbers::u64::LESS,
            u64::value(3),
        ),
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
        crate::libraries::grap::numbers::f64::SIN,
        ::grap::call(
            crate::libraries::grap::numbers::f64::SIN.into(),
            [(
                crate::libraries::grap::numbers::vocabulary::OPERAND,
                f64::value(0.5),
            )],
        ),
    )])
    .collect()
}
