use super::*;
use crate::display::{CompletionRequest, CompletionScope};
use crate::libraries::{completion::select, control::vocabulary as control, text};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

pub(super) fn provider(libraries: Option<CompletionProvider>) -> CompletionProvider {
    Rc::new(move |request| {
        if let Some(offers) = libraries.as_ref().and_then(|provider| {
            provider(&CompletionRequest {
                scope: CompletionScope::Suggested,
                ..*request
            })
        }) {
            return Some(offers);
        }
        let quoted = request.query.trim().starts_with('"');
        let bindings = bindings(request);
        let mut ordered: Vec<_> = bindings.iter().filter(|_| !quoted).collect();
        ordered.sort_by_cached_key(|(cell, depth)| {
            let name = (request.resolve)(**cell)
                .and_then(|definition| name::read(definition.value))
                .unwrap_or("");
            (Reverse(**depth), name.to_lowercase(), name)
        });
        let mut offers: Vec<_> = ordered
            .into_iter()
            .map(|(cell, _)| select(Completion::new(*cell, (*cell).into()).with_detail("binding")))
            .collect();
        if !quoted {
            let library_request = CompletionRequest {
                scope: CompletionScope::Everything,
                ..*request
            };
            offers.extend(
                libraries
                    .as_ref()
                    .and_then(|provider| provider(&library_request))
                    .unwrap_or_default()
                    .into_iter()
                    .chain(
                        request
                            .query
                            .trim()
                            .is_empty()
                            .then(|| super::call_completions(request))
                            .into_iter()
                            .flatten(),
                    )
                    .filter(|offer| {
                        !offer
                            .value
                            .literal()
                            .and_then(Value::as_record)
                            .and_then(|fields| fields.get(&FUNCTION))
                            .and_then(Value::as_cell)
                            .is_some_and(|cell| bindings.contains_key(&cell))
                    }),
            );
            offers.extend([
                select(Completion::new("new list", Value::list([])).with_aliases(["["])),
                select(Completion::new("new record", Value::record([])).with_aliases(["{"])),
            ]);
        }
        offers.push(text::completion(text::query_spelling(request.query)));
        Some(offers)
    })
}

fn pattern_bindings(value: &Value, result: &mut BTreeSet<CellId>) {
    match value {
        Value::Record(fields) => match fields.get(&control::BIND) {
            Some(binder) => result.extend(binder.as_cell()),
            None => fields
                .values()
                .for_each(|value| pattern_bindings(value, result)),
        },
        Value::List(values) => values
            .values()
            .for_each(|value| pattern_bindings(value, result)),
        _ => (),
    }
}

fn binding_names(value: &Value, result: &mut BTreeSet<CellId>) {
    if let Some(fields) = value.as_record() {
        match (fields.get(&control::BIND), fields.get(&control::PATTERN)) {
            (Some(binder), None) => result.extend(binder.as_cell()),
            (None, Some(pattern)) => pattern_bindings(pattern, result),
            _ => (),
        }
    }
}

/// Statically visible bindings only; no evaluation or search through unrelated cells.
fn bindings(request: &CompletionRequest<'_>) -> BTreeMap<CellId, usize> {
    use ::grap::vocabulary::{ENVIRONMENT, EXPRESSION};
    let mut result = BTreeMap::new();
    let mut quoted = false;
    for offset in 0..request.path.len() {
        let Some(fields) = (request.value_at)(&request.path[..offset]).and_then(Value::as_record)
        else {
            continue;
        };
        let suffix = &request.path[offset..];
        if quoted {
            if matches!(suffix.first(), Some(Step::Key(field)) if *field == control::UNQUOTE) {
                quoted = false;
            }
            continue;
        }
        let mut found = BTreeSet::new();
        match fields.get(&FUNCTION).and_then(Value::as_cell) {
            Some(control::QUOTE) if matches!(suffix.first(), Some(Step::Key(EXPRESSION))) => {
                quoted = true;
            }
            Some(control::LET | control::WHERE) => {
                if let Some(list) = fields.get(&control::BINDINGS).and_then(Value::as_list) {
                    match suffix {
                        [Step::Key(EXPRESSION), ..] => {
                            list.values()
                                .for_each(|value| binding_names(value, &mut found));
                        }
                        [
                            Step::Key(control::BINDINGS),
                            Step::Element(current),
                            Step::Key(control::VALUE),
                            ..,
                        ] => {
                            list.iter()
                                .take_while(|(position, _)| *position < *current)
                                .for_each(|(_, value)| binding_names(value, &mut found));
                        }
                        _ => (),
                    }
                }
            }
            Some(control::MATCH) => {
                if let [
                    Step::Key(control::CASES),
                    Step::Element(position),
                    Step::Key(EXPRESSION),
                    ..,
                ] = suffix
                    && let Some(pattern) = fields
                        .get(&control::CASES)
                        .and_then(Value::as_list)
                        .and_then(|cases| cases.get(position))
                        .and_then(Value::as_record)
                        .and_then(|case| case.get(&control::PATTERN))
                {
                    pattern_bindings(pattern, &mut found);
                }
            }
            None if !fields.contains_key(&FUNCTION)
                && matches!(suffix.first(), Some(Step::Key(BODY))) =>
            {
                if let Some(params) = fields.get(&PARAMS).and_then(Value::as_list) {
                    found.extend(params.values().filter_map(Value::as_cell));
                    if let Some(environment) = fields.get(&ENVIRONMENT).and_then(Value::as_record) {
                        found.extend(environment.keys().copied());
                    }
                }
            }
            _ => (),
        }
        result.extend(found.into_iter().map(|cell| (cell, offset)));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::grap::vocabulary::EXPRESSION;

    fn visible(root: Value, path: &[Step]) -> BTreeSet<CellId> {
        let document = gid::Document {
            root: Some(root),
            cells: Cells::new(),
        };
        let libraries = crate::libraries::Libraries::default();
        let sources = crate::sources::Sources {
            doc: &document,
            libraries: &libraries,
        };
        bindings(&CompletionRequest {
            query: "",
            kind: CompletionKind::Value,
            scope: CompletionScope::Suggested,
            path,
            value_at: &|path| sources.resolve_path(path),
            resolve: &|cell| sources.definition(cell),
            cells: &Vec::new,
        })
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
        let libraries = crate::libraries::Libraries::default();
        for (label, expected) in [
            ("banana", [inner_a, inner_z, outer]),
            ("zz", [inner_z, inner_a, outer]),
        ] {
            document.cells.set_value(inner_a, name::record(label, []));
            let sources = crate::sources::Sources {
                doc: &document,
                libraries: &libraries,
            };
            let request = CompletionRequest {
                query: "",
                kind: CompletionKind::Value,
                scope: CompletionScope::Suggested,
                path: &[Step::Key(BODY), Step::Key(BODY)],
                value_at: &|path| sources.resolve_path(path),
                resolve: &|cell| sources.definition(cell),
                cells: &Vec::new,
            };
            let offers = provider(None)(&request).unwrap();
            assert_eq!(
                offers
                    .iter()
                    .filter_map(|offer| offer.value.instantiate().as_cell())
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
        let depths = bindings(&CompletionRequest {
            query: "",
            kind: CompletionKind::Value,
            scope: CompletionScope::Suggested,
            path: &[Step::Key(BODY), Step::Key(BODY)],
            value_at: &|path| sources.resolve_path(path),
            resolve: &|cell| sources.definition(cell),
            cells: &Vec::new,
        });
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
}
