//! Lookup one child of a displayed value, including missing children.

#[cfg(test)]
use crate::libraries::f64;
use gid::{CellId, Resolution, Step, Value};

pub(super) fn of<'a>(
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
mod tests;
