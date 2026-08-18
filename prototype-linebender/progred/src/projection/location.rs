//! Unresolved projection locations and GID-step lookup.

use gid::{CellId, Step, Value};
#[cfg(test)]
use progred_libraries::f64;

/// A place a projection can begin. Children retain the parent and
/// step rather than arriving pre-resolved, so absence is visible to
/// the projection just like every other GID state.
pub enum Location<'a> {
    Root(Option<&'a Value>),
    Child { parent: &'a Value, step: Step },
}

impl Location<'_> {
    pub fn value<'a>(&'a self, resolve: impl Fn(CellId) -> Option<&'a Value>) -> Option<&'a Value> {
        match self {
            Self::Root(value) => *value,
            Self::Child { parent, step } => match step {
                Step::Key(label) => parent.as_record()?.get(label),
                Step::Element(position) => parent.as_list()?.get(position),
                Step::Follow => resolve(parent.as_cell()?),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_child_location_looks_up_the_step_when_projected() {
        let child = crate::test_values::label("child");
        let value = Value::record([(child, f64::value(2.5))]);
        let expected = f64::value(2.5);
        assert_eq!(
            Location::Child {
                parent: &value,
                step: Step::Key(child),
            }
            .value(|_| None),
            Some(&expected),
        );
    }

    #[test]
    fn child_lookup_preserves_missing_fields_and_resolves_follows() {
        let missing = crate::test_values::label("missing");
        let record = Value::record([]);
        assert_eq!(
            Location::Child {
                parent: &record,
                step: Step::Key(missing),
            }
            .value(|_| None),
            None,
        );

        let cell = crate::test_values::label("cell");
        let link = Value::from(cell);
        let resolved = crate::test_values::text("resolved");
        assert_eq!(
            Location::Child {
                parent: &link,
                step: Step::Follow,
            }
            .value(|requested| (requested == cell).then_some(&resolved)),
            Some(&resolved),
        );
    }
}
