use super::completion::{completion_card, completion_placement};
use super::*;
use crate::annotations::Annotations;
use crate::completion::{Commit, Entry, Offers};
use crate::hover::hover_secondary;
use crate::identity::short_id;
use crate::navigate::{projected_name_owner, step_selection};
use crate::sample::{sample_document, sample_vocabulary};
use crate::selection::payload as selection_payload;
use crate::selection::{
    break_edit_run, delete_edge, from_clipboard, from_structure, pending_edge, pending_follow,
    pending_insert, pending_into, pending_value, resolve_query, set_collapse, set_value,
    to_clipboard, toggle_collapse, write_through,
};
use gid::Position;
use gid::{Cells, Document, new_cell_id};
use progred_libraries::layout as layout_data;
use progred_libraries::{Libraries, f64, fidget, name, text};
use ui_events::ScrollDelta;
use ui_events::keyboard::KeyboardEvent;
use ui_events::keyboard::{KeyState, Modifiers};
use ui_events::pointer::{
    PointerButton, PointerButtonEvent, PointerId, PointerInfo, PointerScrollEvent, PointerState,
    PointerType, PointerUpdate,
};

fn contextual_probe(
    _: &progred_display::ProjectionInput<'_, (), Hover>,
) -> Option<progred_display::Layout<(), Hover>> {
    Some(progred_display::text("contextual"))
}

fn ambient_probe(
    _: &progred_display::ProjectionInput<'_, (), Hover>,
) -> Option<progred_display::Layout<(), Hover>> {
    Some(progred_display::text("ambient"))
}

fn declining_probe(
    _: &progred_display::ProjectionInput<'_, (), Hover>,
) -> Option<progred_display::Layout<(), Hover>> {
    None
}

fn libraries(cells: Cells) -> Libraries {
    Libraries::from_contributions([(
        CellId::from_u128(1),
        progred_libraries::Library::<(), ()>::named(
            CellId::from_u128(1),
            "test",
            progred_libraries::Definitions::from_parts(cells, grap::ForeignFunctions::default()),
            progred_display::partial(|_| None),
        ),
    )])
    .0
}

fn core_libraries() -> Libraries {
    crate::stack::load::<()>().libraries
}

fn root_completions<World>(stack: &crate::stack::Stack<World>) -> Vec<progred_display::Completion> {
    let document = Document {
        root: None,
        cells: Cells::new(),
    };
    let sources = src(&document, &stack.libraries);
    (stack.completions)(&progred_display::CompletionRequest {
        query: "",
        kind: progred_display::CompletionKind::Value,
        scope: progred_display::CompletionScope::Suggested,
        path: &[],
        value_at: &|_| None,
        resolve: &|cell| sources.definition(cell),
    })
    .unwrap_or_default()
}

fn completion_entries_with<C: 'static>(
    sources: &Sources,
    raw: bool,
    commit: &Commit<C>,
    query: &str,
    providers: Option<&progred_display::CompletionProvider>,
    contextual: Option<&progred_display::CompletionProvider>,
    everything: bool,
) -> Vec<Entry<C>> {
    use progred_display::{CompletionKind, CompletionRequest, CompletionScope};
    let value_at = |path: &[Step]| sources.resolve_path(path);
    crate::completion::completion_entries_with(
        sources,
        raw,
        commit,
        &CompletionRequest {
            query,
            kind: if matches!(commit, Commit::Label(_)) {
                CompletionKind::Field
            } else {
                CompletionKind::Value
            },
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
}

fn src<'a>(doc: &'a Document, libraries: &'a Libraries) -> Sources<'a> {
    Sources { doc, libraries }
}

fn make_selection(doc: &Document, libraries: &Libraries, path: Path) -> Selection {
    Selection::edge(
        &crate::workspace::Root::document(),
        &src(doc, libraries),
        path,
    )
}

/// Select through the action installed by the projected navigation
/// landmark, as the shell does after an arrow step.
fn make_projected_selection(doc: &Document, libraries: &Libraries, path: Path) -> Selection {
    type World = Vec<(Path, progred_display::LineEdit)>;

    let stack = crate::stack::load::<World>();
    let mut projection_libraries = libraries.clone();
    for (id, definitions) in stack.libraries.iter() {
        projection_libraries.insert(id, definitions.clone());
    }
    let styles = crate::styles::editor(1.0);
    let annotations = Annotations::default();
    let mut fonts = parley::FontContext::new();
    let mut layouts = parley::LayoutContext::new();
    let mut cache = puri::text::TextCache::default();
    let mut tcx = TextCtx {
        fonts: &mut fonts,
        layouts: &mut layouts,
        scale: 1.0,
        cache: &mut cache,
    };
    let measured = project::<World, crate::frame::Paint>(
        ProjectDescription {
            sources: Sources {
                doc,
                libraries: &projection_libraries,
            },
            root: doc.root.as_ref(),
            root_path: &[],
            selection: None,
            scrub_spelling: None,
            source_selection: None,
            annotations: &annotations,
            raw: false,
            styles: &styles,
            width: 500.0,

            projection: Some(&stack.projection),
        },
        &mut tcx,
        Hooks {
            completions: Some(stack.completions.clone()),
            select: Rc::new(|_, _| {}),
            select_payload: Rc::new(|_, _, _| {}),
            start_edit: Rc::new(|selected, path, line| selected.push((path, line))),
            toggle: Rc::new(|_, _| {}),
            update_state: Rc::new(|_, _, _| false),
            edit: Rc::new(|_| None),
            pick: Rc::new(|_, _| false),
            insert: Rc::new(|_, _| {}),
            delete: Rc::new(|_, _| false),
            apply: Rc::new(|_, _, _, _| false),
            point: Rc::new(|_, _, _, _, _| false),
            state_drag: Rc::new(|_, _, _, _, _| {}),
            scrub: Rc::new(|_, _, _, _, _| false),
            select_source: Rc::new(|_, _, _| {}),
            commit_value: Rc::new(|_, _, _| {}),
            commit_label: Rc::new(|_, _, _, _| {}),
            set_completion_view: Rc::new(|_, _, _, _| {}),
        },
    );
    let height = measured.extent.height().max(1.0);
    let placed = measured::place(
        measured,
        Placement::root(Rect::new(0.0, 0.0, 500.0, height)),
    );
    let mut selected = World::new();
    if let Some(target) = placed
        .descends
        .iter()
        .find(|target| target.path.as_ref() == &path)
    {
        (target.select)(&mut selected);
    }
    match selected.pop() {
        Some((path, line)) => Selection::from_line(
            &crate::workspace::Root::document(),
            &src(doc, libraries),
            path,
            line,
        ),
        None => make_selection(doc, libraries, path),
    }
}

fn make_editing_selection(doc: &Document, libraries: &Libraries, path: Path) -> Selection {
    struct NoEval;
    impl progred_display::Env for NoEval {
        fn apply(&self, _: &gid::Value, _: &[(gid::CellId, gid::Value)]) -> (gid::Value, usize) {
            panic!("unexpected projection application")
        }

        fn evaluate(&self, _: &Value) -> (Value, usize) {
            panic!("line projection evaluated")
        }
    }

    let value = src(doc, libraries)
        .resolve_path(&path)
        .expect("selected value");
    let stack = crate::stack::load::<()>();
    let layout = {
        let target = |_| progred_display::ProjectionTarget {
            select: Rc::new(|_: &mut ()| false),
            select_with: Rc::new(|_: &mut (), _| false),
            hover: Hover::Value(Rc::from(path.clone())),
        };
        stack.projection.apply(&progred_display::ProjectionInput {
            env: &NoEval,
            value,
            scale_factor: 1.0,
            writable: true,
            selection: None,
            pending: None,
            state: None,
            targets: progred_display::ProjectionTargets::new(&target),
        })
    }
    .expect("value projection");
    let layout = match layout {
        progred_display::Layout::OnScrub { child, .. } => *child,
        layout => layout,
    };
    let layout = match layout {
        progred_display::Layout::Row { children, .. } => children.into_iter().next().unwrap(),
        layout => layout,
    };
    let progred_display::Layout::LineEdit(line) = layout else {
        panic!("value is not line editable")
    };
    Selection::from_line(
        &crate::workspace::Root::document(),
        &src(doc, libraries),
        path,
        line,
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
fn doc_of(fields: Vec<(CellId, Value)>) -> (Document, CellId) {
    let mut cells = Cells::new();
    let cell = new_cell_id();
    cells.set_value(cell, Value::record(fields));
    (
        Document {
            root: Some(Value::from(cell)),
            cells,
        },
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
        select: Rc::new(|_| true),
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
    let selection =
        from.map(|path| crate::selection::bare_edge(&crate::workspace::Root::document(), path));
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
