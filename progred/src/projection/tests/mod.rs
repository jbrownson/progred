use super::completion::{completion_card, completion_placement};
use super::*;
use crate::annotations::Annotations;
use crate::completion::{Commit, Entry, Offers};
use crate::hover::hover_secondary;
use crate::identity::short_id;
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
use progred_libraries::layout as layout_data;
use progred_libraries::{Libraries, f64, fidget, name, text};
use puri::edit::EditCtx;
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

fn make_selection(path: Path) -> Selection {
    Selection::edge(&crate::workspace::Root::document(), path)
}

#[derive(Default)]
struct TestClipboard(Option<String>);

impl puri::edit::TextClipboard for TestClipboard {
    fn get_text(&mut self) -> Option<String> {
        self.0.clone()
    }

    fn set_text(&mut self, text: &str) {
        self.0 = Some(text.into());
    }
}

struct EditingWorld {
    doc: Rc<Document>,
    libraries: Libraries,
    selection: Option<Selection>,
    fonts: parley::FontContext,
    layouts: parley::LayoutContext<Brush>,
    cache: puri::text::TextCache,
    clipboard: TestClipboard,
}

impl EditingWorld {
    fn new(doc: &Document, libraries: &Libraries) -> Self {
        Self {
            doc: Rc::new(doc.clone()),
            libraries: libraries.clone(),
            selection: None,
            fonts: parley::FontContext::new(),
            layouts: parley::LayoutContext::new(),
            cache: puri::text::TextCache::default(),
            clipboard: TestClipboard::default(),
        }
    }
}

fn editing_frame(world: &mut EditingWorld, raw: bool) -> Placed<EditingWorld, crate::frame::Paint> {
    editing_frame_with_projection(world, raw, None)
}

fn editing_frame_with_projection(
    world: &mut EditingWorld,
    raw: bool,
    projection: Option<&Projection<EditingWorld>>,
) -> Placed<EditingWorld, crate::frame::Paint> {
    let stack = crate::stack::load::<EditingWorld>();
    let styles = crate::styles::editor(1.0);
    let annotations = Annotations::default();
    let mut tcx = TextCtx {
        fonts: &mut world.fonts,
        layouts: &mut world.layouts,
        scale: 1.0,
        cache: &mut world.cache,
    };
    let measured = project::<EditingWorld, crate::frame::Paint>(
        ProjectDescription {
            sources: Sources {
                doc: &world.doc,
                libraries: &world.libraries,
            },
            root: world.doc.root.as_ref(),
            root_path: &[],
            selection: world.selection.as_ref(),
            scrub_spelling: None,
            source_selection: world.selection.as_ref(),
            annotations: &annotations,
            raw,
            styles: &styles,
            width: 500.0,

            projection: (!raw).then_some(projection.unwrap_or(&stack.projection)),
        },
        &mut tcx,
        Hooks {
            completions: Some(stack.completions.clone()),
            select: Rc::new(|world, path| {
                world.selection = Some(make_selection(path));
            }),
            select_payload: Rc::new(|world, path, payload| {
                world.selection = Some(Selection::from_payload(
                    &crate::workspace::Root::document(),
                    &src(&world.doc, &world.libraries),
                    path,
                    payload,
                ));
            }),
            edit_line: Rc::new(|world, path, line, operation| {
                if !writable_at(&src(&world.doc, &world.libraries), path) {
                    return false;
                }
                let Some(selected) = world.selection.as_mut().filter(|selected| {
                    selected.path() == path
                        && selected.stage(&src(&world.doc, &world.libraries)) == Stage::Edge
                }) else {
                    return false;
                };
                line_control::edit(&mut world.doc, &world.libraries, selected, line, |state| {
                    operation(EditCtx {
                        state,
                        fonts: &mut world.fonts,
                        layouts: &mut world.layouts,
                        clipboard: &mut world.clipboard,
                    })
                })
                .0
            }),
            toggle: Rc::new(|_, _| {}),
            update_state: Rc::new(|_, _, _| false),
            edit: Rc::new(|world, operation| {
                let Some(selected) = world.selection.as_mut().filter(|selected| {
                    selected.stage(&src(&world.doc, &world.libraries)) != Stage::Edge
                }) else {
                    return false;
                };
                selected.edit_query(|state| {
                    operation(EditCtx {
                        state,
                        fonts: &mut world.fonts,
                        layouts: &mut world.layouts,
                        clipboard: &mut world.clipboard,
                    })
                })
            }),
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
    measured::place(
        measured,
        Placement::root(Rect::new(0.0, 0.0, 500.0, height)),
    )
}

/// Select through the current projection, without an editing interaction.
fn make_projected_selection(doc: &Document, libraries: &Libraries, path: Path) -> Selection {
    let mut world = EditingWorld::new(doc, libraries);
    let placed = editing_frame(&mut world, false);
    if let Some(target) = placed
        .descends
        .iter()
        .find(|target| target.path.as_ref() == &path)
    {
        (target.select)(&mut world, None);
    }
    world.selection.unwrap_or_else(|| make_selection(path))
}

fn make_projected_editing_selection(
    doc: &Document,
    libraries: &Libraries,
    path: Path,
) -> Selection {
    let mut world = EditingWorld::new(doc, libraries);
    world.selection = Some(make_projected_selection(doc, libraries, path));
    editing_frame(&mut world, false)
        .handler
        .unwrap()
        .dispatch_key(&mut world, &arrow(NamedKey::End));
    world.selection.unwrap()
}

fn make_editing_selection(doc: &Document, libraries: &Libraries, path: Path) -> Selection {
    Selection::from_line(
        &crate::workspace::Root::document(),
        &src(doc, libraries),
        path.clone(),
        projected_line(doc, libraries, &path).expect("value is not line editable"),
    )
}

fn projected_line(
    doc: &Document,
    libraries: &Libraries,
    path: &[Step],
) -> Option<progred_display::LineEdit> {
    struct NoEval;
    impl progred_display::Env for NoEval {
        fn apply(&self, _: &gid::Value, _: &[(gid::CellId, gid::Value)]) -> (gid::Value, usize) {
            panic!("unexpected projection application")
        }

        fn evaluate(&self, _: &Value) -> (Value, usize) {
            panic!("line projection evaluated")
        }
    }

    let value = src(doc, libraries).resolve_path(path)?;
    let stack = crate::stack::load::<()>();
    let layout = {
        let target = |_| progred_display::ProjectionTarget {
            select: Rc::new(|_: &mut ()| false),
            select_with: Rc::new(|_: &mut (), _| false),
            hover: Hover::Value(Rc::from(path)),
        };
        stack.projection.apply(&progred_display::ProjectionInput {
            default_projection: progred_display::partial(|_| None),
            env: &NoEval,
            value: Some(value),
            scale_factor: 1.0,
            writable: true,
            selection: None,
            pending: None,
            state: None,
            targets: progred_display::ProjectionTargets::new(&target),
        })
    }?;
    let layout = match layout {
        progred_display::Layout::OnScrub { child, .. } => *child,
        layout => layout,
    };
    match layout {
        progred_display::Layout::Widget(widget) => placed_line_description(&widget),
        progred_display::Layout::Row { children, .. } => {
            children.into_iter().find_map(|child| match child {
                progred_display::Layout::Widget(widget) => placed_line_description(&widget),
                _ => None,
            })
        }
        _ => None,
    }
}

fn placed_line_description(
    widget: &progred_display::widget::Widget<(), Hover>,
) -> Option<progred_display::LineEdit> {
    use std::cell::RefCell;
    let mut fonts = parley::FontContext::new();
    let mut layouts = parley::LayoutContext::new();
    let mut cache = puri::TextCache::default();
    let captured = Rc::new(RefCell::new(None));
    let output = captured.clone();
    let measured = widget(&mut progred_display::widget::Context {
        text: &mut TextCtx {
            fonts: &mut fonts,
            layouts: &mut layouts,
            cache: &mut cache,
            scale: 1.0,
        },
        styles: &crate::styles::editor(1.0),
        site: &|| {
            let output = output.clone();
            progred_display::widget::Site {
                writable: true,
                selected: true,
                editing: None,
                spelling: None,
                initial_text: &crate::selection::line_edit,
                target: Hover::Value(Rc::from([])),
                value: None,
                select: Rc::new(|_| true),
                edit: Rc::new(move |_, description, _| {
                    output.replace(Some(description.clone()));
                    true
                }),
            }
        },
        pick: Rc::new(|_, _| false),
        picking: |_| false,
        same_target: |_, _| false,
        primary_edit: |_| true,
    });
    let placement = Placement::root(measured.extent.rect_at(Point::ZERO));
    measured::place(measured, placement)
        .handler?
        .dispatch_key(&mut (), &puri::handler::KeyboardEvent::default());
    captured.take()
}

// Direct conversion tests use a fresh projection's callback, just as a
// newly minted handler does; the selection stores no conversion.
fn write_through(doc: &mut Rc<Document>, libraries: &Libraries, selected: &mut Selection) -> bool {
    projected_line(doc, libraries, selected.path())
        .is_some_and(|line| line_control::commit(doc, libraries, selected, &line.update))
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
