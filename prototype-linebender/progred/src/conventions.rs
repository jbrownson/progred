//! Bootstrap libraries and the closed-record projections they
//! cover. Each function checks its own preconditions.

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

pub fn whole_text(value: &Value) -> Option<&str> {
    let text = progred_text::read(value)?;
    value
        .as_record()?
        .keys()
        .all(|label| *label == progred_text::vocabulary::UTF8)
        .then_some(text)
}

fn whole_f64(value: &Value) -> Option<f64> {
    let number = grap_f64::read(value)?;
    value
        .as_record()?
        .keys()
        .all(|label| *label == grap_f64::vocabulary::F64)
        .then_some(number)
}

fn text_value(text: &str) -> Option<Value> {
    Some(progred_text::value(text))
}

fn f64_value(text: &str) -> Option<Value> {
    text.parse::<f64>().ok().map(grap_f64::value)
}

pub fn text(value: &Value) -> Option<LineEdit> {
    whole_text(value).map(|text| LineEdit {
        text: text.to_string(),
        parser: text_value,
        prefix: "\"".into(),
        suffix: "\"".into(),
    })
}

pub fn f64(value: &Value) -> Option<LineEdit> {
    whole_f64(value).map(|number| LineEdit {
        text: number.to_string(),
        parser: f64_value,
        prefix: String::new(),
        suffix: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closed_f64_is_an_editor_and_extra_fields_stay_structural() {
        let number = grap_f64::value(2.5);
        let edit = crate::stack::values(&number).unwrap();
        assert_eq!(edit.text, "2.5");
        assert!(edit.prefix.is_empty());

        let enriched_number = Value::record(number.as_record().unwrap().clone().update(
            crate::test_values::label("created-at"),
            crate::test_values::text("now"),
        ));
        assert_eq!(grap_f64::read(&enriched_number), Some(2.5));
        assert!(crate::stack::values(&enriched_number).is_none());
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
