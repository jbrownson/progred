use super::*;
use crate::display::{CompletionRequest, CompletionScope};
use crate::libraries::{completion::select, grap::control::vocabulary as control};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

/// A slot's offers: the bindings in scope, the libraries' offers, Raw's
/// constructors, and what's typed read as a value.
pub(super) fn offers(request: &CompletionRequest<'_>) -> Option<Vec<Completion>> {
    let quoted = request.query.trim().starts_with('"');
    let (bindings, templated) = scope(request.path, request.value_at, &|cell| {
        (request.resolve)(cell).map(|definition| definition.value)
    });
    let mut offers: Vec<_> = templated.then(unquote).into_iter().collect();
    if !quoted {
        // The closest binder's first.
        let mut ordered: Vec<_> = bindings.iter().collect();
        ordered.sort_by_cached_key(|(cell, binding)| {
            let name = binding
                .computed
                .clone()
                .or_else(|| (request.name)(**cell).map(|name| name.spelling().into_owned()))
                .unwrap_or_default();
            (Reverse(binding.depth), name.to_lowercase(), name)
        });
        offers.extend(ordered.into_iter().map(|(cell, binding)| {
            match &binding.computed {
                // A parameter with no name of its own offers what its list
                // calls it, grey, as it's drawn.
                Some(computed) => Completion {
                    face: crate::display::Face::Dim,
                    ..select(computed.clone(), (*cell).into())
                },
                None => select(*cell, (*cell).into()),
            }
            .with_detail("binding")
        }));
        let library_request = CompletionRequest {
            scope: CompletionScope::Everything,
            ..*request
        };
        let library_offers: Vec<_> = request
            .libraries
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
                    .preview
                    .as_ref()
                    .and_then(Value::as_record)
                    .and_then(|fields| fields.get(&FUNCTION))
                    .and_then(Value::as_cell)
                    .is_some_and(|cell| cell == control::QUOTE || bindings.contains_key(&cell))
            })
            .collect();
        offers.extend(library_offers);
        offers.extend(crate::display::raw::constructors(request.kind));
    }
    if !templated {
        offers.extend(quote(request));
    }
    // What's typed, read as a value: a number, say, then text.
    offers.extend(crate::display::raw::typed(request));
    Some(offers)
}

/// Quote is drawn as `"`, so typing that mark offers it beside the string.
fn quote(request: &CompletionRequest<'_>) -> Option<Completion> {
    let source = (request.resolve)(control::QUOTE)?.resolution;
    Some(
        super::call_completion(control::QUOTE.into(), control::QUOTE, request.resolve)
            .with_detail(super::call_detail(request, source))
            .with_aliases(["\""]),
    )
}

/// Unquote is drawn as `` ` ``; its hole is code again, inside the template.
fn unquote() -> Completion {
    crate::libraries::completion::insert(
        control::UNQUOTE,
        Value::record([]),
        Some(crate::selection::pending_at(&[Step::Key(control::UNQUOTE)])),
    )
    .with_aliases(["`"])
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

/// A binding in scope: how far in its binder is, and for a parameter with
/// no name of its own, what its list calls it.
pub(super) struct Binding {
    pub(super) depth: usize,
    pub(super) computed: Option<String>,
}

/// The bindings statically visible at `path`, found by walking out through
/// what encloses it, each read by `value_at` (`value_of` reads cells'
/// definitions); and whether `path` is in a quote's template, which is data
/// apart from its unquotes. No evaluation or search through unrelated cells.
pub(super) fn scope<'a>(
    path: &[Step],
    value_at: &dyn Fn(&[Step]) -> Option<&'a Value>,
    value_of: &dyn Fn(CellId) -> Option<&'a Value>,
) -> (BTreeMap<CellId, Binding>, bool) {
    use ::grap::vocabulary::{ENVIRONMENT, EXPRESSION, LAMBDA};
    let mut result = BTreeMap::new();
    let mut quoted = false;
    for offset in 0..path.len() {
        let Some(fields) = value_at(&path[..offset]).and_then(Value::as_record) else {
            continue;
        };
        let suffix = &path[offset..];
        let mut computed = BTreeMap::new();
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
                    let params: Vec<_> = params.values().filter_map(Value::as_cell).collect();
                    computed = super::unnamed_parameters(&params, value_of);
                    found.extend(params);
                }
            }
            // A closure's environment binds in its lambda.
            None if matches!(suffix.first(), Some(Step::Key(LAMBDA))) => {
                if let Some(environment) = fields.get(&ENVIRONMENT).and_then(Value::as_record) {
                    found.extend(environment.keys().copied());
                }
            }
            _ => (),
        }
        result.extend(found.into_iter().map(|cell| {
            (
                cell,
                Binding {
                    depth: offset,
                    computed: computed.remove(&cell),
                },
            )
        }));
    }
    (result, quoted)
}

#[cfg(test)]
mod tests;
