use super::*;
use crate::libraries::{editor::selection as selection_data, editor::site, grap::control, text};
use gid::{Cells, Document, Resolution, new_cell_id};
use std::rc::Rc;

/// An occurrence at a fresh path that jumps to `source`, as a projection's
/// jump draws it.
fn jump(source: &[gid::Step]) -> Path {
    vec![Step::Key(new_cell_id()), Step::jump(Location::root(source))]
}

impl Edit<'_> {
    /// Runs the Grap handler `function` evaluates to at `path`.
    fn grap(&mut self, path: Path, function: Value, event: Value) -> bool {
        let function =
            grap::evaluate(&function, &self.session.sources(), grap::DEFAULT_FUEL).result;
        self.grap_runtime(&crate::test_document(), path, function, event)
    }
}

/// An editor whose root holds `value` in a field, the document path to it,
/// and an occurrence that jumps there.
fn fixture(value: Value) -> (crate::Editor, gid::Path, Path) {
    let field = new_cell_id();
    let source = vec![gid::Step::Key(field)];
    let editor = crate::editor_of(Value::record([(field, value)]));
    let occurrence = jump(&source);
    (editor, source, occurrence)
}

/// The value at `path`, as the occurrence there reads it.
fn read<'a>(editor: &'a crate::Editor, path: &[Step]) -> Option<&'a Value> {
    Read {
        sources: editor.sources(),
    }
    .resolve_path(path)
}

fn line(spelling: &str) -> crate::display::LineEdit {
    crate::display::LineEdit {
        text: spelling.into(),
        update: Rc::new(text::edit),
        prefix: String::new(),
        suffix: String::new(),
        family: Default::default(),
        placeholder: None,
    }
}

#[test]
fn shared_source_edits_keep_selection_and_annotations_on_the_chosen_occurrence() {
    let (mut editor, source, first) = fixture(text::value("before"));
    let second = jump(&source);
    let annotation = text::value("local state");
    Edit::new(editor.session()).annotate(&crate::test_document(), &first, annotation.clone());
    Edit::new(editor.session()).select(&crate::test_document(), &second);
    assert!(
        Edit::new(editor.session()).edit_line(&second, &line("before"), &|edit| {
            edit.state.set_text("after");
            true
        })
    );
    assert_eq!(read(&editor, &first), Some(&text::value("after")));
    assert_eq!(read(&editor, &second), Some(&text::value("after")));
    assert_eq!(editor.model.selection.as_ref().unwrap().path(), second);
    let annotations = &editor
        .model
        .workspace
        .pane_at(&crate::test_document(), &first)
        .unwrap()
        .annotations;
    assert_eq!(annotations.at(&first), Some(&annotation));
    assert_eq!(annotations.at(&second), None);
    assert_eq!(annotations.at(&crate::occurrence::of(&source)), None);
    assert!(editor.model.step_history(true, &editor.stack.libraries));
    assert_eq!(read(&editor, &second), Some(&text::value("before")));
    let selected = editor.model.selection.as_ref().unwrap();
    assert_eq!(selected.path(), second);
    assert_eq!(selected.location(), Some(Location::root(source.as_slice())));
    assert!(
        Edit::new(editor.session()).edit_line(&second, &line("before"), &|edit| {
            edit.state.set_text("after undo");
            true
        })
    );
    assert_eq!(
        editor.sources().resolve_path(&source),
        Some(&text::value("after undo"))
    );
}

#[test]
fn structural_paste_delete_and_undo_keep_the_occurrence() {
    let (mut editor, source, occurrence) = fixture(text::value("old"));
    Edit::new(editor.session()).select(&crate::test_document(), &occurrence);
    assert!(editor.paste_value(text::value("pasted")));
    assert_eq!(
        editor.sources().resolve_path(&source),
        Some(&text::value("pasted"))
    );
    assert_eq!(editor.model.selection.as_ref().unwrap().path(), occurrence);
    assert!(editor.delete_selected_edge(Default::default()));
    assert_eq!(editor.sources().resolve_path(&source), None);
    assert!(editor.model.step_history(true, &editor.stack.libraries));
    let selected = editor.model.selection.as_ref().unwrap();
    assert_eq!(selected.path(), occurrence);
    assert_eq!(selected.location(), Some(Location::root(source.as_slice())));
    assert_eq!(
        selected.value(&editor.sources()),
        Some(&text::value("pasted"))
    );
    assert!(editor.model.step_history(true, &editor.stack.libraries));
    assert_eq!(
        editor.sources().resolve_path(&source),
        Some(&text::value("old"))
    );
}

#[test]
fn a_missing_child_under_a_jump_uses_the_ordinary_completion_and_continuation() {
    let (mut editor, source, mut occurrence) = fixture(Value::record([]));
    let field = new_cell_id();
    occurrence.push(Step::Key(field));
    Edit::new(editor.session()).select(&crate::test_document(), &occurrence);
    assert_eq!(
        editor
            .model
            .selection
            .as_ref()
            .unwrap()
            .stage(&editor.sources()),
        selection::Stage::Pending
    );
    assert!(crate::completion::commit(
        &mut editor.session(),
        text::value("inserted"),
        None,
        Some(Rc::new(|view, path, changes| {
            assert_eq!(view.resolve_path(path), Some(&text::value("inserted")));
            changes.select(path.to_vec(), crate::selection::payload::edge());
            true
        }))
    ));
    let source = [source, vec![gid::Step::Key(field)]].concat();
    assert_eq!(
        editor.sources().resolve_path(&source),
        Some(&text::value("inserted"))
    );
    let selected = editor.model.selection.as_ref().unwrap();
    assert_eq!(selected.path(), occurrence);
    assert_eq!(selected.location(), Some(Location::root(source.as_slice())));
    assert_eq!(selected.stage(&editor.sources()), selection::Stage::Edge);
}

#[test]
fn a_grap_handler_acts_at_its_occurrence_without_an_opaque_value() {
    let (mut editor, source, occurrence) = fixture(text::value("data"));
    let annotation = text::value("grap state");
    let handler = grap::lambda(
        [],
        grap::call(
            control::vocabulary::DO.into(),
            [(
                control::vocabulary::EXPRESSIONS,
                Value::list([
                    grap::call(
                        site::vocabulary::SET.into(),
                        [(site::vocabulary::VALUE, annotation.clone())],
                    ),
                    grap::call(
                        selection_data::vocabulary::SET.into(),
                        [
                            (
                                selection_data::vocabulary::PATH,
                                grap::call(site::vocabulary::PATH.into(), []),
                            ),
                            (site::vocabulary::VALUE, crate::selection::payload::edge()),
                        ],
                    ),
                ]),
            )],
        ),
    );
    assert!(Edit::new(editor.session()).grap(occurrence.clone(), handler, Value::record([])));
    let selected = editor.model.selection.as_ref().unwrap();
    assert_eq!(selected.path(), occurrence);
    assert_eq!(selected.location(), Some(Location::root(source.as_slice())));
    assert_eq!(
        editor
            .model
            .workspace
            .pane_at(&crate::test_document(), &occurrence)
            .unwrap()
            .annotations
            .at(&occurrence),
        Some(&annotation)
    );
    assert_eq!(
        crate::occurrence::from_value(&crate::occurrence::value(&occurrence)),
        Some(occurrence)
    );
    assert!(editor.paste_value(text::value("changed after Grap selection")));
    assert_eq!(
        editor.sources().resolve_path(&source),
        Some(&text::value("changed after Grap selection"))
    );
}

#[test]
fn folds_are_occurrence_local_and_undoable_without_dirtying_the_document() {
    let (mut editor, source, first) = fixture(Value::list([text::value("item")]));
    let second = jump(&source);
    let document = editor.model.doc.clone();
    assert!(
        editor
            .session()
            .set_folded(&crate::test_document(), &first, false, Some(true))
    );
    let annotations = &editor
        .model
        .workspace
        .pane_at(&crate::test_document(), &first)
        .unwrap()
        .annotations;
    assert!(crate::annotations::folded(annotations.at(&first), false));
    assert!(!crate::annotations::folded(annotations.at(&second), false));
    assert!(Rc::ptr_eq(&document, &editor.model.doc));
    assert!(editor.model.step_history(true, &editor.stack.libraries));
    assert!(
        editor
            .session()
            .set_folded(&crate::test_document(), &second, false, Some(true))
    );
}

#[test]
fn gesture_writes_share_the_occurrence_and_one_undo_step() {
    let (mut editor, source, occurrence) = fixture(text::value("before"));
    let mut run = Site::new(occurrence.clone().into()).edit_run();
    assert!(run.select(&mut editor.session()));
    assert!(run.write(&mut editor.session(), text::value("first")));
    assert!(run.write(&mut editor.session(), text::value("second")));
    assert!(!run.write(&mut editor.session(), text::value("second")));
    assert!(editor.model.step_history(true, &editor.stack.libraries));
    assert_eq!(
        editor.sources().resolve_path(&source),
        Some(&text::value("before"))
    );
    let selected = editor.model.selection.as_ref().unwrap();
    assert_eq!(selected.path(), occurrence);
    assert_eq!(selected.location(), Some(Location::root(source.as_slice())));
    assert!(!editor.model.history.can_undo());
}

#[test]
fn library_values_reached_by_a_jump_remain_read_only_and_failed_writes_change_nothing() {
    let mut editor = crate::editor_of(text::ID.into());
    let occurrence = jump(&[gid::Step::Follow(Resolution::Library(text::ID))]);
    let document = editor.model.doc.clone();
    assert!(read(&editor, &occurrence).is_some());
    assert!(
        !Read {
            sources: editor.sources()
        }
        .writable(&occurrence)
    );
    assert!(!Edit::new(editor.session()).replace(&occurrence, text::value("denied")));
    assert!(Rc::ptr_eq(&document, &editor.model.doc));
    assert!(!editor.model.history.can_undo());
}

#[test]
fn inserting_a_list_sibling_under_a_jump_preserves_the_occurrence() {
    let (mut editor, source, occurrence) = fixture(Value::list([text::value("first")]));
    let position = editor
        .sources()
        .resolve_path(&source)
        .unwrap()
        .as_list()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    let position = gid::position::between(Some(&position), None).unwrap();
    let item = [occurrence.clone(), vec![Step::Element(position.clone())]].concat();
    Edit::new(editor.session()).select(&crate::test_document(), &item);
    let selected = editor.model.selection.as_ref().unwrap();
    assert_eq!(&selected.path()[..occurrence.len()], occurrence);
    assert_eq!(
        selected.location(),
        Some(Location::root(
            [source.clone(), vec![gid::Step::Element(position)]].concat()
        ))
    );
    assert!(crate::completion::commit(
        &mut editor.session(),
        text::value("second"),
        None,
        Some(crate::selection::at(&[], crate::selection::payload::edge()))
    ));
    let values: Vec<_> = editor
        .sources()
        .resolve_path(&source)
        .unwrap()
        .as_list()
        .unwrap()
        .values()
        .cloned()
        .collect();
    assert_eq!(values, [text::value("first"), text::value("second")]);
}

/// An editor whose root holds `cell` in a field, `cell`'s definition being a
/// record with "inside" in a field: those two fields and the cell.
fn referenced_cell() -> (crate::Editor, [gid::CellId; 3]) {
    let (field, inner, cell) = (new_cell_id(), new_cell_id(), new_cell_id());
    let mut cells = Cells::new();
    cells.set_value(cell, Value::record([(inner, text::value("inside"))]));
    let editor = crate::test_editor(Document {
        root: Some(Value::record([(field, cell.into())])),
        cells,
    });
    (editor, [field, inner, cell])
}

#[test]
fn following_a_cell_starts_a_location_at_its_definition() {
    let (mut editor, [field, inner, cell]) = referenced_cell();
    let reference = vec![Step::Key(new_cell_id()), Step::Computed, Step::Key(field)];
    let inside = [
        reference.clone(),
        vec![
            Step::jump(Location::in_cell(cell, Resolution::Document, vec![])),
            Step::Key(inner),
        ],
    ]
    .concat();
    assert!(
        crate::occurrence::location(&reference).is_none(),
        "the reference is computed"
    );
    assert!(!Edit::new(editor.session()).replace(&reference, text::value("denied")));
    assert_eq!(read(&editor, &inside), Some(&text::value("inside")));
    assert!(Edit::new(editor.session()).replace(&inside, text::value("changed")));
    assert_eq!(
        editor.model.doc.cells.value(cell),
        Some(&Value::record([(inner, text::value("changed"))]))
    );
}

#[test]
fn a_route_through_a_cell_and_its_definitions_location_are_the_same_place() {
    let (editor, [field, inner, cell]) = referenced_cell();
    let routed = crate::occurrence::of(&[
        gid::Step::Key(field),
        gid::Step::Follow(Resolution::Document),
        gid::Step::Key(inner),
    ]);
    let jumped = vec![
        Step::Key(field),
        Step::jump(Location::in_cell(cell, Resolution::Document, vec![])),
        Step::Key(inner),
    ];
    let [routed_location, jumped_location] =
        [&routed, &jumped].map(|path| crate::occurrence::location(path).unwrap());
    assert_ne!(routed_location, jumped_location);
    let sources = editor.sources();
    assert_eq!(
        routed_location.stored(&sources),
        jumped_location.stored(&sources)
    );
    assert!(
        crate::occurrence::location(&[routed.clone(), vec![Step::Computed]].concat()).is_none()
    );
    assert_eq!(read(&editor, &routed), Some(&text::value("inside")));
    assert_eq!(read(&editor, &jumped), Some(&text::value("inside")));
}
