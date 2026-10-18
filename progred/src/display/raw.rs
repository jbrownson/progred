//! Raw's offers: the editor's constructors, and what's typed read as a value.
//! Raw's holes list them, and a projection can offer them among its own, as
//! Grap's slots do.

use crate::display::widget::offers::{Entry, HoleOffers, entry, fitting, rank};
use crate::display::widget::picker::Picker;
use crate::display::{Completion, CompletionKind, CompletionRequest, CompletionScope, Face, Host};
use gid::{Value, new_cell_id};
use std::rc::Rc;

/// Raw's picker for a hole of `kind`, its brackets making its constructors.
pub fn picker<W: Host>(kind: CompletionKind) -> Picker<W> {
    Picker::new(kind, offers()).with_keys(keys(kind))
}

/// The picker of a document's empty root: Raw's, led by what the libraries
/// start a document as.
pub fn start_picker<W: Host>() -> Picker<W> {
    Picker::new(CompletionKind::Value, start()).with_keys(keys(CompletionKind::Value))
}

/// Raw's offers: the editor's constructors, and what the libraries read the
/// query as, a value or a field's label, such as the name library's new cell
/// named as typed. An empty query reads as nothing; a projection that expects
/// a value can offer one for it. Quoted text and bytes lead; otherwise a
/// reading that spells the query exactly comes first, then the constructors
/// the query names, then the other readings and the text or new label.
fn offers<W: Host>() -> HoleOffers<W> {
    Rc::new(entries)
}

/// What the libraries start a document as, then [`offers`].
fn start<W: Host>() -> HoleOffers<W> {
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
fn keys(kind: CompletionKind) -> Vec<(&'static str, Completion)> {
    let cell = Completion::new("new cell", move |session| {
        let Some(picking) = session.pending() else {
            return;
        };
        match kind {
            CompletionKind::Value => picking.commit_container(new_cell_id().into()),
            CompletionKind::Field => {
                picking.commit_label(new_cell_id(), None, Some(crate::selection::pending_at(&[])))
            }
        };
    });
    let containers = [
        ("[", "new list", Value::list([])),
        ("{", "new record", Value::record([])),
    ]
    .into_iter()
    .filter(|_| kind == CompletionKind::Value)
    .map(|(delimiter, display, value)| {
        let offer = Completion::new(display, move |session| {
            if let Some(picking) = session.pending() {
                picking.commit_container(value.clone());
            }
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

/// The blob typed as `0x` and its hex, inserted and selected.
fn typed_blob(bytes: Vec<u8>) -> Completion {
    let value = Value::from(bytes);
    let committed = value.clone();
    Completion {
        face: Face::Id,
        preview: Some(value.clone()),
        ..Completion::new(value.to_string(), move |session| {
            if let Some(picking) = session.pending() {
                picking.commit(
                    committed.clone(),
                    Some(crate::selection::at(&[], crate::selection::payload::edge())),
                );
            }
        })
    }
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

fn entries<W: Host>(request: &CompletionRequest<'_>) -> Vec<Entry<W>> {
    let kind = request.kind;
    let query = request.query;
    let trimmed = query.trim();
    let blob = (kind == CompletionKind::Value)
        .then(|| trimmed.strip_prefix("0x").and_then(gid::read_hex))
        .flatten();
    let atom_leads = trimmed.starts_with('"') || blob.is_some();
    let (readings, atoms): (Vec<_>, Vec<_>) = match kind {
        // A new label is what's typed taken as a label, so it goes where
        // text does.
        CompletionKind::Field => {
            let (readings, _) = literals(request);
            let labels = readings
                .into_iter()
                .filter_map(|offer| fitting(request, offer))
                .collect();
            (Vec::new(), labels)
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
                .map(|blob| entry(request, typed_blob(blob)))
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
