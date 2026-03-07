use crate::generated::semantics::{ISA, NUMBER_TYPE, STRING_TYPE};
use crate::graph::{Gid, Id};
use im::HashMap;
use std::sync::OnceLock;

#[derive(Clone, Copy, Default)]
pub struct BuiltinValuesGid;

fn string_edges() -> &'static HashMap<Id, Id> {
    static STRING_EDGES: OnceLock<HashMap<Id, Id>> = OnceLock::new();
    STRING_EDGES.get_or_init(|| HashMap::unit(ISA.clone(), STRING_TYPE.clone()))
}

fn number_edges() -> &'static HashMap<Id, Id> {
    static NUMBER_EDGES: OnceLock<HashMap<Id, Id>> = OnceLock::new();
    NUMBER_EDGES.get_or_init(|| HashMap::unit(ISA.clone(), NUMBER_TYPE.clone()))
}

impl Gid for BuiltinValuesGid {
    fn edges(&self, entity: &Id) -> Option<&HashMap<Id, Id>> {
        match entity {
            Id::String(_) => Some(string_edges()),
            Id::Number(_) => Some(number_edges()),
            Id::Uuid(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn string_has_builtin_isa() {
        assert_eq!(BuiltinValuesGid.get(&Id::String("hello".into()), &ISA), Some(&STRING_TYPE));
    }

    #[test]
    fn number_has_builtin_isa() {
        assert_eq!(BuiltinValuesGid.get(&Id::Number(ordered_float::OrderedFloat(42.0)), &ISA), Some(&NUMBER_TYPE));
    }
}
