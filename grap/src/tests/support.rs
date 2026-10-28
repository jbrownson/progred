use crate::{ForeignFunction, Host, isa, vocabulary};
use gid::{CellId, Record, Value};

/// A test cell's definition and, for a foreign function, the code it runs.
#[derive(Clone)]
pub(crate) struct TestDefinition {
    value: Value,
    implementation: Option<ForeignFunction>,
}

impl From<Value> for TestDefinition {
    fn from(value: Value) -> Self {
        Self {
            value,
            implementation: None,
        }
    }
}

impl TestDefinition {
    /// `cell` as `implementation`: its `description` says it's a foreign
    /// function implemented under `cell`, with the parameters it declares
    /// unless it lists its own.
    pub(crate) fn foreign(
        cell: CellId,
        mut description: Record,
        implementation: ForeignFunction,
    ) -> Self {
        description.set(isa::ISA, Value::from(vocabulary::kind::FOREIGN_FUNCTION));
        description.set(vocabulary::IMPLEMENTATION, Value::from(cell));
        if let Some(parameters) = implementation.declared_parameters()
            && description.get(&vocabulary::PARAMS).is_none()
        {
            description.set(
                vocabulary::PARAMS,
                Value::list(parameters.iter().map(|&parameter| parameter.into())),
            );
        }
        Self {
            value: Value::Record(description),
            implementation: Some(implementation),
        }
    }

    pub(crate) fn value(&self) -> &Value {
        &self.value
    }
}

/// A host defining each cell as its function says.
pub(crate) struct TestHost<F>(pub(crate) F);

impl<F: Fn(CellId) -> Option<TestDefinition>> Host for TestHost<F> {
    fn definition(&self, cell: CellId) -> Option<Value> {
        (self.0)(cell).map(|definition| definition.value)
    }

    fn implementation(&self, cell: CellId) -> Option<ForeignFunction> {
        (self.0)(cell)?.implementation
    }
}
