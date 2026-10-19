use super::*;
use crate::test_support::ask_over;
use ::grap::vocabulary::EXPRESSION;
use gid::Cells;

fn visible(root: Value, path: &[Step]) -> BTreeSet<CellId> {
    let document = gid::Document {
        root: Some(root),
        cells: Cells::new(),
    };
    let libraries = crate::library::Libraries::default();
    let sources = crate::sources::Sources {
        doc: &document,
        libraries: &libraries,
    };
    ask_over(
        &sources,
        CompletionKind::Value,
        CompletionScope::Everything,
        "",
        path,
        bindings,
    )
    .0
    .into_keys()
    .collect()
}

#[test]
fn completion_bindings_sort_nearest_scope_first_then_by_current_name() {
    let mut ids = [gid::new_cell_id(), gid::new_cell_id(), gid::new_cell_id()];
    ids.sort();
    let [outer, inner_z, inner_a] = ids;
    let mut document = gid::Document {
        root: Some(::grap::lambda(
            [outer],
            ::grap::lambda([inner_z, inner_a], Value::record([])),
        )),
        cells: Cells::new(),
    };
    for (cell, label) in [(outer, "apple"), (inner_z, "Zebra"), (inner_a, "banana")] {
        document.cells.set_value(cell, name::record(label, []));
    }
    let libraries = crate::library::Libraries::default();
    for (label, expected) in [
        ("banana", [inner_a, inner_z, outer]),
        ("zz", [inner_z, inner_a, outer]),
    ] {
        document.cells.set_value(inner_a, name::record(label, []));
        let sources = crate::sources::Sources {
            doc: &document,
            libraries: &libraries,
        };
        let offers = ask_over(
            &sources,
            CompletionKind::Value,
            CompletionScope::Everything,
            "",
            &[Step::Key(BODY), Step::Key(BODY)],
            offers,
        )
        .unwrap();
        assert_eq!(
            offers
                .iter()
                .filter(|offer| offer.detail == Some("binding".into()))
                .filter_map(|offer| offer.test_value().as_cell())
                .collect::<Vec<_>>(),
            expected,
        );
    }
    let root = ::grap::lambda([outer, inner_z], ::grap::lambda([outer], Value::record([])));
    document.root = Some(root);
    let sources = crate::sources::Sources {
        doc: &document,
        libraries: &libraries,
    };
    let (depths, _) = ask_over(
        &sources,
        CompletionKind::Value,
        CompletionScope::Everything,
        "",
        &[Step::Key(BODY), Step::Key(BODY)],
        bindings,
    );
    assert_eq!(
        depths[&outer], 1,
        "a rebound cell belongs to the nearest scope"
    );
    assert_eq!(depths[&inner_z], 0);
}

#[test]
fn completion_bindings_follow_lambda_scope_and_quote_boundaries() {
    let outer = gid::new_cell_id();
    let inner = gid::new_cell_id();
    let lambda = |parameter, body| {
        Value::record([
            (PARAMS, Value::list([Value::from(parameter)])),
            (BODY, body),
        ])
    };
    let root = lambda(outer, lambda(inner, Value::record([])));
    assert_eq!(
        visible(root.clone(), &[Step::Key(BODY), Step::Key(BODY)]),
        BTreeSet::from([outer, inner])
    );
    assert_eq!(
        visible(root, &[Step::Key(BODY), Step::Key(PARAMS)]),
        BTreeSet::from([outer])
    );
    let root = lambda(
        outer,
        ::grap::call(
            control::QUOTE.into(),
            [(
                EXPRESSION,
                lambda(
                    inner,
                    Value::record([(control::UNQUOTE, Value::record([]))]),
                ),
            )],
        ),
    );
    assert_eq!(
        visible(
            root,
            &[
                Step::Key(BODY),
                Step::Key(EXPRESSION),
                Step::Key(BODY),
                Step::Key(control::UNQUOTE)
            ]
        ),
        BTreeSet::from([outer])
    );
}

#[test]
fn completion_bindings_include_only_preceding_let_bindings_and_current_match_case() {
    let first = gid::new_cell_id();
    let second = gid::new_cell_id();
    let binder = |cell| Value::record([(control::BIND, Value::from(cell))]);
    let clauses = Value::list([
        Value::record([
            (control::BIND, first.into()),
            (control::VALUE, Value::record([])),
        ]),
        Value::record([
            (control::PATTERN, Value::list([binder(second)])),
            (control::VALUE, Value::record([])),
        ]),
    ]);
    let positions: Vec<_> = clauses
        .as_list()
        .unwrap()
        .iter()
        .map(|(position, _)| position.clone())
        .collect();
    for function in [control::LET, control::WHERE] {
        let root = ::grap::call(
            function.into(),
            [
                (control::BINDINGS, clauses.clone()),
                (EXPRESSION, Value::record([])),
            ],
        );
        assert_eq!(
            visible(root.clone(), &[Step::Key(EXPRESSION)]),
            BTreeSet::from([first, second])
        );
        for (index, expected) in [BTreeSet::new(), BTreeSet::from([first])]
            .into_iter()
            .enumerate()
        {
            assert_eq!(
                visible(
                    root.clone(),
                    &[
                        Step::Key(control::BINDINGS),
                        Step::Element(positions[index].clone()),
                        Step::Key(control::VALUE)
                    ]
                ),
                expected
            );
        }
    }
    let cases = Value::list([first, second].map(|cell| {
        Value::record([
            (control::PATTERN, binder(cell)),
            (EXPRESSION, Value::record([])),
        ])
    }));
    let positions: Vec<_> = cases
        .as_list()
        .unwrap()
        .iter()
        .map(|(position, _)| position.clone())
        .collect();
    let root = ::grap::call(
        control::MATCH.into(),
        [(control::CASES, cases), (control::VALUE, Value::record([]))],
    );
    for (position, cell) in positions.into_iter().zip([first, second]) {
        assert_eq!(
            visible(
                root.clone(),
                &[
                    Step::Key(control::CASES),
                    Step::Element(position),
                    Step::Key(EXPRESSION)
                ]
            ),
            BTreeSet::from([cell])
        );
    }
    assert!(visible(root, &[Step::Key(control::VALUE)]).is_empty());
}
