//! Completion offers for pending value and label queries.

use crate::display::{
    Completion, CompletionKind, CompletionProvider, CompletionRequest, CompletionScope,
    CompletionText, CompletionValue, Face,
};
use crate::filter;
use crate::identity::short_id;
use crate::libraries::{blob, name, text};
use crate::site::Continuation;
use crate::sources::Sources;
use gid::{CellId, Resolution, Value, new_cell_id};
use std::rc::Rc;

pub use crate::display::widget::offers::{Entry, Offers};

fn activation(
    kind: CompletionKind,
    value: CompletionValue,
    on_commit: Option<Continuation>,
) -> Option<Rc<dyn Fn(&mut crate::Editor)>> {
    match kind {
        CompletionKind::Value => Some(Rc::new(move |world| {
            crate::editing::commit_value(world, value.instantiate(), on_commit.clone())
        })),
        CompletionKind::Field => value.literal().and_then(Value::as_cell).map(|cell| {
            Rc::new(move |world: &mut crate::Editor| {
                crate::editing::commit_label(world, cell, None, on_commit.clone())
            }) as Rc<dyn Fn(&mut crate::Editor)>
        }),
    }
}

fn new_cell(kind: CompletionKind) -> Rc<dyn Fn(&mut crate::Editor)> {
    Rc::new(move |world| match kind {
        CompletionKind::Value => crate::editing::commit_value(
            world,
            new_cell_id().into(),
            Some(crate::libraries::selection::at(
                &[],
                crate::libraries::selection::edge(),
            )),
        ),
        CompletionKind::Field => crate::editing::commit_label(
            world,
            new_cell_id(),
            None,
            Some(crate::libraries::selection::pending_at(&[])),
        ),
    })
}

fn completion_entry(
    sources: &Sources,
    offer: Completion,
    kind: CompletionKind,
) -> Option<Entry<crate::Editor>> {
    offered_entry(
        completion_text(sources, &offer.display),
        offer
            .detail
            .as_ref()
            .map(|detail| completion_text(sources, detail)),
        offer.value,
        offer.on_commit,
        kind,
    )
}

fn value_entry(
    display: String,
    detail: Option<String>,
    value: Value,
    on_commit: Continuation,
    kind: CompletionKind,
) -> Option<Entry<crate::Editor>> {
    offered_entry(
        display,
        detail,
        CompletionValue::Literal(value),
        Some(on_commit),
        kind,
    )
}

fn offered_entry(
    display: String,
    detail: Option<String>,
    value: CompletionValue,
    on_commit: Option<Continuation>,
    kind: CompletionKind,
) -> Option<Entry<crate::Editor>> {
    activation(kind, value.clone(), on_commit).map(|activate| Entry {
        display,
        detail,
        matches: Vec::new(),
        face: if value.literal().and_then(text::read).is_some() {
            Face::String
        } else if value.literal().and_then(Value::as_blob).is_some() {
            Face::Id
        } else {
            Face::Label
        },
        source: value.literal().and_then(Value::as_cell),
        activate,
    })
}

pub(crate) fn constructor_entries(
    kind: CompletionKind,
) -> Vec<(&'static str, Entry<crate::Editor>)> {
    std::iter::once((
        "(",
        Entry {
            display: "new cell".to_string(),
            detail: None,
            matches: Vec::new(),
            face: Face::Dim,
            source: None,
            activate: new_cell(kind),
        },
    ))
    .chain(
        [
            ("[", "new list", Value::list([])),
            ("{", "new record", Value::record([])),
        ]
        .into_iter()
        .filter_map(|(key, display, value)| {
            value_entry(
                display.to_string(),
                None,
                value,
                crate::libraries::selection::at(&[], crate::libraries::selection::edge()),
                kind,
            )
            .map(|mut entry| {
                entry.face = Face::Dim;
                (key, entry)
            })
        }),
    )
    .collect()
}

pub(crate) struct Prepared {
    pub document: std::rc::Rc<gid::Document>,
    pub document_changed: bool,
    pub path: gid::Path,
    pub effects: crate::site::PendingChanges,
}

/// Prepare the insertion and interpret its continuation against the
/// resulting document. The caller installs everything only on success.
pub(crate) fn prepare(
    sources: &Sources,
    selection: &crate::selection::Selection,
    annotations: &crate::annotations::Annotations,
    value: Value,
    definition: Option<Value>,
    on_commit: Option<&Continuation>,
) -> Option<Prepared> {
    use crate::selection::{self, Stage};
    let mut document = std::rc::Rc::new(sources.doc.clone());
    let mut path = selection.path().to_vec();
    let document_changed = match selection.stage(sources) {
        Stage::Pending => {
            selection::set_value(&mut document, sources.libraries, &path, value).then_some(true)?
        }
        Stage::Label => {
            let label = value.as_cell()?;
            path.push(gid::Step::Key(label));
            if sources.resolve_path(&path).is_some() {
                false
            } else {
                let changed = definition.is_some();
                if let Some(value) = definition {
                    std::rc::Rc::make_mut(&mut document)
                        .cells
                        .set_value(label, value);
                }
                changed
            }
        }
        Stage::Edge => return None,
    };
    let annotation = annotations.at(&path).cloned();
    let selection = Some((selection.path().to_vec(), selection.payload()));
    let mut effects = crate::site::PendingChanges {
        annotation,
        annotation_changed: false,
        selection,
        selection_changed: false,
    };
    if let Some(function) = on_commit {
        function(
            &Sources {
                doc: &document,
                libraries: sources.libraries,
            },
            &path,
            &mut effects,
        )
        .then_some(())?;
    }
    Some(Prepared {
        document,
        document_changed,
        path,
        effects,
    })
}

pub(crate) fn completion_entries_with(
    sources: &Sources,
    raw: bool,
    request: &CompletionRequest<'_>,
    providers: Option<&CompletionProvider>,
    contextual: Option<&CompletionProvider>,
) -> (Vec<Entry<crate::Editor>>, bool) {
    let kind = request.kind;
    let narrow = CompletionRequest {
        scope: CompletionScope::Suggested,
        ..*request
    };
    let suggested = (!raw)
        .then(|| {
            contextual
                .and_then(|provider| provider(&narrow))
                .or_else(|| providers.and_then(|provider| provider(&narrow)))
        })
        .flatten();
    if request.scope == CompletionScope::Suggested
        && let Some(offers) = suggested
    {
        return (contextual_entries(sources, offers, request), false);
    }
    let query = request.query;
    let labels = kind == CompletionKind::Field;
    let trimmed = query.trim();
    let quoted = trimmed.starts_with('"');
    let universal = CompletionRequest {
        scope: CompletionScope::Everything,
        ..*request
    };
    let value_entries = providers
        .filter(|_| !quoted)
        .and_then(|provider| provider(&universal))
        .map(|offers| contextual_entries(sources, offers, request))
        .unwrap_or_default();
    let blob = (!labels).then(|| blob::parse(trimmed)).flatten();
    let spelling = text::query_spelling(query);
    let atom_leads = quoted || blob.is_some();
    let text_entry = blob
        .is_some()
        .then(|| completion_entry(sources, text::completion(query), kind))
        .flatten();
    let atom_entry = match kind {
        CompletionKind::Field => {
            let spelling = spelling.to_string();
            Entry {
                display: spelling.clone(),
                detail: Some("new label".to_string()),
                matches: Vec::new(),
                face: Face::Dim,
                source: None,
                activate: Rc::new(move |world| {
                    crate::editing::commit_label(
                        world,
                        new_cell_id(),
                        Some(name::record(&spelling, [])),
                        Some(crate::libraries::selection::pending_at(&[])),
                    )
                }),
            }
        }
        CompletionKind::Value => completion_entry(
            sources,
            blob.map(blob::completion)
                .unwrap_or_else(|| text::completion(spelling)),
            kind,
        )
        .unwrap(),
    };
    let reference_selection = if labels {
        crate::libraries::selection::pending_at(&[])
    } else {
        crate::libraries::selection::at(&[], crate::libraries::selection::edge())
    };
    let (mut local, mut external): (Vec<_>, Vec<_>) = document_cells(sources)
        .into_iter()
        .filter(|cell| {
            !labels
                || !request
                    .value()
                    .and_then(Value::as_record)
                    .is_some_and(|fields| fields.contains_key(cell))
        })
        .map(|cell| {
            let definition = sources.resolve(cell);
            let name = (!raw)
                .then(|| definition.and_then(|value| name::read(value.value)))
                .flatten();
            let source = definition
                .map(|value| value.source)
                .or_else(|| sources.contributors(cell).next());
            let mut entry = value_entry(
                name.map(str::to_owned).unwrap_or_else(|| short_id(cell)),
                source.map(|source| source_name(sources, source)),
                Value::from(cell),
                reference_selection.clone(),
                kind,
            )
            .unwrap();
            if name.is_none() {
                entry.face = Face::Id;
            }
            (entry, name.is_some(), sources.external(cell))
        })
        .partition(|(_, _, external)| !*external);
    local.sort_by(|a, b| a.0.display.cmp(&b.0.display));
    external.sort_by(|a, b| a.0.display.cmp(&b.0.display));
    let mut references_pool: Vec<_> = local
        .into_iter()
        .map(|(entry, named, _)| (entry, named, None))
        .collect();
    references_pool.extend(
        constructor_entries(kind)
            .into_iter()
            .map(|(key, entry)| (entry, true, Some(key))),
    );
    references_pool.extend(
        external
            .into_iter()
            .map(|(entry, named, _)| (entry, named, None)),
    );
    let references: Vec<_> = filter::rank_with_aliases(
        references_pool,
        |(entry, _, _)| &entry.display,
        |(_, _, alias)| alias.as_slice(),
        query,
    )
    .into_iter()
    .map(|ranked| {
        let demoted = ranked.fuzzy() || !ranked.item.1;
        (
            Entry {
                matches: ranked.matches,
                ..ranked.item.0
            },
            demoted,
        )
    })
    .collect();
    let mut entries = suggested
        .map(|offers| contextual_entries(sources, offers, request))
        .unwrap_or_default();
    if atom_leads {
        entries.push(atom_entry);
        entries.extend(text_entry);
        entries.extend(value_entries);
        entries.extend(references.into_iter().map(|(entry, _)| entry));
    } else {
        let (weak, strong): (Vec<_>, Vec<_>) =
            references.into_iter().partition(|(_, demoted)| *demoted);
        entries.extend(strong.into_iter().map(|(entry, _)| entry));
        entries.extend(value_entries);
        entries.push(atom_entry);
        entries.extend(weak.into_iter().map(|(entry, _)| entry));
    }
    (entries, true)
}

fn contextual_entries(
    sources: &Sources,
    offers: Vec<Completion>,
    request: &CompletionRequest<'_>,
) -> Vec<Entry<crate::Editor>> {
    filter::rank_with_aliases(
        offers
            .into_iter()
            .filter(|offer| {
                request.kind != CompletionKind::Field
                    || !offer
                        .value
                        .literal()
                        .and_then(Value::as_cell)
                        .is_some_and(|cell| {
                            request
                                .value()
                                .and_then(Value::as_record)
                                .is_some_and(|fields| fields.contains_key(&cell))
                        })
            })
            .map(|offer| (completion_text(sources, &offer.display), offer))
            .collect(),
        |(display, _)| display,
        |(_, completion)| &completion.aliases,
        request.query,
    )
    .into_iter()
    .filter_map(|ranked| {
        let (display, completion) = ranked.item;
        offered_entry(
            display,
            completion
                .detail
                .as_ref()
                .map(|detail| completion_text(sources, detail)),
            completion.value,
            completion.on_commit,
            request.kind,
        )
        .map(|entry| Entry {
            matches: ranked.matches,
            ..entry
        })
    })
    .collect()
}

fn completion_text(sources: &Sources<'_>, text: &CompletionText) -> String {
    match text {
        CompletionText::Literal(text) => text.clone(),
        CompletionText::Name(cell) => sources
            .name(*cell)
            .map(str::to_owned)
            .unwrap_or_else(|| short_id(*cell)),
    }
}

fn source_name(sources: &Sources<'_>, source: Resolution) -> String {
    completion_text(
        sources,
        &CompletionText::Name(match source {
            Resolution::Document => crate::libraries::path::vocabulary::DOCUMENT,
            Resolution::Library(library) => library,
        }),
    )
}

/// The cells a value links, walked structurally — lists and records
/// are values, so their contents are right here; record labels
/// reference too.
fn value_cells(value: &Value, cells: &mut Vec<CellId>) {
    match value {
        Value::Cell(cell) => cells.push(*cell),
        Value::Blob(_) => {}
        Value::List(elements) => {
            for element in elements.values() {
                value_cells(element, cells);
            }
        }
        Value::Record(fields) => {
            for (label, field) in fields {
                cells.push(*label);
                value_cells(field, cells);
            }
        }
    }
}

/// Every cell the document or its library mentions — table entries,
/// links inside values, cells used as labels, and the root's own
/// links. Bare cells referenced anywhere are included: still
/// referenceable. Library cells are offered so the conventions are
/// typeable from keystroke one. Sorted for a deterministic offer
/// order.
fn document_cells(sources: &Sources) -> Vec<CellId> {
    let mut cells = Vec::new();
    for cell in sources.cells() {
        cells.push(cell);
        for value in sources.values(cell) {
            value_cells(value.value, &mut cells);
        }
    }
    if let Some(root) = sources.root() {
        value_cells(root, &mut cells);
    }
    cells.sort();
    cells.dedup();
    cells
}
