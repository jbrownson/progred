//! Test helpers shared by several modules' tests: sources over the core
//! libraries, small documents, key events, and completion offers, read
//! directly or through a projection.

use crate::annotations::Annotations;
use crate::display::CompletionKind;
use crate::display::widget::offers::Entry;
use crate::library::Libraries;
use crate::projection::{ProjectDescription, Projection, project};
use crate::selection::{Selection, pending_edge};
use crate::sources::Sources;
use gid::{CellId, Cells, Document, Path, Position, Step, Value, new_cell_id};
use kurbo::{Point, Rect};
use puri::text::TextCtx;
use std::rc::Rc;
use ui_events::keyboard::{Key, KeyState, KeyboardEvent, Modifiers, NamedKey};
use uig::Placement;

pub(crate) fn core_libraries() -> Libraries {
    crate::stack::load().libraries
}

pub(crate) fn root_completions<World>(
    stack: &crate::stack::Stack<World>,
) -> Vec<crate::display::Completion> {
    let document = Document {
        root: None,
        cells: Cells::new(),
    };
    let sources = src(&document, &stack.libraries);
    (stack.completions)(&crate::display::CompletionRequest {
        raw: false,
        query: "",
        kind: crate::display::CompletionKind::Value,
        scope: crate::display::CompletionScope::Start,
        path: &[],
        value_at: &|_| None,
        resolve: &|cell| sources.resolve(cell),
        cells: &|| sources.cells(),
        name: &|cell| crate::libraries::name::of(&sources, cell),
        libraries: None,
    })
    .unwrap_or_default()
}

/// The libraries' answers to `scope`, as a projection that asks for them,
/// such as Grap's slots, offers them.
pub(crate) fn library_offers(
    completions: crate::display::CompletionProvider,
    scope: crate::display::CompletionScope,
) -> crate::display::CompletionProvider {
    std::rc::Rc::new(move |request| {
        completions(&crate::display::CompletionRequest { scope, ..*request })
    })
}

/// What a hole lists for `query`: `contextual`'s offers ranked, or without
/// them Raw's, reading what's typed through `providers`.
pub(crate) fn completion_entries_with(
    sources: &Sources,
    raw: bool,
    kind: &crate::display::CompletionKind,
    query: &str,
    providers: Option<&crate::display::CompletionProvider>,
    contextual: Option<&crate::display::CompletionProvider>,
) -> Vec<Entry<CompletionResult>> {
    use crate::display::{CompletionRequest, CompletionScope};
    let value_at = |path: &[Step]| sources.resolve_path(path);
    let offers = contextual.map_or_else(
        || crate::display::raw::picker(*kind).offers,
        |provider| crate::display::ranked(provider.clone()),
    );
    offers(&CompletionRequest {
        raw,
        query,
        kind: *kind,
        scope: CompletionScope::Everything,
        path: &[],
        value_at: &value_at,
        resolve: &|cell| sources.resolve(cell),
        cells: &|| sources.cells(),
        name: &|cell| crate::libraries::name::of(&sources, cell),
        libraries: providers,
    })
    .into_iter()
    .map(|entry| test_entry(entry, *kind))
    .collect()
}

pub(crate) fn src<'a>(doc: &'a Document, libraries: &'a Libraries) -> Sources<'a> {
    Sources { doc, libraries }
}

pub(crate) fn make_selection(path: Path) -> Selection {
    Selection::edge(&crate::test_root(), path)
}

/// A one-cell document: the root links a cell holding `fields`.
pub(crate) fn doc_of(fields: Vec<(CellId, Value)>) -> (Rc<Document>, CellId) {
    let mut cells = Cells::new();
    let cell = new_cell_id();
    cells.set_value(cell, Value::record(fields));
    (
        Rc::new(Document {
            root: Some(Value::from(cell)),
            cells,
        }),
        cell,
    )
}

/// Three unnamed record slots, empty until filled.
pub(crate) const EMPTY_SLOTS: &str = r#"{
  "binders": {
    "first": 9940ece27410c72a5308a544890ccc71,
    "second": f717b766d250a7b86c5eb842885c4417,
    "third": 5e716c07490849f072b4e9017dd6230d,
  },
  "cells": {},
  "root": {},
}"#;

/// Configure the editor's projection slots as a tutorial page does.
pub(crate) fn set_tutorial_slots(world: &mut crate::Editor, slots: &[CellId]) {
    world.stack.projection = crate::web_embed::tutorial_slots(
        Some(
            &slots
                .iter()
                .map(CellId::to_string)
                .collect::<Vec<_>>()
                .join(","),
        ),
        world.stack.projection.clone(),
        &world.stack.libraries,
    )
    .unwrap();
}

/// The ordered positions of a list value's elements.
pub(crate) fn positions(value: &Value) -> Vec<Position> {
    value.as_list().unwrap().keys().cloned().collect()
}

pub(crate) fn arrow(named: NamedKey) -> KeyboardEvent {
    KeyboardEvent {
        key: Key::Named(named),
        state: KeyState::Down,
        modifiers: Modifiers::empty(),
        ..Default::default()
    }
}

#[derive(Default)]
pub(crate) struct CompletionResult {
    pub(crate) value: Option<Value>,
    pub(crate) label: Option<(CellId, Option<Value>)>,
    pub(crate) selected: Option<(Path, Value)>,
}

/// What choosing `entry` did, as [`test_entry`] records it.
pub(crate) fn activated(entry: &Entry<CompletionResult>) -> CompletionResult {
    let mut result = CompletionResult::default();
    (entry.activate)(&mut result);
    result
}

pub(crate) fn test_entry(
    entry: Entry<crate::Editor>,
    kind: CompletionKind,
) -> Entry<CompletionResult> {
    Entry {
        display: entry.display,
        detail: entry.detail,
        matches: entry.matches,
        face: entry.face,
        source: entry.source,
        activate: Rc::new(move |result| {
            let mut world = crate::test_editor(Document {
                root: (kind == CompletionKind::Field).then(|| Value::record([])),
                cells: Cells::new(),
            });
            let root = world.model.workspace.document_root().clone();
            world.model.selection = Some(match kind {
                CompletionKind::Value => crate::selection::pending_value(&root, vec![]),
                CompletionKind::Field => pending_edge(
                    &root,
                    &crate::editing::Scope::default().view(world.sources()),
                    vec![],
                )
                .unwrap(),
            });
            (entry.activate)(&mut world);
            result.selected = world
                .model
                .selection
                .as_ref()
                .map(|s| (s.path().to_vec(), s.payload()));
            match kind {
                CompletionKind::Value => result.value = world.model.doc.root.clone(),
                CompletionKind::Field => {
                    let cell = match world.model.selection.as_ref().unwrap().path().first() {
                        Some(Step::Key(cell)) => *cell,
                        _ => panic!("label activation must select its field"),
                    };
                    result.label = Some((cell, world.model.doc.cells.value(cell).cloned()));
                }
            }
        }),
    }
}

pub(crate) fn projected_completion_entries(
    doc: &Document,
    selection: &Selection,
) -> Vec<Entry<crate::Editor>> {
    projected_completion_entries_with(doc, selection, None, None)
}

pub(crate) fn projected_completion_entries_with(
    doc: &Document,
    selection: &Selection,
    projection: Option<&Projection<crate::Editor>>,
    provider: Option<&crate::display::CompletionProvider>,
) -> Vec<Entry<crate::Editor>> {
    projected_completion_entries_in(&crate::stack::load(), doc, selection, projection, provider)
}

pub(crate) fn projected_completion_entries_in(
    stack: &crate::stack::Stack<crate::Editor>,
    doc: &Document,
    selection: &Selection,
    projection: Option<&Projection<crate::Editor>>,
    provider: Option<&crate::display::CompletionProvider>,
) -> Vec<Entry<crate::Editor>> {
    let styles = crate::display::widget::style::editor(
        crate::display::widget::style::Theme::Light.palette(),
        1.0,
    );
    let annotations = Annotations::default();
    let mut fonts = crate::fonts::bundled_font_context();
    let mut layouts = parley::LayoutContext::new();
    let mut cache = puri::text::TextCache::default();
    let mut tcx = TextCtx {
        fonts: &mut fonts,
        layouts: &mut layouts,
        scale: 1.0,
        cache: &mut cache,
    };
    let computations = crate::computations::Computations::from_sources(src(doc, &stack.libraries));
    let measured = project(
        ProjectDescription {
            focused: true,
            computations: &computations,
            view: &crate::test_root(),
            completions: provider.or(Some(&stack.completions)),
            sources: src(doc, &stack.libraries),
            root: doc.root.as_ref(),
            root_path: &[],
            selection: Some(selection),
            source_selection: Some(selection),
            annotations: &annotations,
            raw: false,
            styles: &styles,
            width: 500.0,

            projection: Some(projection.unwrap_or(&stack.projection)),
        },
        &mut tcx,
    );
    let extent = measured.extent;
    crate::display::widget::frame::place(
        measured,
        Placement::root(Rect::from_origin_size(Point::ZERO, extent.size())),
        &Default::default(),
    )
    .completion
    .expect("the selected pending emits its offers")
    .entries
}

pub(crate) fn activate_projected(
    doc: &Document,
    selection: &Selection,
    entry: &Entry<crate::Editor>,
) -> crate::Editor {
    let mut editor = crate::test_editor(doc.clone());
    editor.model.workspace.document.root = selection.root().clone();
    editor.model.selection = Some(Selection::from_payload(
        selection.root(),
        &editor.sources(),
        selection.path().to_vec(),
        selection.payload(),
    ));
    (entry.activate)(&mut editor);
    editor
}

pub(crate) fn inserted_value(
    doc: &Document,
    selection: &Selection,
    entry: &Entry<crate::Editor>,
) -> Option<Value> {
    activate_projected(doc, selection, entry)
        .sources()
        .resolve_path(selection.path())
        .cloned()
}
