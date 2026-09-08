use super::completion::{completion_card, completion_placement};
use super::*;
use crate::annotations::Annotations;
use crate::completion::{Entry, Offers};
use crate::hover::hover_secondary;
use crate::identity::short_id;
use crate::libraries::layout as layout_data;
use crate::libraries::{Libraries, f64, fidget, name, text};
use crate::navigate::{projected_name_owner, step_selection};
use crate::placed::leaf;
use crate::sample::{sample_document, sample_vocabulary};
use crate::selection::payload as selection_payload;
use crate::selection::{
    break_edit_run, delete_edge, from_clipboard, from_structure, pending_edge, pending_follow,
    pending_insert, pending_into, pending_value, resolve_query, set_collapse, set_value,
    to_clipboard, toggle_collapse,
};
use gid::Position;
use gid::{Cells, Document, new_cell_id};
use kurbo::Rect;
use measured::Extent;
use peniko::Brush;
use ui_events::ScrollDelta;
use ui_events::keyboard::KeyboardEvent;
use ui_events::keyboard::{KeyState, Modifiers};
use ui_events::pointer::{
    PointerButton, PointerButtonEvent, PointerId, PointerInfo, PointerScrollEvent, PointerState,
    PointerType, PointerUpdate,
};

fn libraries(cells: Cells) -> Libraries {
    Libraries::from_contributions([(
        CellId::from_u128(1),
        crate::libraries::Library::<(), ()>::named(
            CellId::from_u128(1),
            "test",
            crate::libraries::Definitions::from_parts(cells, grap::ForeignFunctions::default()),
            crate::display::partial(|_| None),
        ),
    )])
    .0
}

fn core_libraries() -> Libraries {
    crate::stack::load().libraries
}

fn root_completions<World>(stack: &crate::stack::Stack<World>) -> Vec<crate::display::Completion> {
    let document = Document {
        root: None,
        cells: Cells::new(),
    };
    let sources = src(&document, &stack.libraries);
    (stack.completions)(&crate::display::CompletionRequest {
        query: "",
        kind: crate::display::CompletionKind::Value,
        scope: crate::display::CompletionScope::Suggested,
        path: &[],
        value_at: &|_| None,
        resolve: &|cell| sources.definition(cell),
    })
    .unwrap_or_default()
}

fn completion_entries_with(
    sources: &Sources,
    raw: bool,
    kind: &crate::display::CompletionKind,
    query: &str,
    providers: Option<&crate::display::CompletionProvider>,
    contextual: Option<&crate::display::CompletionProvider>,
    everything: bool,
) -> Vec<Entry<completion::CompletionResult>> {
    use crate::display::{CompletionRequest, CompletionScope};
    let value_at = |path: &[Step]| sources.resolve_path(path);
    crate::completion::completion_entries_with(
        sources,
        raw,
        &CompletionRequest {
            query,
            kind: *kind,
            scope: if everything {
                CompletionScope::Everything
            } else {
                CompletionScope::Suggested
            },
            path: &[],
            value_at: &value_at,
            resolve: &|cell| sources.definition(cell),
        },
        providers,
        contextual,
    )
    .0
    .into_iter()
    .map(|entry| completion::test_entry(entry, *kind))
    .collect()
}

fn src<'a>(doc: &'a Document, libraries: &'a Libraries) -> Sources<'a> {
    Sources { doc, libraries }
}

fn make_selection(path: Path) -> Selection {
    Selection::edge(&crate::test_root(), path)
}

type EditingWorld = crate::Editor;

fn editing_world(doc: &Document, libraries: &Libraries) -> EditingWorld {
    let mut world = crate::test_editor(doc.clone());
    world.stack.libraries = libraries.clone();
    world
}

fn editing_frame(world: &mut EditingWorld, raw: bool) -> crate::placed::Ready<EditingWorld> {
    editing_frame_with_projection(world, raw, None)
}

fn editing_frame_with_projection(
    world: &mut EditingWorld,
    raw: bool,
    projection: Option<&Projection<EditingWorld>>,
) -> crate::placed::Ready<EditingWorld> {
    editing_frame_at(world, raw, projection, None)
}

fn editing_frame_at(
    world: &mut EditingWorld,
    raw: bool,
    projection: Option<&Projection<EditingWorld>>,
    pointer: Option<Point>,
) -> crate::placed::Ready<EditingWorld> {
    let stack = crate::stack::load();
    let styles = crate::styles::editor(1.0);
    let annotations = Annotations::default();
    let mut tcx = TextCtx {
        fonts: &mut world.font_cx,
        layouts: &mut world.layout_cx,
        scale: 1.0,
        cache: &mut world.text_cache,
    };
    let measured = project(
        ProjectDescription {
            view: &crate::test_root(),
            completions: Some(&stack.completions),
            sources: Sources {
                doc: &world.model.doc,
                libraries: &world.stack.libraries,
            },
            root: world.model.doc.root.as_ref(),
            root_path: &[],
            selection: world.model.selection.as_ref(),
            source_selection: world.model.selection.as_ref(),
            annotations: &annotations,
            raw,
            styles: &styles,
            width: 500.0,

            projection: (!raw).then_some(projection.unwrap_or(&stack.projection)),
        },
        &mut tcx,
    );
    let height = measured.extent.height().max(1.0);
    measured::place(
        measured,
        Placement::root(Rect::new(0.0, 0.0, 500.0, height)),
    )
    .run(&placed::HoverInput {
        pointer,
        ..Default::default()
    })
}

/// Select through the current projection, without an editing interaction.
fn make_projected_selection(doc: &Document, libraries: &Libraries, path: Path) -> Selection {
    let mut world = editing_world(doc, libraries);
    let placed = editing_frame(&mut world, false);
    if let Some(target) = placed
        .descends
        .iter()
        .find(|target| target.path.as_ref() == &path)
    {
        (target.select)(&mut world, None);
    }
    world
        .model
        .selection
        .unwrap_or_else(|| make_selection(path))
}

fn make_projected_editing_selection(
    doc: &Document,
    libraries: &Libraries,
    path: Path,
) -> Selection {
    let mut world = editing_world(doc, libraries);
    world.model.selection = Some(make_projected_selection(doc, libraries, path));
    editing_frame(&mut world, false)
        .handler
        .unwrap()
        .dispatch_key(&mut world, &arrow(NamedKey::End));
    world.model.selection.unwrap()
}

fn make_editing_selection(doc: &Document, libraries: &Libraries, path: Path) -> Selection {
    make_projected_editing_selection(doc, libraries, path)
}

fn write_text(doc: &mut Rc<Document>, libraries: &Libraries, selected: &mut Selection) -> bool {
    write_with(doc, libraries, selected, text::edit)
}

fn write_with(
    doc: &mut Rc<Document>,
    libraries: &Libraries,
    selected: &mut Selection,
    update: impl Fn(&str, Option<&Value>) -> Option<Value> + 'static,
) -> bool {
    line_control::commit(
        doc,
        libraries,
        selected,
        &crate::libraries::line_edit::native(update),
    )
}

fn toggle_fold(sources: &Sources, collapse: &mut Annotations, path: &[Step]) -> bool {
    toggle_collapse(sources, collapse, path)
}

fn set_fold(sources: &Sources, collapse: &mut Annotations, path: &[Step], closed: bool) -> bool {
    set_collapse(sources, collapse, path, closed)
}

fn key(s: &str) -> Step {
    Step::Key(crate::test_values::label(s))
}

/// A one-cell document: the root links a cell holding `fields`.
fn doc_of(fields: Vec<(CellId, Value)>) -> (Rc<Document>, CellId) {
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

/// The ordered positions of a list value's elements.
fn positions(value: &Value) -> Vec<Position> {
    value.as_list().unwrap().keys().cloned().collect()
}

const LINE: f64 = 16.0;

fn stop(path: Vec<Step>, x0: f64, y0: f64, x1: f64, y1: f64) -> Descend<()> {
    Descend {
        root: None,
        path: Rc::from(path),
        rect: Rect::new(x0, y0, x1, y1),
        select: Rc::new(|_, _| true),
    }
}

fn arrow(named: NamedKey) -> KeyboardEvent {
    KeyboardEvent {
        key: Key::Named(named),
        state: KeyState::Down,
        modifiers: Modifiers::empty(),
        ..Default::default()
    }
}

fn stepped(ds: &[Descend<()>], from: Option<Vec<Step>>, named: NamedKey) -> Option<Path> {
    let selection = from.map(|path| crate::selection::bare_edge(&crate::test_root(), path));
    step_selection(ds, None, selection.as_ref(), LINE, &arrow(named))
        .map(|descend| descend.path.to_vec())
}

mod completion;
mod display;
mod editing;
mod events;
mod frame;
mod navigation;

fn measured_rect(width: f64) -> kurbo::Rect {
    kurbo::Rect::new(0.0, 0.0, width, 100.0)
}
