//! Completion offers for pending value and label queries.

use crate::document::{Document, short_id};
use crate::filter;
use crate::selection::{parse_blob, set_value};
use crate::sources::Sources;
use progred_graph::{CellId, Cells, Step, Value, new_cell_id};
use vello::kurbo::Rect;

/// A completion offer on a pending. The display styles itself by the
/// action's kind at draw time.
#[derive(Clone)]
pub struct Entry {
    pub display: String,
    pub detail: Option<String>,
    /// Byte spans of `display` the query matched, for highlighting.
    pub matches: Vec<filter::Match>,
    /// The display spells a bare short id — an unnamed cell — so it
    /// draws in the id face, as ids do everywhere.
    pub id: bool,
    pub action: EntryAction,
}

#[derive(Clone)]
pub enum EntryAction {
    /// Commit this value: an inferred atom or a reference.
    Value(Value),
    /// Mint a cell named by this text and use its identity as a label.
    NewLabel(String),
    /// Mint a bare cell and commit a link to it.
    NewCell,
    /// Commit an empty list value.
    NewList,
    /// Commit an empty inline record value — anonymous structure, no
    /// cell minted.
    NewRecord,
}

/// The completion popup a pending row emits during placement; the
/// shell draws it after the body and commits from it. Recomputed
/// every frame like everything else.
pub struct Popup {
    pub anchor: Rect,
    pub entries: Vec<Entry>,
    pub choice: usize,
}

/// Placement contexts that carry the frame's popup.
pub trait HasPopup {
    fn popup(&mut self) -> &mut Option<Popup>;
}

/// The universal completion layer for `query`: the inferred value,
/// references to everything named (document and orphans alike, ranked
/// by the fuzzy tiers), and a fresh bare cell. The label stage
/// (`labels`) offers cell references plus a freshly minted cell named
/// by the query — no blobs, and "new list"/"new record" stay value
/// offers.
pub(crate) fn completion_entries(
    sources: &Sources,
    raw: bool,
    labels: bool,
    query: &str,
) -> Vec<Entry> {
    let trimmed = query.trim();
    let quoted = trimmed.trim_start().starts_with('"');
    let blob = (!labels).then(|| parse_blob(trimmed)).flatten();
    let text = trimmed
        .strip_prefix('"')
        .map(|inner| inner.strip_suffix('"').unwrap_or(inner))
        .unwrap_or(query);
    let atom = blob
        .as_ref()
        .map(|bytes| Value::from(bytes.clone()))
        .unwrap_or_else(|| progred_text::value(text));
    // Quotes and `0x` state atom intent, so the atom leads; otherwise
    // a confident (non-fuzzy) NAMED match is likelier the intent than
    // a new literal — typing a visible name should default to the
    // reference, quoting always forces text, and bare ids never
    // outrank the typed text.
    let atom_leads = quoted || blob.is_some();
    // The typed text is always insertable as itself: a blob query
    // offers its text form right below the blob (a quote already
    // states text intent, so quoted queries stay text-only).
    let text_entry = blob.is_some().then(|| Entry {
        display: format!("\"{query}\""),
        detail: None,
        matches: Vec::new(),
        id: false,
        action: EntryAction::Value(progred_text::value(query)),
    });
    let atom_entry = Entry {
        display: if labels {
            text.to_string()
        } else {
            progred_text::read(&atom)
                .map(|text| format!("\"{text}\""))
                .unwrap_or_else(|| atom.to_string())
        },
        detail: labels.then(|| "new label".to_string()),
        matches: Vec::new(),
        id: false,
        action: if labels {
            EntryAction::NewLabel(text.to_string())
        } else {
            EntryAction::Value(atom)
        },
    };
    // Every cell the document contains is referenceable: named ones
    // by name, unnamed ones by the short id they render as — what
    // you see is what you can type. Unnamed keys start with the
    // ellipsis, which sorts after names, so they trail on an empty
    // query. "new list" and "new record" rank among them under their
    // own display text: type toward one and it surfaces, type away
    // and it leaves.
    let (local, external): (Vec<_>, Vec<_>) = document_cells(sources)
        .into_iter()
        .map(
            |cell| match crate::conventions::display_name(sources, raw, cell) {
                Some(name) => (
                    (name.to_string(), true, EntryAction::Value(Value::from(cell))),
                    sources.external(cell),
                ),
                None => (
                    (short_id(cell), false, EntryAction::Value(Value::from(cell))),
                    sources.external(cell),
                ),
            },
        )
        .partition(|(_, external)| !*external);
    let strip_origin = |((display, named, action), _)| (display, named, action);
    let mut local: Vec<_> = local.into_iter().map(strip_origin).collect();
    let mut external: Vec<_> = external.into_iter().map(strip_origin).collect();
    local.sort_by(|a, b| a.0.cmp(&b.0));
    external.sort_by(|a, b| a.0.cmp(&b.0));
    let mut references_pool = local;
    // Constructors follow the current document on an empty query;
    // library vocabulary follows them. A non-empty query still ranks
    // all three groups by the ordinary matching tiers.
    references_pool.push(("new cell".to_string(), true, EntryAction::NewCell));
    if !labels {
        references_pool.push(("new list".to_string(), true, EntryAction::NewList));
        references_pool.push(("new record".to_string(), true, EntryAction::NewRecord));
    }
    references_pool.extend(external);
    let references: Vec<(Entry, bool)> = filter::rank(references_pool, |(key, _, _)| key, query)
        .into_iter()
        .take(8)
        .map(|ranked| {
            // A DEMOTED reference ranks after the typed atom: fuzzy,
            // or an unnamed cell's bare id — ids are for reading,
            // names are for reaching (want it reachable? name it).
            let fuzzy = ranked.fuzzy();
            let matches = ranked.matches;
            let (display, named, action) = ranked.item;
            let demoted = fuzzy || !named;
            let detail = match &action {
                EntryAction::Value(value) => value
                    .as_cell()
                    .map(short_id)
                    .filter(|detail| *detail != display),
                _ => None,
            };
            let entry = Entry {
                display,
                detail,
                matches,
                id: !named,
                action,
            };
            (entry, demoted)
        })
        .collect();
    let mut entries = Vec::new();
    if atom_leads {
        entries.push(atom_entry);
        entries.extend(text_entry);
        entries.extend(references.into_iter().map(|(entry, _)| entry));
    } else {
        let (weak, strong): (Vec<_>, Vec<_>) =
            references.into_iter().partition(|(_, demoted)| *demoted);
        entries.extend(strong.into_iter().map(|(entry, _)| entry));
        entries.push(atom_entry);
        entries.extend(weak.into_iter().map(|(entry, _)| entry));
    }
    entries
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
        cells.push(*cell);
        if let Some(value) = sources.value(*cell) {
            value_cells(value, &mut cells);
        }
    }
    if let Some(root) = sources.root() {
        value_cells(root, &mut cells);
    }
    cells.sort();
    cells.dedup();
    cells
}

/// Resolves a value-stage entry to the value it denotes. Pure: a new
/// cell's mint is a bare id — nothing said until a value is written.
/// Labels and values resolve alike — the label stage never
/// offers a non-label action.
pub fn resolve_entry(action: &EntryAction) -> Option<Value> {
    match action {
        EntryAction::Value(value) => Some(value.clone()),
        EntryAction::NewLabel(_) => None,
        EntryAction::NewCell => Some(Value::from(new_cell_id())),
        EntryAction::NewList => Some(Value::list([])),
        EntryAction::NewRecord => Some(Value::record([])),
    }
}

/// Resolves a label-stage entry. Free text becomes a newly minted
/// named cell; an existing cell value reuses its identity. The
/// optional cell value is what the caller must add to the document.
pub fn resolve_label(action: &EntryAction) -> Option<(CellId, Option<(CellId, Value)>)> {
    match action {
        EntryAction::Value(value) => value.as_cell().map(|cell| (cell, None)),
        EntryAction::NewLabel(name) => {
            let cell = new_cell_id();
            Some((cell, Some((cell, progred_name::record(name, [])))))
        }
        EntryAction::NewCell => Some((new_cell_id(), None)),
        EntryAction::NewList | EntryAction::NewRecord => None,
    }
}

/// Commits a pending from a chosen entry: resolves the action to a
/// value and writes it.
pub fn commit_pending(
    doc: &mut Document,
    library: &Cells,
    path: &[Step],
    action: &EntryAction,
) -> bool {
    resolve_entry(action).is_some_and(|value| set_value(doc, library, path, value))
}
