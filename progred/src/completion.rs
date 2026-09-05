//! Completion offers for pending value and label queries.

use crate::filter;
use crate::identity::short_id;
use crate::selection::parse_blob;
use crate::sources::Sources;
use gid::{CellId, Resolution, Value, new_cell_id};
use progred_display::{ActionHandler, CompletionProvider, Face};
use progred_libraries::{name, text};
use std::ops::Range;
use std::rc::Rc;

pub struct Entry<C> {
    pub display: String,
    pub detail: Option<String>,
    pub matches: Vec<Range<usize>>,
    pub face: Face,
    pub source: Option<CellId>,
    pub activate: ActionHandler<C>,
}

impl<C> Clone for Entry<C> {
    fn clone(&self) -> Self {
        Self {
            display: self.display.clone(),
            detail: self.detail.clone(),
            matches: self.matches.clone(),
            face: self.face,
            source: self.source,
            activate: self.activate.clone(),
        }
    }
}

/// The insertion capability supplied by the active completion site.
pub enum Commit<C> {
    Value(Rc<dyn Fn(&mut C, Value) -> bool>),
    Label(Rc<dyn Fn(&mut C, CellId, Option<Value>) -> bool>),
}

impl<C: 'static> Commit<C> {
    fn value(&self, value: Value) -> Option<ActionHandler<C>> {
        match self {
            Self::Value(commit) => {
                let commit = commit.clone();
                Some(Rc::new(move |world| commit(world, value.clone())))
            }
            Self::Label(commit) => value.as_cell().map(|cell| {
                let commit = commit.clone();
                Rc::new(move |world: &mut C| commit(world, cell, None)) as ActionHandler<C>
            }),
        }
    }

    fn new_cell(&self) -> ActionHandler<C> {
        match self {
            Self::Value(commit) => {
                let commit = commit.clone();
                Rc::new(move |world| commit(world, Value::from(new_cell_id())))
            }
            Self::Label(commit) => {
                let commit = commit.clone();
                Rc::new(move |world| commit(world, new_cell_id(), None))
            }
        }
    }
}

impl<C: 'static> Entry<C> {
    fn value(
        display: String,
        detail: Option<String>,
        value: Value,
        commit: &Commit<C>,
    ) -> Option<Self> {
        commit.value(value.clone()).map(|activate| Self {
            display,
            detail,
            matches: Vec::new(),
            face: if text::read(&value).is_some() {
                Face::String
            } else if value.as_blob().is_some() {
                Face::Id
            } else {
                Face::Label
            },
            source: value.as_cell(),
            activate,
        })
    }
}

/// Retained in the placed frame for attribution to the exact visible offers.
pub struct Offers<C> {
    pub entries: Vec<Entry<C>>,
}

pub(crate) fn completion_entries_with<C: 'static>(
    sources: &Sources,
    raw: bool,
    commit: &Commit<C>,
    query: &str,
    contextual: Option<&CompletionProvider>,
    everything: bool,
) -> Vec<Entry<C>> {
    if !everything && let Some(contextual) = contextual {
        return contextual_entries(contextual, query, commit);
    }
    let labels = matches!(commit, Commit::Label(_));
    let trimmed = query.trim();
    let quoted = trimmed.starts_with('"');
    let blob = (!labels).then(|| parse_blob(trimmed)).flatten();
    let spelling = trimmed
        .strip_prefix('"')
        .map(|inner| inner.strip_suffix('"').unwrap_or(inner))
        .unwrap_or(query);
    let atom = blob
        .as_ref()
        .map(|bytes| Value::from(bytes.clone()))
        .unwrap_or_else(|| text::value(spelling));
    let atom_leads = quoted || blob.is_some();
    let text_entry = blob
        .is_some()
        .then(|| Entry::value(format!("\"{query}\""), None, text::value(query), commit))
        .flatten();
    let atom_entry = match commit {
        Commit::Label(commit) => {
            let commit = commit.clone();
            let spelling = spelling.to_string();
            Entry {
                display: spelling.clone(),
                detail: Some("new label".to_string()),
                matches: Vec::new(),
                face: Face::Dim,
                source: None,
                activate: Rc::new(move |world| {
                    commit(world, new_cell_id(), Some(name::record(&spelling, [])))
                }),
            }
        }
        Commit::Value(_) => Entry::value(
            text::read(&atom)
                .map(|text| format!("\"{text}\""))
                .unwrap_or_else(|| atom.to_string()),
            None,
            atom,
            commit,
        )
        .unwrap(),
    };
    let (mut local, mut external): (Vec<_>, Vec<_>) = document_cells(sources)
        .into_iter()
        .flat_map(|cell| {
            let names: Vec<_> = (!raw)
                .then(|| {
                    sources
                        .values(cell)
                        .filter_map(|value| {
                            name::read(value.value).map(|name| (name.to_string(), value.source))
                        })
                        .collect()
                })
                .unwrap_or_default();
            if names.is_empty() {
                let sources_for_cell: Vec<_> = sources
                    .definitions(cell)
                    .map(|definition| source_name(sources, definition.source))
                    .collect();
                let mut entry = Entry::value(
                    short_id(cell),
                    (!sources_for_cell.is_empty()).then(|| sources_for_cell.join(" / ")),
                    Value::from(cell),
                    commit,
                )
                .unwrap();
                entry.face = Face::Id;
                vec![(entry, false, sources.external(cell))]
            } else {
                names
                    .into_iter()
                    .map(|(name, source)| {
                        (
                            Entry::value(
                                name,
                                Some(format!(
                                    "{} · {}",
                                    source_name(sources, source),
                                    short_id(cell)
                                )),
                                Value::from(cell),
                                commit,
                            )
                            .unwrap(),
                            true,
                            !matches!(source, Resolution::Document),
                        )
                    })
                    .collect()
            }
        })
        .partition(|(_, _, external)| !*external);
    local.sort_by(|a, b| a.0.display.cmp(&b.0.display));
    external.sort_by(|a, b| a.0.display.cmp(&b.0.display));
    let mut references_pool: Vec<_> = local
        .into_iter()
        .map(|(entry, named, _)| (entry, named))
        .collect();
    references_pool.push((
        Entry {
            display: "new cell".to_string(),
            detail: None,
            matches: Vec::new(),
            face: Face::Dim,
            source: None,
            activate: commit.new_cell(),
        },
        true,
    ));
    references_pool.extend(
        [
            ("new list", Value::list([])),
            ("new record", Value::record([])),
        ]
        .into_iter()
        .filter_map(|(display, value)| {
            Entry::value(display.to_string(), None, value, commit).map(|mut entry| {
                entry.face = Face::Dim;
                (entry, true)
            })
        }),
    );
    references_pool.extend(external.into_iter().map(|(entry, named, _)| (entry, named)));
    let references: Vec<_> = filter::rank(references_pool, |(entry, _)| &entry.display, query)
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
    let mut entries = contextual
        .map(|provider| contextual_entries(provider, query, commit))
        .unwrap_or_default();
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

fn contextual_entries<C: 'static>(
    provider: &CompletionProvider,
    query: &str,
    commit: &Commit<C>,
) -> Vec<Entry<C>> {
    let completions = provider(query);
    let keys: Vec<_> = completions
        .iter()
        .enumerate()
        .flat_map(|(index, completion)| {
            std::iter::once((index, completion.display.clone(), true)).chain(
                (!query.is_empty())
                    .then_some(completion.aliases.iter())
                    .into_iter()
                    .flatten()
                    .cloned()
                    .map(move |alias| (index, alias, false)),
            )
        })
        .collect();
    let mut seen = vec![false; completions.len()];
    filter::rank(keys, |(_, key, _)| key, query)
        .into_iter()
        .filter_map(|ranked| {
            let (index, _, display_matched) = ranked.item;
            if std::mem::replace(&mut seen[index], true) {
                None
            } else {
                let completion = &completions[index];
                Entry::value(
                    completion.display.clone(),
                    completion.detail.clone(),
                    completion.value.clone(),
                    commit,
                )
                .map(|entry| Entry {
                    matches: display_matched
                        .then_some(ranked.matches)
                        .unwrap_or_default(),
                    ..entry
                })
            }
        })
        .collect()
}

fn source_name(sources: &Sources<'_>, source: Resolution) -> String {
    match source {
        Resolution::Document => "document".to_string(),
        Resolution::Library(library) => sources
            .library_name(library)
            .map(str::to_string)
            .unwrap_or_else(|| short_id(library)),
    }
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
