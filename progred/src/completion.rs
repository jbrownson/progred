//! Completion offers for pending value and label queries.

use crate::filter;
use crate::identity::short_id;
use crate::sources::Sources;
use gid::{CellId, Resolution, Value, new_cell_id};
use progred_display::{CompletionProvider, CompletionValue, Face};
use progred_libraries::{blob, name, text};
use std::ops::Range;
use std::rc::Rc;

pub struct Entry<C> {
    pub display: String,
    pub detail: Option<String>,
    pub matches: Vec<Range<usize>>,
    pub face: Face,
    pub source: Option<CellId>,
    pub activate: Rc<dyn Fn(&mut C)>,
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
    Value(Rc<dyn Fn(&mut C, Value, Option<Value>)>),
    Label(Rc<dyn Fn(&mut C, CellId, Option<Value>, Option<Value>)>),
}

impl<C: 'static> Commit<C> {
    fn value(
        &self,
        value: CompletionValue,
        on_commit: Option<Value>,
    ) -> Option<Rc<dyn Fn(&mut C)>> {
        match self {
            Self::Value(commit) => {
                let commit = commit.clone();
                Some(Rc::new(move |world| {
                    commit(world, value.instantiate(), on_commit.clone())
                }))
            }
            Self::Label(commit) => value.literal().and_then(Value::as_cell).map(|cell| {
                let commit = commit.clone();
                Rc::new(move |world: &mut C| commit(world, cell, None, on_commit.clone()))
                    as Rc<dyn Fn(&mut C)>
            }),
        }
    }

    fn new_cell(&self) -> Rc<dyn Fn(&mut C)> {
        match self {
            Self::Value(commit) => {
                let commit = commit.clone();
                Rc::new(move |world| commit(world, Value::from(new_cell_id()), None))
            }
            Self::Label(commit) => {
                let commit = commit.clone();
                Rc::new(move |world| commit(world, new_cell_id(), None, None))
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
        Self::offered(
            display,
            detail,
            CompletionValue::Literal(value),
            None,
            commit,
        )
    }

    fn offered(
        display: String,
        detail: Option<String>,
        value: CompletionValue,
        on_commit: Option<Value>,
        commit: &Commit<C>,
    ) -> Option<Self> {
        commit.value(value.clone(), on_commit).map(|activate| Self {
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
}

/// Retained in the placed frame for attribution to the exact visible offers.
pub struct Offers<C> {
    pub entries: Vec<Entry<C>>,
}

pub(crate) fn constructor_entries<C: 'static>(commit: &Commit<C>) -> Vec<(&'static str, Entry<C>)> {
    std::iter::once((
        "(",
        Entry {
            display: "new cell".to_string(),
            detail: None,
            matches: Vec::new(),
            face: Face::Dim,
            source: None,
            activate: commit.new_cell(),
        },
    ))
    .chain(
        [
            ("[", "new list", Value::list([])),
            ("{", "new record", Value::record([])),
        ]
        .into_iter()
        .filter_map(|(key, display, value)| {
            Entry::value(display.to_string(), None, value, commit).map(|mut entry| {
                entry.face = Face::Dim;
                (key, entry)
            })
        }),
    )
    .collect()
}

pub(crate) struct Prepared {
    pub document: gid::Document,
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
    on_commit: Option<&Value>,
) -> Option<Prepared> {
    use crate::selection::{self, Stage};
    let mut document = sources.doc.clone();
    let mut path = selection.path().to_vec();
    let (document_changed, payload) = match selection.stage() {
        Stage::Pending => selection::set_value(&mut document, sources.libraries, &path, value)
            .then_some((true, selection::payload::edge()))?,
        Stage::Label => {
            let label = value.as_cell()?;
            path.push(gid::Step::Key(label));
            if sources.resolve_path(&path).is_some() {
                (false, selection::payload::edge())
            } else {
                let changed = definition.is_some();
                if let Some(value) = definition {
                    document.cells.set_value(label, value);
                }
                (changed, selection::payload::pending("", 0))
            }
        }
        Stage::Edge => return None,
    };
    let annotation = annotations.at(&path).cloned();
    let selection = Some((path.clone(), payload));
    let mut effects = match on_commit {
        Some(function) => crate::site::evaluate(
            function,
            [],
            &path,
            annotation,
            selection,
            &Sources {
                doc: &document,
                libraries: sources.libraries,
            },
            grap::DEFAULT_FUEL,
        )?,
        None => crate::site::PendingChanges {
            annotation,
            annotation_changed: false,
            selection,
            selection_changed: false,
        },
    };
    effects.selection_changed = true;
    Some(Prepared {
        document,
        document_changed,
        path,
        effects,
    })
}

pub(crate) fn completion_entries_with<C: 'static>(
    sources: &Sources,
    raw: bool,
    commit: &Commit<C>,
    query: &str,
    value_completions: Option<&CompletionProvider>,
    contextual: Option<&CompletionProvider>,
    everything: bool,
) -> Vec<Entry<C>> {
    if !everything && let Some(contextual) = contextual {
        return contextual_entries(contextual, query, commit);
    }
    let labels = matches!(commit, Commit::Label(_));
    let trimmed = query.trim();
    let quoted = trimmed.starts_with('"');
    let value_entries = value_completions
        .filter(|_| !labels && !quoted)
        .map(|provider| contextual_entries(provider, query, commit))
        .unwrap_or_default();
    let blob = (!labels).then(|| blob::parse(trimmed)).flatten();
    let spelling = trimmed
        .strip_prefix('"')
        .map(|inner| inner.strip_suffix('"').unwrap_or(inner))
        .unwrap_or(query);
    let atom = blob
        .as_ref()
        .map(|bytes| Value::from(bytes.clone()))
        .unwrap_or_else(|| text::value(spelling));
    let atom_leads = quoted || blob.is_some() || !value_entries.is_empty();
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
                    commit(
                        world,
                        new_cell_id(),
                        Some(name::record(&spelling, [])),
                        None,
                    )
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
                    .contributors(cell)
                    .map(|source| source_name(sources, source))
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
        .map(|(entry, named, _)| (entry, named, None))
        .collect();
    references_pool.extend(
        constructor_entries(commit)
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
    let mut entries = contextual
        .map(|provider| contextual_entries(provider, query, commit))
        .unwrap_or_default();
    entries.extend(value_entries);
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
    filter::rank_with_aliases(
        provider(query),
        |completion| &completion.display,
        |completion| &completion.aliases,
        query,
    )
    .into_iter()
    .filter_map(|ranked| {
        let completion = ranked.item;
        Entry::offered(
            completion.display,
            completion.detail,
            completion.value,
            completion.on_commit,
            commit,
        )
        .map(|entry| Entry {
            matches: ranked.matches,
            ..entry
        })
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
