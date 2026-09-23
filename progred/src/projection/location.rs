//! Lookup one child of a displayed value, including missing children.

#[cfg(test)]
use crate::libraries::f64;
use gid::{CellId, Resolution, Step, Value};

pub(super) fn child<'a>(
    parent: &grap::RuntimeValue,
    step: &Step,
    resolve: impl Fn(CellId, &Resolution) -> Option<&'a Value>,
) -> Option<grap::RuntimeValue> {
    match step {
        Step::Key(label) => parent.field(*label),
        Step::Element(position) => parent.list_element(position),
        Step::Follow(resolution) => {
            resolve(parent.as_cell()?, resolution).map(grap::RuntimeValue::from)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_child_location_looks_up_the_step_when_projected() {
        let field = crate::test_values::label("child");
        let value = Value::record([(field, f64::value(2.5))]);
        let expected = f64::value(2.5);
        assert_eq!(
            child(&(&value).into(), &Step::Key(field), |_, _| None)
                .map(grap::RuntimeValue::into_value),
            Some(expected),
        );
    }

    #[test]
    fn child_lookup_preserves_missing_fields_and_resolves_follows() {
        let missing = crate::test_values::label("missing");
        let record = Value::record([]);
        assert_eq!(
            child(&record.into(), &Step::Key(missing), |_, _| None)
                .map(grap::RuntimeValue::into_value),
            None,
        );

        let cell = crate::test_values::label("cell");
        let link = Value::from(cell);
        let resolved = crate::test_values::text("resolved");
        assert_eq!(
            child(
                &link.into(),
                &Step::Follow(Resolution::Document),
                |requested, resolution| {
                    (requested == cell && *resolution == Resolution::Document).then_some(&resolved)
                }
            )
            .map(grap::RuntimeValue::into_value),
            Some(resolved.clone()),
        );
    }
}
