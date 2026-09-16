//! Lookup one child of a displayed value, including missing children.

#[cfg(test)]
use crate::libraries::f64;
use gid::{CellId, Resolution, Step, Value};

pub(super) fn child<'a>(
    parent: &'a Value,
    step: &Step,
    resolve: impl Fn(CellId, &Resolution) -> Option<&'a Value>,
) -> Option<&'a Value> {
    match step {
        Step::Key(label) => parent.as_record()?.get(label),
        Step::Element(position) => parent.as_list()?.get(position),
        Step::Follow(resolution) => resolve(parent.as_cell()?, resolution),
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
            child(&value, &Step::Key(field), |_, _| None),
            Some(&expected),
        );
    }

    #[test]
    fn child_lookup_preserves_missing_fields_and_resolves_follows() {
        let missing = crate::test_values::label("missing");
        let record = Value::record([]);
        assert_eq!(child(&record, &Step::Key(missing), |_, _| None), None,);

        let cell = crate::test_values::label("cell");
        let link = Value::from(cell);
        let resolved = crate::test_values::text("resolved");
        assert_eq!(
            child(
                &link,
                &Step::Follow(Resolution::Document),
                |requested, resolution| {
                    (requested == cell && *resolution == Resolution::Document).then_some(&resolved)
                }
            ),
            Some(&resolved),
        );
    }
}
