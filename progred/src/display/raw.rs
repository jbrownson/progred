//! Raw: GID's own kinds drawn as they are, and Raw's offers, the editor's
//! constructors and what's typed read as a value. Raw's holes list the
//! offers, and a projection can offer them among its own, as Grap's slots do.

use crate::display::fold::{cell_starts_closed, foldable};
use crate::display::host::{Layout, ProjectionInput};
use crate::display::widget::offers::{Entry, HoleOffers, entry, fitting, rank};
use crate::display::widget::picker::Picker;
use crate::display::{
    Completion, CompletionKind, CompletionRequest, CompletionScope, Delim, Face, Host,
};
use gid::{Value, new_cell_id};
use std::rc::Rc;

/// Raw's view of the value at `input`, GID's own kinds and nothing else: a
/// blob reads as its hex, edited in place, a cell as its value followed
/// between parentheses, and lists and records as their structure. Cells,
/// lists and records fold; lists and records start open, and cells as
/// [`cell_starts_closed`] says.
pub fn view<W: Host>(input: &ProjectionInput<'_, W, ::grap::RuntimeValue>) -> Option<Layout<W>> {
    let value = input.value?;
    Some(match value.shape() {
        ::grap::Shape::Blob(bytes)
            if bytes.len() <= SHORT_BLOB
                || input.selection.is_some() && bytes.len() <= EDITABLE_BLOB =>
        {
            crate::display::line_edit(hex_editor(bytes))
        }
        ::grap::Shape::Blob(bytes) => crate::display::activatable(
            crate::display::id(format!(
                "0x{}… ({} bytes)",
                gid::hex_string(&bytes[..8]),
                bytes.len()
            )),
            &input.site(),
        ),
        ::grap::Shape::Cell(cell) => cell_view(input, cell),
        ::grap::Shape::List(positions) => {
            let empty = positions.is_empty();
            let open = crate::display::structure::list_layout_at(input, positions, None);
            if empty {
                open
            } else {
                foldable(Delim::Bracket, None, |_| false, open)
            }
        }
        ::grap::Shape::Record(keys) => {
            let empty = keys.is_empty();
            let open = crate::display::structure::Fields::default().record(input)?;
            if empty {
                open
            } else {
                foldable(Delim::Brace, None, |_| false, open)
            }
        }
    })
}

/// The most bytes a blob shows whole; a longer one shows its first eight
/// bytes and its length until it's selected.
const SHORT_BLOB: usize = 16;

/// The most bytes a selected blob opens as hex to edit; a longer one is
/// replaced instead, by typing a new value over it.
const EDITABLE_BLOB: usize = 1024;

/// A blob's bytes, edited as hex. What's typed replaces them only once
/// it reads as whole bytes.
fn hex_editor(bytes: &[u8]) -> crate::display::LineEdit {
    crate::display::LineEdit {
        text: gid::hex_string(bytes),
        update: Rc::new(|spelling: &str, _: Option<&Value>| {
            gid::read_hex(spelling.trim()).map(Value::from)
        }),
        prefix: "0x".into(),
        suffix: String::new(),
        family: crate::display::TextFamily::Monospace,
        placeholder: None,
    }
}

/// A cell as its value, followed where it resolves, between parentheses;
/// closed, as its name.
fn cell_view<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    cell: gid::CellId,
) -> Layout<W> {
    let resolved = input.env.resolve(cell);
    let open = crate::display::structure::authoring(crate::display::selectable_bracket(
        Delim::Paren,
        crate::display::descend(
            [gid::Step::Follow(
                resolved
                    .as_ref()
                    .map_or(gid::Resolution::Document, |definition| {
                        definition.resolution
                    }),
            )],
            crate::display::Projected::ordinary(),
        ),
    ));
    let Some(definition) = resolved else {
        return open;
    };
    let value = definition.value.clone();
    let name = match input.env.name(cell) {
        Some(name) => crate::display::named(&name, Face::Name),
        None => crate::display::faced(crate::identity::short_id(cell), Face::Id),
    };
    foldable(
        Delim::Paren,
        Some(name),
        move |viewing| cell_starts_closed(viewing, cell, &value),
        open,
    )
}

/// Raw's picker for a hole of `kind`, its brackets making its constructors.
pub fn picker<W: Host>(kind: CompletionKind) -> Picker<W> {
    Picker::new(kind, Rc::new(entries)).with_keys(keys(kind))
}

/// The picker of a document's empty root: Raw's, led by what the libraries
/// start a document as.
pub fn start_picker<W: Host>() -> Picker<W> {
    Picker::new(CompletionKind::Value, start()).with_keys(keys(CompletionKind::Value))
}

/// What the libraries start a document as, then Raw's [`entries`].
fn start<W: Host>() -> HoleOffers<W> {
    Rc::new(|request| {
        let start = CompletionRequest {
            scope: CompletionScope::Start,
            ..*request
        };
        request
            .libraries
            .and_then(|provider| provider(&start))
            .map(|offers| rank(offers, request))
            .unwrap_or_default()
            .into_iter()
            .chain(entries(request))
            .collect()
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
        if let Some(picking) = session.pending() {
            match kind {
                CompletionKind::Value => picking.commit_container(new_cell_id().into()),
                CompletionKind::Field => picking.commit_label(
                    new_cell_id(),
                    None,
                    Some(crate::selection::pending_at(&[])),
                ),
            };
        }
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

/// Raw's offers: the editor's constructors, and what the libraries read the
/// query as, a value or a field's label, such as the name library's new cell
/// named as typed. An empty query reads as nothing; a projection that expects
/// a value can offer one for it. Quoted text and bytes lead; otherwise a
/// reading that spells the query exactly comes first, then the constructors
/// the query names, then the other readings and the text or new label.
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
    let constructors = ranked_constructors(request);
    if atom_leads {
        atoms
            .into_iter()
            .chain(readings)
            .chain(constructors.into_iter().map(|(entry, _)| entry))
            .collect()
    } else {
        let (weak, strong): (Vec<_>, Vec<_>) =
            constructors.into_iter().partition(|(_, fuzzy)| *fuzzy);
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
}

/// The [`constructors`] the query names, best first, each with whether it
/// matched only fuzzily.
fn ranked_constructors<W: Host>(request: &CompletionRequest<'_>) -> Vec<(Entry<W>, bool)> {
    crate::filter::rank_with_aliases(
        constructors(request.kind)
            .into_iter()
            .map(|offer| {
                let aliases = offer.aliases.clone();
                (entry(request, offer), aliases)
            })
            .collect(),
        |(entry, _)| &entry.display,
        |(_, aliases)| aliases.as_slice(),
        request.query,
    )
    .into_iter()
    .map(|ranked| {
        (
            Entry {
                matches: ranked.matches,
                ..ranked.item.0
            },
            ranked.fuzzy,
        )
    })
    .collect()
}

#[cfg(test)]
mod tests;
