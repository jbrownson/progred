//! Materialize one GID result without expanding shared runtime subgraphs.
//! The borrowed input stays alive for the walk; the address table dies with it.
use super::*;
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
            RuntimeValueKind::F64(value) => value
                .original
                .clone()
                .unwrap_or_else(|| crate::f64::value(value.number)),
            RuntimeValueKind::Record(fields) => self
                .share(Some(Shared::Record(Rc::as_ptr(fields))), |this| {
                    Value::record(fields.iter().map(|(key, value)| (*key, this.value(value))))
                }),
            RuntimeValueKind::List(elements) => self
                .share(Some(Shared::List(Rc::as_ptr(elements))), |this| {
                    Value::list(elements.iter().map(|value| this.value(value)))
                }),
            RuntimeValueKind::Foreign(cell) => ffi(*cell),
            RuntimeValueKind::Closure(closure) => {
                let mut fields = closure.fields.clone();
                fields.insert(vocabulary::BODY, self.value(&closure.body.0.source));
                fields.insert(
                    vocabulary::ENVIRONMENT,
                    self.environment(&closure.environment),
                );
                if let Some(origin) = source::origin(&closure.body) {
                    fields.insert(source::vocabulary::BODY_ORIGIN, source::value(&origin));
                } else {
                    fields.remove(&source::vocabulary::BODY_ORIGIN);
                }
                Value::record([(vocabulary::CLOSURE, Value::Record(fields))])
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
                let mut fields = Vec::new();
                let mut frame = environment.frame.as_deref();
                while let Some(current) = frame {
                    // Match lookup order; do not materialize shadowed values.
                    for (index, value) in current.bindings.iter().rev() {
                        if seen.insert(*index) {
                            fields.push((indices.cell(*index), this.value(value)));
                        }
                    }
                    frame = current.parent.as_deref();
                }
                Value::record(fields)
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn same_list(left: &Value, right: &Value) -> bool {
        std::ptr::eq(
            left.as_list().unwrap().iter().as_slice(),
            right.as_list().unwrap().iter().as_slice(),
        )
    }

    #[test]
    fn shared_runtime_subgraphs_stay_shared_without_interning_equal_values() {
        let leaf = RuntimeValue::list([RuntimeValue::f64(42.0)]);
        let tree = (0..12)
            .fold(leaf, |child, _| RuntimeValue::list([child.clone(), child]))
            .into_value();
        let mut node = &tree;
        for _ in 0..12 {
            let children: Vec<_> = node.as_list().unwrap().values().collect();
            assert!(same_list(children[0], children[1]));
            node = children[0];
        }
        assert_eq!(node, &Value::list([f64::value(42.0)]));

        let equal = RuntimeValue::list([
            RuntimeValue::list([RuntimeValue::f64(42.0)]),
            RuntimeValue::list([RuntimeValue::f64(42.0)]),
        ])
        .into_value();
        let children: Vec<_> = equal.as_list().unwrap().values().collect();
        assert_eq!(children[0], children[1]);
        assert!(!same_list(children[0], children[1]));
    }

    #[test]
    fn records_and_existing_gid_data_preserve_sharing_and_positions() {
        let key = gid::new_cell_id();
        let source = Value::list([Value::record([])]);
        let record = RuntimeValue::record([(key, source.clone().into())]);
        let result = RuntimeValue::list([record.clone(), record]).into_value();
        let children: Vec<_> = result.as_list().unwrap().values().collect();
        assert!(std::ptr::eq(
            children[0].as_record().unwrap().iter().as_slice(),
            children[1].as_record().unwrap().iter().as_slice(),
        ));
        assert!(same_list(
            children[0].as_record().unwrap().get(&key).unwrap(),
            &source
        ));
    }

    #[test]
    fn closures_share_captured_data_with_each_other_and_the_explicit_value() {
        let captured = gid::new_cell_id();
        let host = crate::TestHost(|_: CellId| Vec::new());
        let mut context = crate::context(&host, None, 100);
        let data = RuntimeValue::list([RuntimeValue::f64(42.0)]);
        let environment = Environment::with_indices(context.indices.clone())
            .extended_indexed([(cell_index(&context.indices, captured), data.clone())]);
        let first = context.closure_value([], captured.into(), &environment);
        let second = context.closure_value([], Value::record([]), &environment);
        let result = RuntimeValue::list([first, second, data]).into_value();
        let children: Vec<_> = result.as_list().unwrap().values().collect();
        let environment = |value: &Value| {
            value
                .as_record()
                .unwrap()
                .get(&vocabulary::CLOSURE)
                .unwrap()
                .as_record()
                .unwrap()
                .get(&vocabulary::ENVIRONMENT)
                .unwrap()
                .clone()
        };
        let first = environment(children[0]);
        let second = environment(children[1]);
        assert!(std::ptr::eq(
            first.as_record().unwrap().iter().as_slice(),
            second.as_record().unwrap().iter().as_slice(),
        ));
        assert!(same_list(
            first.as_record().unwrap().get(&captured).unwrap(),
            children[2]
        ));
        drop(context);
        let evaluated = crate::apply_value(children[0], [], &host, 100);
        assert!(evaluated.completed);
        assert_eq!(&evaluated.result, children[2]);
    }

    #[test]
    fn different_closure_environments_preserve_shared_outer_containers() {
        let (list, record, local) = (gid::new_cell_id(), gid::new_cell_id(), gid::new_cell_id());
        let host = crate::TestHost(|_: CellId| Vec::new());
        let mut context = crate::context(&host, None, 100);
        let outer = Environment::with_indices(context.indices.clone()).extended_runtime([
            (list, RuntimeValue::list([RuntimeValue::f64(42.0)])),
            (
                record,
                RuntimeValue::record([(list, RuntimeValue::f64(42.0))]),
            ),
        ]);
        let closures = RuntimeValue::list([1.0, 2.0].map(|number| {
            context.closure_value(
                [],
                local.into(),
                &outer.extended_runtime([(local, RuntimeValue::f64(number))]),
            )
        }));
        drop(outer);
        let result = closures.into_value();
        let environments: Vec<_> = result
            .as_list()
            .unwrap()
            .values()
            .map(|value| {
                value
                    .as_record()
                    .unwrap()
                    .get(&vocabulary::CLOSURE)
                    .unwrap()
                    .as_record()
                    .unwrap()
                    .get(&vocabulary::ENVIRONMENT)
                    .unwrap()
                    .as_record()
                    .unwrap()
            })
            .collect();
        let first = environments[0];
        let second = environments[1];
        assert_eq!(first.get(&local), Some(&f64::value(1.0)));
        assert_eq!(second.get(&local), Some(&f64::value(2.0)));
        assert!(same_list(
            first.get(&list).unwrap(),
            second.get(&list).unwrap()
        ));
        assert!(std::ptr::eq(
            first
                .get(&record)
                .unwrap()
                .as_record()
                .unwrap()
                .iter()
                .as_slice(),
            second
                .get(&record)
                .unwrap()
                .as_record()
                .unwrap()
                .iter()
                .as_slice(),
        ));
    }

    #[test]
    fn shadowed_bindings_are_not_materialized() {
        let key = gid::new_cell_id();
        let environment = Environment::default();
        let index = cell_index(&environment.indices, key);
        let old = RuntimeValue::list([RuntimeValue::f64(1.0)]);
        let environment = environment
            .extended_indexed([(index, old.clone())])
            .extended_indexed([(index, RuntimeValue::f64(2.0))]);
        let mut reify = Reify::default();
        let result = reify.environment(&environment);
        assert_eq!(result, Value::record([(key, f64::value(2.0))]));
        assert!(
            reify
                .shared
                .keys()
                .all(|key| matches!(key, Shared::Environment(_)))
        );
    }

    #[test]
    fn environment_materialization_matches_lookup_with_repeated_bindings() {
        let (a, b, c) = (gid::new_cell_id(), gid::new_cell_id(), gid::new_cell_id());
        let mut environment = Environment::default()
            .extended_runtime([(a, RuntimeValue::f64(1.0)), (b, RuntimeValue::f64(2.0))]);
        let outer = environment.clone();
        environment.push_runtime([(a, RuntimeValue::f64(3.0))]);
        let environment = environment.extended_runtime([
            (b, RuntimeValue::f64(4.0)),
            (b, RuntimeValue::f64(5.0)),
            (c, RuntimeValue::f64(6.0)),
        ]);
        assert_eq!(outer.get(a), Some(f64::value(1.0)));
        assert_eq!(
            Value::from(&environment),
            Value::record([a, b, c].map(|key| (key, environment.get(key).unwrap())))
        );
        assert_eq!(
            Value::from(&environment),
            Value::record([
                (a, f64::value(3.0)),
                (b, f64::value(5.0)),
                (c, f64::value(6.0))
            ])
        );
    }
}
