use super::*;
use crate::display::recording::{Recorded, record};
use crate::display::test_support::{ProjectionCall, inspect};

fn unshared<World, Hover>(mut layout: &Recorded<World, Hover>) -> &Recorded<World, Hover> {
    while let Recorded::Shared { child, .. } = layout {
        layout = child.as_ref();
    }
    layout
}

fn probe(_: &ProjectionInput<'_, (), ()>) -> Option<Layout<(), ()>> {
    None
}

#[test]
fn stored_paths_jump_and_supplied_values_are_distinct_operations() {
    let steps = vec![Step::Key(gid::new_cell_id()), Step::Key(gid::new_cell_id())];
    let source = vec![Step::Key(gid::new_cell_id())];
    let layout: Layout<(), ()> = descend_path_local(steps.clone(), partial(probe), &partial(probe));
    match inspect(&layout) {
        ProjectionCall::DescendPath {
            steps: actual,
            projection,
            default_projection,
        } => {
            assert_eq!(actual, steps);
            assert!(projection.is_some());
            assert!(default_projection.is_some());
        }
        _ => panic!("stored traversal"),
    }
    let layout: Layout<(), ()> = jump(steps.clone(), source.clone());
    match inspect(&layout) {
        ProjectionCall::Jump {
            steps: actual,
            document,
            conject,
            projection,
            default_projection,
        } => {
            assert_eq!(actual, steps);
            assert_eq!(document, crate::location::Location::root(source.clone()));
            assert_eq!(
                conject.apply(&steps, &document.path),
                Some([source, steps].concat())
            );
            assert!(projection.is_none());
            assert!(default_projection.is_none());
        }
        _ => panic!("source jump"),
    }
    assert_eq!(Conject::detached().apply(&[], &[]), None);
}

fn run_partial(projection: &Partial<(), ()>) -> Option<Layout<(), ()>> {
    struct NoEval;
    impl Env for NoEval {
        fn apply_scoped(
            &self,
            _: &Value,
            _: &[(CellId, Value)],
            _scope: Option<&grap::ForeignOverlay<'_>>,
        ) -> grap::Evaluation<gid::Value> {
            panic!("unexpected application")
        }

        fn evaluate(&self, _: &Value) -> Value {
            panic!("unexpected evaluation")
        }
    }

    projection(&ProjectionInput {
        default_projection: partial(|_| None),
        env: &NoEval,
        value: Some(&Value::record([]).into()),
        scale_factor: 1.0,
        writable: false,
        selection: None,
        pending: None,
        state: None,
        targets: ProjectionTargets::new(&|_| panic!("unexpected target lookup")),
    })
}

#[test]
fn empty_partial_composition_is_an_identity() {
    let empty = compose_partials([]);
    assert!(run_partial(&empty).is_none());
    let success = partial(|_| Some(slot()));
    for projection in [
        compose_partials([empty.clone(), success.clone()]),
        compose_partials([success, empty]),
    ] {
        assert!(run_partial(&projection).is_some());
    }
}

#[test]
fn partial_composition_preserves_order_and_stops_at_success() {
    let visited = Rc::new(std::cell::RefCell::new(Vec::new()));
    let projection = |index, succeeds: bool| {
        let visited = visited.clone();
        partial(move |_| {
            visited.borrow_mut().push(index);
            succeeds.then(slot)
        })
    };
    let first = projection(1, false);
    let second = projection(2, false);
    let third = projection(3, true);
    let unreachable = partial(|_| panic!("later partial must not run"));
    for combined in [
        compose_partials([
            first.clone(),
            second.clone(),
            third.clone(),
            unreachable.clone(),
        ]),
        compose_partials([
            compose_partials([first.clone(), second.clone()]),
            compose_partials([third.clone(), unreachable.clone()]),
        ]),
        compose_partials([first, compose_partials([second, third, unreachable])]),
    ] {
        visited.borrow_mut().clear();
        assert!(run_partial(&combined).is_some());
        assert_eq!(*visited.borrow(), [1, 2, 3]);
    }
}

#[test]
fn all_declining_partials_leave_fallback_to_the_caller() {
    assert!(run_partial(&compose_partials([partial(probe), partial(probe)])).is_none());
}

#[test]
fn record_uses_the_supplied_order_in_both_responsive_forms() {
    const FIRST: CellId = CellId::from_u128(1);
    const SECOND: CellId = CellId::from_u128(2);
    let first = Value::from(vec![1]);
    let second = Value::from(vec![2]);
    let layout: crate::view::Layout = crate::display::record(
        [(FIRST, &first), (SECOND, &second)],
        |left, right| right.cmp(left),
        |key, value| RecordField {
            label: dim("field"),
            value: at([Step::Key(key)], value),
        },
    );
    let layout = record(&layout);
    let (_, child, _) = crate::display::test_support::delimited(&layout);
    let Recorded::Alternatives(forms) = child else {
        panic!("a record has responsive forms");
    };
    let Recorded::Row { children, .. } = forms[0].content() else {
        panic!("the first form is flat");
    };
    let Recorded::Row {
        children: first, ..
    } = children[0].content()
    else {
        panic!("a flat field keeps its label and value together");
    };
    assert!(matches!(&inspect(&(unshared(&first[2]))),
        ProjectionCall::At { steps, .. } if *steps == [Step::Key(SECOND)]
    ));
    let Recorded::Row {
        children: second, ..
    } = children[2].content()
    else {
        panic!("a flat field keeps its label and value together");
    };
    assert!(matches!(&inspect(&(unshared(&second[2]))),
        ProjectionCall::At { steps, .. } if *steps == [Step::Key(FIRST)]
    ));
    let Recorded::Col { children, .. } = forms[1].content() else {
        panic!("the second form is a column");
    };
    let Recorded::Alternatives(first) = children[0].content() else {
        panic!("a column field may break after its label");
    };
    let Recorded::Row {
        children: inline, ..
    } = first[0].content()
    else {
        panic!("a field first stays inline");
    };
    assert!(matches!(&inspect(&(unshared(&inline[1]))),
        ProjectionCall::At { steps, .. } if *steps == [Step::Key(SECOND)]
    ));
    let Recorded::Col {
        children: broken, ..
    } = first[1].content()
    else {
        panic!("a field may put its value below its label");
    };
    let Recorded::Pad { child, .. } = &broken[1] else {
        panic!("a broken value is indented");
    };
    assert!(matches!(&inspect(&(unshared(child))),
        ProjectionCall::At { steps, .. } if *steps == [Step::Key(SECOND)]
    ));
}
