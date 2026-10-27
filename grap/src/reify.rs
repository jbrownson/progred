//! Materialize one GID result without expanding shared runtime subgraphs.
//! The borrowed input stays alive for the walk; the address table dies with it.
use super::*;
use crate::environment::{EnvironmentFrame, frames};
use crate::runtime::RuntimeValueKind;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Shared {
    Record(*const [(CellId, RuntimeValue)]),
    List(*const [RuntimeValue]),
    Environment(*const EnvironmentFrame),
}

#[derive(Default)]
struct Reify {
    shared: HashMap<Shared, Value>,
}

pub(super) fn value(value: &RuntimeValue) -> Value {
    Reify::default().value(value)
}

pub(super) fn environment(environment: &Environment) -> Value {
    Reify::default().environment(environment)
}

impl Reify {
    fn share(&mut self, key: Option<Shared>, build: impl FnOnce(&mut Self) -> Value) -> Value {
        if let Some(value) = key.and_then(|key| self.shared.get(&key)) {
            value.clone()
        } else {
            let value = build(self);
            if let Some(key) = key {
                self.shared.insert(key, value.clone());
            }
            value
        }
    }

    fn value(&mut self, value: &RuntimeValue) -> Value {
        match &value.0 {
            RuntimeValueKind::Data(value) => value.clone(),
            RuntimeValueKind::Unboxed(unboxed) => unboxed.to_value(),
            RuntimeValueKind::Record(fields) => self
                .share(Some(Shared::Record(Rc::as_ptr(fields))), |this| {
                    Value::record(fields.iter().map(|(key, value)| (*key, this.value(value))))
                }),
            RuntimeValueKind::List(elements) => self
                .share(Some(Shared::List(Rc::as_ptr(elements))), |this| {
                    Value::list(elements.iter().map(|value| this.value(value)))
                }),
            // The lambda as written, then what it closes over and where its
            // body came from, beside it.
            RuntimeValueKind::Closure(closure) => {
                let mut lambda = closure.fields.clone();
                lambda.set(vocabulary::BODY, self.value(&closure.body.0.source));
                Value::record(
                    [
                        (crate::isa::ISA, Value::from(vocabulary::kind::CLOSURE)),
                        (vocabulary::LAMBDA, Value::Record(lambda)),
                        (
                            vocabulary::ENVIRONMENT,
                            self.environment(&closure.environment),
                        ),
                    ]
                    .into_iter()
                    .chain(
                        source::origin(&closure.body).map(|origin| {
                            (source::vocabulary::BODY_ORIGIN, source::value(&origin))
                        }),
                    ),
                )
            }
        }
    }

    fn environment(&mut self, environment: &Environment) -> Value {
        self.share(
            environment
                .frame
                .as_ref()
                .map(|frame| Shared::Environment(Rc::as_ptr(frame))),
            |this| {
                let indices = environment.indices.borrow();
                let mut seen = HashSet::new();
                // Match lookup order; do not materialize shadowed values.
                Value::record(
                    frames(environment)
                        .flat_map(|frame| frame.bindings.iter().rev())
                        .filter(|(index, _)| seen.insert(*index))
                        .map(|(index, value)| (indices.cell(*index), this.value(value))),
                )
            },
        )
    }
}

#[cfg(test)]
#[path = "tests/reify.rs"]
mod tests;
