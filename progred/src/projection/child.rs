//! Lookup one child of a displayed value, including missing children.

use gid::{CellId, Step, Value};

pub(super) fn of<'a>(
    parent: &grap::RuntimeValue,
    step: &Step,
    resolve: impl Fn(CellId) -> Option<&'a Value>,
) -> Option<grap::RuntimeValue> {
    match step {
        Step::Key(label) => parent.field(*label),
        Step::Element(position) => parent.list_element(position),
        Step::Follow => resolve(parent.as_cell()?).map(grap::RuntimeValue::from),
    }
}

#[cfg(test)]
#[path = "tests/child.rs"]
mod tests;
