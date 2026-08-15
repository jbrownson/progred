//! Bootstrap libraries and the facet projections they cover. Each
//! function checks its own preconditions.

use crate::display::LineEdit;
use crate::sources::Sources;
use progred_graph::{CellId, Cells, Value};

pub mod vocabulary {
    use progred_graph::CellId;

    /// Field projection: its value is evaluated by Grap and the result
    /// is recursively projected in a derived, read-only context. This
    /// is not a Grap evaluator form.
    pub const GRAP: CellId = CellId::from_u128(0xac807d20d964e141d44c1b2eb98e5ca9);
}

pub fn library() -> Cells {
    let mut cells = progred_name::library()
        .merged(progred_isa::library())
        .merged(grap::library())
        .merged(grap_absent::library())
        .merged(grap_control::library())
        .merged(grap_f64::library());
    cells.set_value(vocabulary::GRAP, progred_name::record("grap", []));
    cells
}

pub fn foreign_functions() -> grap::ForeignFunctions {
    grap::ForeignFunctions::merge_all([
        grap::functions(),
        grap_control::functions(),
        grap_f64::functions(),
    ])
}

pub fn name<'a>(sources: &'a Sources, cell: CellId) -> Option<&'a str> {
    sources.value(cell).and_then(progred_name::read)
}

/// Raw shows the uninterpreted value and therefore uses the short id.
pub fn display_name<'a>(sources: &'a Sources, raw: bool, cell: CellId) -> Option<&'a str> {
    (!raw).then(|| name(sources, cell)).flatten()
}

pub fn grap(
    foreign: &grap::ForeignFunctions,
    field: CellId,
    expression: &Value,
    resolve: impl Fn(CellId) -> Option<Value>,
) -> Option<Value> {
    (field == vocabulary::GRAP).then(|| {
        grap::evaluate(expression, resolve, foreign, grap::DEFAULT_FUEL).result
    })
}

fn overlay(current: &Value, parsed: Value) -> Value {
    match (current.as_record(), parsed.as_record()) {
        (Some(current), Some(parsed)) => Value::record(
            parsed
                .iter()
                .fold(current.clone(), |fields, (key, value)| {
                    fields.update(*key, value.clone())
                }),
        ),
        _ => parsed,
    }
}

fn text_value(current: &Value, text: &str) -> Option<Value> {
    Some(overlay(current, progred_text::value(text)))
}

fn f64_value(current: &Value, text: &str) -> Option<Value> {
    text.parse::<f64>().ok().map(|n| overlay(current, grap_f64::value(n)))
}

pub fn text(value: &Value) -> Option<LineEdit> {
    progred_text::read(value).map(|text| LineEdit {
        text: text.to_string(),
        update: text_value,
        prefix: "\"".into(),
        suffix: "\"".into(),
    })
}

pub fn f64(value: &Value) -> Option<LineEdit> {
    grap_f64::read(value).map(|number| LineEdit {
        text: number.to_string(),
        update: f64_value,
        prefix: String::new(),
        suffix: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f64_line_recognizes_an_open_record() {
        let number = grap_f64::value(2.5);
        let edit = crate::stack::values(&number).unwrap();
        assert_eq!(edit.text, "2.5");
        assert!(edit.prefix.is_empty());

        let tagged = Value::record(number.as_record().unwrap().clone().update(
            crate::test_values::label("unit"),
            crate::test_values::text("mm"),
        ));
        assert_eq!(grap_f64::read(&tagged), Some(2.5));
        assert_eq!(
            crate::stack::values(&tagged).map(|edit| edit.text),
            Some("2.5".into())
        );
    }

    #[test]
    fn grap_evaluates_only_its_own_field() {
        let foreign = foreign_functions();
        let expression = grap::call(
            Value::from(grap_f64::vocabulary::ADD),
            [
                (grap_f64::vocabulary::LEFT, grap_f64::value(2.0)),
                (grap_f64::vocabulary::RIGHT, grap_f64::value(3.0)),
            ],
        );
        assert_eq!(
            grap(&foreign, vocabulary::GRAP, &expression, |_| None),
            Some(grap_f64::value(5.0))
        );
        assert_eq!(
            grap(&foreign, crate::test_values::label("data"), &expression, |_| None),
            None
        );
    }
}
