use super::*;
use crate::display::{CompletionRequest, CompletionScope};
use crate::libraries::{completion::select, control::vocabulary as control};
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

/// A slot's offers: the bindings in scope, the libraries' offers, Raw's
/// constructors, and what's typed read as a value.
pub(super) fn offers(request: &CompletionRequest<'_>) -> Option<Vec<Completion>> {
    let quoted = request.query.trim().starts_with('"');
    let templated = templated(request);
    let bindings = bindings(request);
    let mut ordered: Vec<_> = bindings.iter().filter(|_| !quoted).collect();
    ordered.sort_by_cached_key(|(cell, depth)| {
        let name = (request.resolve)(**cell)
            .and_then(|definition| name::read(definition.value))
            .unwrap_or("");
        (Reverse(**depth), name.to_lowercase(), name)
    });
    let mut offers: Vec<_> = templated.then(unquote).into_iter().collect();
    offers.extend(
        ordered
            .into_iter()
            .map(|(cell, _)| select(*cell, (*cell).into()).with_detail("binding")),
    );
    if !quoted {
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
    let source = (request.resolve)(control::QUOTE)?.source;
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

/// A quote's expression is a template: data, apart from its unquotes.
fn templated(request: &CompletionRequest<'_>) -> bool {
    let mut quoted = false;
    for offset in 0..request.path.len() {
        let Some(fields) = (request.value_at)(&request.path[..offset]).and_then(Value::as_record)
        else {
            continue;
        };
        let next = request.path.get(offset);
        quoted = if quoted {
            !matches!(next, Some(Step::Key(field)) if *field == control::UNQUOTE)
        } else {
            fields.get(&FUNCTION).and_then(Value::as_cell) == Some(control::QUOTE)
                && matches!(next, Some(Step::Key(field)) if *field == ::grap::vocabulary::EXPRESSION)
        };
    }
    quoted
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
mod tests;
