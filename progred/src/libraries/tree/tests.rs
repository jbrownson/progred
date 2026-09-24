use super::*;
use crate::libraries::{control::vocabulary as c, f64};
use ::grap::vocabulary as g;

const A: CellId = CellId::from_u128(1);
const B: CellId = CellId::from_u128(2);
const C: CellId = CellId::from_u128(3);

mod runtime;

fn call(function: CellId, fields: impl IntoIterator<Item = (CellId, Value)>) -> Value {
    ::grap::call(function.into(), fields)
}
fn group(children: Value) -> Value {
    call(GROUP, [(CHILDREN, children)])
}
fn leaf(value: Value) -> Value {
    call(LEAF, [(VALUE, value)])
}
fn sequence(children: impl IntoIterator<Item = Value>) -> Value {
    call(c::DO, [(c::EXPRESSIONS, Value::list(children))])
}
fn document(body: Value) -> gid::Document {
    let mut cells = Cells::new();
    cells.set_value(A, ::grap::lambda([], body));
    gid::Document {
        root: Some(A.into()),
        cells,
    }
}
fn with_host<T>(doc: &gid::Document, f: impl FnOnce(&crate::sources::Sources) -> T) -> T {
    let libraries = crate::stack::load().libraries;
    f(&crate::sources::Sources {
        doc,
        libraries: &libraries,
    })
}

fn sources(node: &Node) -> Vec<&SourceOrigin> {
    node.source
        .iter()
        .chain(
            node.children
                .iter()
                .flat_map(|children| children.values().flat_map(sources)),
        )
        .collect()
}

#[test]
fn lists_are_groups_and_equal_leaves_have_distinct_occurrences() {
    let doc = document(group(Value::list([
        leaf(f64::value(1.0)),
        Value::list([leaf(f64::value(1.0))]),
    ])));
    let result = with_host(&doc, |host| {
        build(&Value::from(A).into(), host, 1000).unwrap()
    });
    assert_eq!(
        result.items.to_value(),
        Value::list([f64::value(1.0), Value::list([f64::value(1.0)])])
    );
    let origins = sources(&result.root);
    assert_eq!(origins.len(), 4);
    let root = vec![gid::Step::Key(g::BODY)];
    assert!(
        matches!(&result.root.source, Some(SourceOrigin::Cell {cell, path, ..}) if *cell==A && *path==root)
    );
    for source in &origins {
        let SourceOrigin::Cell { cell, path, .. } = source else {
            panic!("expected stored provenance")
        };
        assert_eq!(*cell, A);
        let target = path
            .iter()
            .try_fold(doc.cells.value(A).unwrap(), |value, step| match step {
                gid::Step::Key(key) => value.as_record()?.get(key),
                gid::Step::Element(position) => value.as_list()?.get(position),
                _ => None,
            });
        let target = target.unwrap();
        assert!(
            target.as_list().is_some()
                || matches!(
                    target
                        .as_record()
                        .and_then(|fields| fields.get(&g::FUNCTION))
                        .and_then(Value::as_cell),
                    Some(GROUP | LEAF)
                ),
            "emissions must link to their calls, not their arguments: {path:?}"
        );
    }
    let mut sources = Vec::new();
    for source in origins {
        if !sources.contains(&source) {
            sources.push(source);
        }
    }
    assert_eq!(sources.len(), 4);
}

#[test]
fn alternate_consumer_runs_without_collecting_a_tree() {
    #[derive(Default)]
    struct Count {
        groups: usize,
        depth: usize,
        leaves: usize,
    }
    impl Sink for Count {
        fn begin_group(&mut self, _: Option<SourceOrigin>) {
            self.groups += 1;
            self.depth += 1;
        }
        fn end_group(&mut self) {
            self.depth -= 1;
        }
        fn leaf(&mut self, _: RuntimeValue, _: Option<SourceOrigin>) {
            self.leaves += 1;
        }
    }
    let doc = document(group(Value::list([
        leaf(Value::record([])),
        Value::list([leaf(Value::record([]))]),
    ])));
    let sink = Rc::new(RefCell::new(Count::default()));
    with_host(&doc, |host| {
        let emit = |_, context: &mut Context, _: &Expression, _: &Environment| {
            interpret(context, sink.clone(), |context| {
                context.apply_value(&A.into(), [])
            })
        };
        let result = ::grap::evaluate_value_scoped(
            &call(B, []),
            host,
            &::grap::ForeignOverlay::from_value(&[B], &emit).tracked(),
            1000,
        );
        assert!(result.completed && !absent::is_absent(&result.result));
    });
    let count = sink.borrow();
    assert_eq!((count.groups, count.depth, count.leaves), (2, 0, 2));
}

#[test]
fn leaf_mapping_composes_inside_out_without_changing_origins() {
    let arithmetic = |function, right| {
        ::grap::lambda(
            [VALUE],
            call(
                function,
                [
                    (f64::vocabulary::LEFT, VALUE.into()),
                    (f64::vocabulary::RIGHT, f64::value(right)),
                ],
            ),
        )
    };
    let map = |mapping, children| call(MAP, [(MAPPING, mapping), (CHILDREN, children)]);
    let doc = document(group(Value::list([
        map(
            arithmetic(f64::vocabulary::MULTIPLY, 2.0),
            map(arithmetic(f64::vocabulary::SUM, 1.0), leaf(f64::value(2.0))),
        ),
        leaf(f64::value(2.0)),
    ])));
    let result = with_host(&doc, |host| {
        build(&Value::from(A).into(), host, 1000).unwrap()
    });
    assert_eq!(
        result.items.to_value(),
        Value::list([f64::value(6.0), f64::value(2.0)])
    );
    for source in sources(&result.root).into_iter().skip(1) {
        let SourceOrigin::Cell { path, .. } = source else {
            panic!()
        };
        let target = path
            .iter()
            .try_fold(doc.cells.value(A).unwrap(), |value, step| match step {
                gid::Step::Key(key) => value.as_record()?.get(key),
                gid::Step::Element(position) => value.as_list()?.get(position),
                _ => None,
            })
            .unwrap();
        assert_eq!(
            target.as_record().unwrap().get(&g::FUNCTION),
            Some(&LEAF.into()),
            "mapping retains the leaf call's origin, not the mapper or the leaf argument"
        );
    }
}

#[test]
fn failures_stop_children_and_scopes_balance_when_a_caller_recovers() {
    let failure = absent::with_reason(B);
    // Absents keep effects, as elsewhere in Grap; recovering doesn't roll back
    // previously emitted leaves. It does close groups and remove leaf mappings.
    let fail = call(
        MAP,
        [
            (MAPPING, ::grap::lambda([VALUE], f64::value(99.0))),
            (
                CHILDREN,
                group(Value::list([
                    leaf(f64::value(1.0)),
                    failure.clone(),
                    leaf(f64::value(3.0)),
                ])),
            ),
        ],
    );
    let recover = call(
        c::MATCH,
        [
            (c::VALUE, fail),
            (
                c::CASES,
                Value::list([Value::record([
                    (c::PATTERN, Value::record([(c::BIND, C.into())])),
                    (g::EXPRESSION, leaf(f64::value(4.0))),
                ])]),
            ),
        ],
    );
    let doc = document(group(recover));
    let result = with_host(&doc, |host| {
        build(&Value::from(A).into(), host, 1000).unwrap()
    });
    assert_eq!(
        result.items.to_value(),
        Value::list([Value::list([f64::value(99.0)]), f64::value(4.0)])
    );
    let doc = document(group(Value::list([leaf(f64::value(1.0)), failure.clone()])));
    assert_eq!(
        with_host(&doc, |host| build(&Value::from(A).into(), host, 1000))
            .err()
            .map(RuntimeValue::into_value),
        Some(failure)
    );
    assert!(with_host(&doc, |host| build(&Value::from(A).into(), host, 1)).is_err());
    let doc = document(sequence([leaf(f64::value(1.0)), leaf(f64::value(2.0))]));
    assert_eq!(
        with_host(&doc, |host| build(&Value::from(A).into(), host, 1000))
            .err()
            .map(RuntimeValue::into_value),
        Some(absent::with_reason(INVALID_OUTPUT))
    );
}

#[test]
fn memo_tracks_definition_reads_absents_and_unrelated_edits() {
    let mut doc = document(group(Value::list([leaf(B.into())])));
    let libraries = crate::stack::load().libraries;
    let computations = crate::computations::Computations::default();
    let root = crate::workspace::Root::document();
    let demand = |doc: &gid::Document| {
        computations.begin(Rc::new(doc.clone()), libraries.clone());
        prepared(&computations, &root, &[], Value::from(A).into(), 1000)
    };
    let missing = demand(&doc);
    assert!(missing.is_err());
    assert!(Rc::ptr_eq(&missing, &demand(&doc)));
    doc.cells.set_value(B, f64::value(7.0));
    let first = demand(&doc);
    assert_eq!(
        first.as_ref().as_ref().unwrap().items.to_value(),
        Value::list([f64::value(7.0)])
    );
    doc.cells.set_value(C, f64::value(9.0));
    assert!(Rc::ptr_eq(&first, &demand(&doc)));
    doc.cells.set_value(B, f64::value(8.0));
    assert_eq!(
        demand(&doc).as_ref().as_ref().unwrap().items.to_value(),
        Value::list([f64::value(8.0)])
    );
}

#[test]
fn nested_collect_checks_arguments_and_does_not_steal_outer_emissions() {
    let doc = document(group(Value::list([
        leaf(call(
            COLLECT,
            [(
                PROGRAM,
                ::grap::lambda([], group(Value::list([leaf(f64::value(2.0))]))),
            )],
        )),
        leaf(f64::value(3.0)),
    ])));
    let result = with_host(&doc, |host| {
        build(&Value::from(A).into(), host, 1000).unwrap()
    });
    assert_eq!(
        result.items.to_value(),
        Value::list([Value::list([f64::value(2.0)]), f64::value(3.0)])
    );
    assert!(
        result
            .root
            .children
            .as_ref()
            .unwrap()
            .values()
            .all(|node| node.children.is_none()),
        "collecting a list into a leaf must not turn the leaf into a group"
    );
    let doc = document(group(call(COLLECT, [])));
    assert!(with_host(&doc, |host| build(&Value::from(A).into(), host, 1000)).is_err());
}

#[test]
fn untracked_builders_and_halted_runs_are_not_memoized() {
    use std::cell::Cell;
    for tracked in [false, true] {
        let runs = Rc::new(Cell::new(0));
        let mut definitions = Definitions::default();
        let implementation = ForeignFunction::from_value({
            let runs = runs.clone();
            move |_, _, _| {
                runs.set(runs.get() + 1);
                Ok(Value::record([]))
            }
        });
        definitions.insert(
            B,
            ::grap::Definition::foreign(
                Value::record([]),
                if tracked {
                    implementation.tracked()
                } else {
                    implementation
                },
            ),
        );
        let mut libraries = crate::stack::load().libraries;
        libraries.insert(C, definitions);
        let body = if tracked {
            group(sequence([call(B, []), call(A, [])]))
        } else {
            group(Value::list([leaf(call(B, []))]))
        };
        let doc = document(body);
        let computations =
            crate::computations::Computations::from_sources(crate::sources::Sources {
                doc: &doc,
                libraries: &libraries,
            });
        let root = crate::workspace::Root::document();
        let first = prepared(&computations, &root, &[], Value::from(A).into(), 100);
        let before = runs.get();
        assert!(before > 0);
        let second = prepared(&computations, &root, &[], Value::from(A).into(), 100);
        assert!(runs.get() > before);
        assert_eq!(first.is_err(), tracked);
        assert_eq!(second.is_err(), tracked);
    }
}
