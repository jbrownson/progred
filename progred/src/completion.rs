//! Completion offers for pending value and label queries.

use crate::display::{
    Completion, CompletionKind, CompletionProvider, CompletionRequest, CompletionScope,
    CompletionText, Face,
};
use crate::filter;
use crate::identity::short_id;
use crate::libraries::{blob, name, text};
use crate::site::Continuation;
use crate::sources::Sources;
use gid::{Value, new_cell_id};
use std::rc::Rc;

pub use crate::display::widget::offers::{Entry, Offers};

/// Whose offers a hole shows.
#[derive(Clone, Copy)]
pub(crate) enum HoleOffers<'a> {
    /// The drawing projection's own vocabulary, and nothing else.
    Projection(&'a CompletionProvider),
    /// Raw's: the editor's constructors, and what's typed read as a value.
    Raw,
    /// A document's empty root's: what the libraries start a document as,
    /// then Raw's.
    Start,
}

/// Labels can only be cells; an offer advertising another value is not one.
fn completion_entry(
    sources: &Sources,
    offer: Completion,
    kind: CompletionKind,
) -> Option<Entry<crate::Editor>> {
    (kind == CompletionKind::Value
        || offer
            .preview
            .as_ref()
            .is_none_or(|value| value.as_cell().is_some()))
    .then(|| entry(sources, offer))
}

fn entry(sources: &Sources, offer: Completion) -> Entry<crate::Editor> {
    Entry {
        display: completion_text(sources, &offer.display),
        detail: offer
            .detail
            .as_ref()
            .map(|detail| completion_text(sources, detail)),
        matches: Vec::new(),
        face: offer.face,
        source: offer.preview.as_ref().and_then(Value::as_cell),
        activate: offer.activate,
    }
}

/// The editor's constructors: a new cell, and as a value a new list or
/// record. Each opens for what goes inside it. A hole's offers include them
/// where its projection wants them, found by their delimiters too.
pub(crate) fn constructors(kind: CompletionKind) -> Vec<Completion> {
    delimited_constructors(kind)
        .into_iter()
        .map(|(_, offer)| offer)
        .collect()
}

/// The [`constructors`] beside their delimiters: `(` for the cell, `[` for
/// the list and `{` for the record. Raw's holes make one at once when its
/// delimiter is typed into an empty query.
pub(crate) fn delimited_constructors(kind: CompletionKind) -> Vec<(&'static str, Completion)> {
    let cell = Completion::new("new cell", move |world| match kind {
        CompletionKind::Value => crate::editing::commit_container(world, new_cell_id().into()),
        CompletionKind::Field => crate::editing::commit_label(
            world,
            new_cell_id(),
            None,
            Some(crate::libraries::selection::pending_at(&[])),
        ),
    });
    let containers = [
        ("[", "new list", Value::list([])),
        ("{", "new record", Value::record([])),
    ]
    .into_iter()
    .filter(|_| kind == CompletionKind::Value)
    .map(|(delimiter, display, value)| {
        let offer = Completion::new(display, move |world| {
            crate::editing::commit_container(world, value.clone())
        });
        (delimiter, offer)
    });
    std::iter::once(("(", cell))
        .chain(containers)
        .map(|(delimiter, offer)| {
            let offer = Completion {
                face: Face::Dim,
                ..offer.with_aliases([delimiter])
            };
            (delimiter, offer)
        })
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
    selection.writable(sources).then_some(())?;
    let mut document = std::rc::Rc::new(sources.doc.clone());
    let mut path = selection.path().to_vec();
    let document_changed = match selection.stage(sources) {
        Stage::Pending => {
            let defined = definition.zip(value.as_cell());
            selection::set_value(
                &mut document,
                sources.libraries,
                &selection.location()?,
                value,
            )
            .then_some(())?;
            if let Some((definition, cell)) = defined {
                std::rc::Rc::make_mut(&mut document)
                    .cells
                    .set_value(cell, definition);
            }
            true
        }
        Stage::Label => {
            let label = value.as_cell()?;
            path.push(gid::Step::Key(label));
            if selection.scope().read(sources, &path).is_some() {
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
    let selection_scope = selection.scope();
    let selection = Some((selection.path().to_vec(), selection.payload()));
    let mut effects = crate::site::PendingChanges {
        annotation,
        annotation_changed: false,
        selection,
        selection_changed: false,
    };
    if let Some(function) = on_commit {
        function(
            &selection_scope.view(Sources {
                doc: &document,
                libraries: sources.libraries,
            }),
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

/// A hole's offers: what the projection drawing it supplies and nothing else,
/// or Raw's, led at a document's empty root by the libraries' starting points.
pub(crate) fn completion_entries_with(
    sources: &Sources,
    request: &CompletionRequest<'_>,
    libraries: Option<&CompletionProvider>,
    hole: HoleOffers<'_>,
) -> Vec<Entry<crate::Editor>> {
    match hole {
        HoleOffers::Projection(projection) => {
            contextual_entries(sources, projection(request).unwrap_or_default(), request)
        }
        HoleOffers::Raw => raw_entries(sources, request, libraries),
        HoleOffers::Start => {
            let start = CompletionRequest {
                scope: CompletionScope::Start,
                ..*request
            };
            let mut entries = libraries
                .and_then(|provider| provider(&start))
                .map(|offers| contextual_entries(sources, offers, request))
                .unwrap_or_default();
            entries.extend(raw_entries(sources, request, libraries));
            entries
        }
    }
}

/// What the libraries read the query as, such as a number. Text, which any
/// nonempty query reads as, comes after the readings particular to it.
pub(crate) fn typed(
    libraries: Option<&CompletionProvider>,
    request: &CompletionRequest<'_>,
) -> Vec<Completion> {
    let (readings, strings) = literals(libraries, request);
    readings.into_iter().chain(strings).collect()
}

/// The readings particular to the query, and the query as text.
fn literals(
    libraries: Option<&CompletionProvider>,
    request: &CompletionRequest<'_>,
) -> (Vec<Completion>, Vec<Completion>) {
    let literal = CompletionRequest {
        scope: CompletionScope::Literal,
        ..*request
    };
    let (strings, readings) = libraries
        .and_then(|provider| provider(&literal))
        .unwrap_or_default()
        .into_iter()
        .partition(|offer| offer.preview.as_ref().and_then(text::read).is_some());
    (readings, strings)
}

/// Raw's offers: the editor's constructors, and what's typed read as a value,
/// or for a field's label a new cell named as typed. An empty query reads as
/// nothing; a projection that expects a value can offer one for it. Quoted
/// text and bytes
/// lead; otherwise a reading that spells the query exactly comes first, then
/// the constructors the query names, then the other readings and the text.
fn raw_entries(
    sources: &Sources,
    request: &CompletionRequest<'_>,
    libraries: Option<&CompletionProvider>,
) -> Vec<Entry<crate::Editor>> {
    let kind = request.kind;
    let query = request.query;
    let trimmed = query.trim();
    let blob = (kind == CompletionKind::Value)
        .then(|| blob::parse(trimmed))
        .flatten();
    let atom_leads = trimmed.starts_with('"') || blob.is_some();
    let (readings, atoms): (Vec<_>, Vec<_>) = match kind {
        CompletionKind::Field => {
            let spelling = text::query_spelling(query).to_string();
            let label = Entry {
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
            };
            (
                Vec::new(),
                (!trimmed.is_empty()).then_some(label).into_iter().collect(),
            )
        }
        CompletionKind::Value => {
            let (readings, strings) = literals(libraries, request);
            let entries = |offers: Vec<Completion>| {
                offers
                    .into_iter()
                    .map(|offer| entry(sources, offer))
                    .collect::<Vec<_>>()
            };
            let atoms = blob
                .map(|blob| entry(sources, blob::completion(blob)))
                .into_iter()
                .chain(entries(strings))
                .collect();
            (entries(readings), atoms)
        }
    };
    let constructors: Vec<_> = filter::rank_with_aliases(
        constructors(kind)
            .into_iter()
            .map(|offer| {
                let aliases = offer.aliases.clone();
                (entry(sources, offer), aliases)
            })
            .collect(),
        |(entry, _)| &entry.display,
        |(_, aliases)| aliases.as_slice(),
        query,
    )
    .into_iter()
    .map(|ranked| {
        let fuzzy = ranked.fuzzy();
        (
            Entry {
                matches: ranked.matches,
                ..ranked.item.0
            },
            fuzzy,
        )
    })
    .collect();
    if atom_leads {
        return atoms
            .into_iter()
            .chain(readings)
            .chain(constructors.into_iter().map(|(entry, _)| entry))
            .collect();
    }
    let (weak, strong): (Vec<_>, Vec<_>) = constructors.into_iter().partition(|(_, fuzzy)| *fuzzy);
    let (exact, other): (Vec<_>, Vec<_>) = readings
        .into_iter()
        .partition(|entry| !query.is_empty() && entry.display.eq_ignore_ascii_case(query));
    exact
        .into_iter()
        .chain(strong.into_iter().map(|(entry, _)| entry))
        .chain(other)
        .chain(atoms)
        .chain(weak.into_iter().map(|(entry, _)| entry))
        .collect()
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
                        .preview
                        .as_ref()
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
        let (_, completion) = ranked.item;
        completion_entry(sources, completion, request.kind).map(|entry| Entry {
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
