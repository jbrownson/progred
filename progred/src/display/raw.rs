//! Raw's offers: the editor's constructors, and what's typed read as a value.
//! Raw's holes list them, and a projection can offer them among its own, as
//! Grap's slots do.

use crate::display::widget::offers::{Entry, HoleOffers, entry, rank};
use crate::display::{Completion, CompletionKind, CompletionRequest, CompletionScope, Face};
use crate::libraries::{blob, name, text};
use gid::{Value, new_cell_id};
use std::rc::Rc;

/// Raw's offers: the editor's constructors, and what's typed read as a value,
/// or for a field's label a new cell named as typed. An empty query reads as
/// nothing; a projection that expects a value can offer one for it. Quoted
/// text and bytes lead; otherwise a reading that spells the query exactly
/// comes first, then the constructors the query names, then the other
/// readings and the text.
pub fn offers() -> HoleOffers {
    Rc::new(entries)
}

/// A document's empty root's offers: what the libraries start a document as,
/// then [`offers`].
pub fn start() -> HoleOffers {
    Rc::new(|request| {
        let start = CompletionRequest {
            scope: CompletionScope::Start,
            ..*request
        };
        let mut starts = request
            .libraries
            .and_then(|provider| provider(&start))
            .map(|offers| rank(offers, request))
            .unwrap_or_default();
        starts.extend(entries(request));
        starts
    })
}

/// The editor's constructors: a new cell, and as a value a new list or
/// record. Each opens for what goes inside it. A hole's offers include them
/// where its projection wants them, found by their delimiters too.
pub fn constructors(kind: CompletionKind) -> Vec<Completion> {
    keys(kind).into_iter().map(|(_, offer)| offer).collect()
}

/// The [`constructors`] beside their delimiters: `(` for the cell, `[` for
/// the list and `{` for the record. Raw's holes make one at once when its
/// delimiter is typed into an empty query.
pub fn keys(kind: CompletionKind) -> Vec<(&'static str, Completion)> {
    let cell = Completion::new("new cell", move |world| match kind {
        CompletionKind::Value => crate::editing::commit_container(world, new_cell_id().into()),
        CompletionKind::Field => crate::editing::commit_label(
            world,
            new_cell_id(),
            None,
            Some(crate::selection::pending_at(&[])),
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

/// What the libraries read the query as, such as a number. Text, which any
/// nonempty query reads as, comes after the readings particular to it.
pub fn typed(request: &CompletionRequest<'_>) -> Vec<Completion> {
    let (readings, strings) = literals(request);
    readings.into_iter().chain(strings).collect()
}

/// The readings particular to the query, and the query as text.
fn literals(request: &CompletionRequest<'_>) -> (Vec<Completion>, Vec<Completion>) {
    let literal = CompletionRequest {
        scope: CompletionScope::Literal,
        ..*request
    };
    let (strings, readings) = request
        .libraries
        .and_then(|provider| provider(&literal))
        .unwrap_or_default()
        .into_iter()
        .partition(|offer| {
            offer
                .preview
                .as_ref()
                .and_then(::grap::text::read)
                .is_some()
        });
    (readings, strings)
}

fn entries(request: &CompletionRequest<'_>) -> Vec<Entry<crate::Editor>> {
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
                        Some(crate::selection::pending_at(&[])),
                    )
                }),
            };
            (
                Vec::new(),
                (!trimmed.is_empty()).then_some(label).into_iter().collect(),
            )
        }
        CompletionKind::Value => {
            let (readings, strings) = literals(request);
            let entries = |offers: Vec<Completion>| {
                offers
                    .into_iter()
                    .map(|offer| entry(request, offer))
                    .collect::<Vec<_>>()
            };
            let atoms = blob
                .map(|blob| entry(request, blob::completion(blob)))
                .into_iter()
                .chain(entries(strings))
                .collect();
            (entries(readings), atoms)
        }
    };
    let constructors: Vec<_> = crate::filter::rank_with_aliases(
        constructors(kind)
            .into_iter()
            .map(|offer| {
                let aliases = offer.aliases.clone();
                (entry(request, offer), aliases)
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

#[cfg(test)]
mod tests;
