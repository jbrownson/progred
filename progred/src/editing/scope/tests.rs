use super::*;
use crate::libraries::{grap::control, path as path_data, selection as selection_data, site, text};
use crate::location::Location;
use gid::{Cells, Document, new_cell_id};
use std::borrow::Cow;

fn path() -> Path {
    vec![Step::Key(new_cell_id())]
}

impl Scope {
    fn jump(&self, occurrence: Path, document: Path) -> Self {
        self.located(occurrence, Location::root(document))
    }
}

impl Edit<'_> {
    /// Runs the Grap handler `function` evaluates to at `path`.
    fn grap(&mut self, root: Root, path: Path, function: Value, event: Value) -> bool {
        let function =
            grap::evaluate(&function, &self.session.sources(), grap::DEFAULT_FUEL).result;
        self.grap_runtime(root, path, function, event)
    }
}

fn fixture(value: Value) -> (crate::Editor, Root, Path, Path, Scope) {
    let source = path();
    let occurrence = path();
    let Step::Key(field) = source[0] else {
        unreachable!()
    };
    let editor = crate::editor_of(Value::record([(field, value)]));
    let root = editor.model.workspace.document_root().clone();
    let scope = Scope::default().jump(occurrence.clone(), source.clone());
    (editor, root, source, occurrence, scope)
}

fn line(spelling: &str) -> crate::display::LineEdit {
    crate::display::LineEdit {
        text: spelling.into(),
        update: Rc::new(text::edit),
        prefix: String::new(),
        suffix: String::new(),
        family: Default::default(),
    }
}

#[test]
fn conjects_compose_and_the_identity_scope_borrows_its_path() {
    let (source, first, second, sibling) = (path(), path(), path(), path());
    let child = Step::Key(new_cell_id());
    let inner_source = [first.clone(), vec![child.clone()]].concat();
    let scope = Scope::default().jump(first.clone(), source.clone());
    let inner = scope.located(
        second.clone(),
        scope.location(&inner_source).unwrap().into_owned(),
    );
    let expected = [source.clone(), vec![child]].concat();
    assert_eq!(inner.location(&second), Some(Location::root(expected)));
    assert_eq!(inner.location(&first), Some(Location::root(source.clone())));
    assert_eq!(
        inner.location(&sibling),
        Some(Location::root(sibling.clone()))
    );
    assert!(matches!(
        Scope::default().location(&source),
        Some(Location {
            path: Cow::Borrowed(_),
            ..
        })
    ));
}

#[test]
fn detached_boundaries_do_not_fall_back() {
    let (prefix, document, outside) = (path(), path(), path());
    let detached = Scope::default()
        .jump(Vec::new(), document.clone())
        .detached(prefix.clone());
    assert!(detached.location(&prefix).is_none());
    assert_eq!(
        detached.location(&outside),
        Some(Location::root([document, outside.clone()].concat()))
    );
    let restored = detached.jump(prefix.clone(), outside.clone());
    assert_eq!(restored.location(&prefix), Some(Location::root(outside)));
}

#[test]
fn shared_source_edits_keep_selection_and_annotations_on_the_chosen_occurrence() {
    let (mut editor, root, source, first, a) = fixture(text::value("before"));
    let second = path();
    let b = Scope::default().jump(second.clone(), source.clone());
    let annotation = text::value("local state");
    a.open(editor.session())
        .annotate(&root, &first, annotation.clone());
    b.open(editor.session()).select(&root, &second);
    assert!(
        b.open(editor.session())
            .edit_line(&root, &second, &line("before"), &|edit| {
                edit.state.set_text("after");
                true
            })
    );
    assert_eq!(
        a.view(editor.sources()).resolve_path(&first),
        Some(&text::value("after"))
    );
    assert_eq!(
        b.view(editor.sources()).resolve_path(&second),
        Some(&text::value("after"))
    );
    assert_eq!(editor.model.selection.as_ref().unwrap().path(), second);
    let annotations = &editor.model.workspace.view(&root).unwrap().annotations;
    assert_eq!(annotations.at(&first), Some(&annotation));
    assert_eq!(annotations.at(&second), None);
    assert_eq!(annotations.at(&source), None);
    assert!(editor.model.step_history(true));
    assert_eq!(
        b.view(editor.sources()).resolve_path(&second),
        Some(&text::value("before"))
    );
    let selected = editor.model.selection.as_ref().unwrap();
    assert!(
        selected
            .scope()
            .same_location(&b, &editor.sources(), selected.path())
    );
    assert!(
        b.open(editor.session())
            .edit_line(&root, &second, &line("before"), &|edit| {
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
fn structural_paste_delete_and_undo_use_the_same_scope() {
    let (mut editor, root, source, occurrence, scope) = fixture(text::value("old"));
    scope.open(editor.session()).select(&root, &occurrence);
    assert!(editor.paste_value(text::value("pasted")));
    assert_eq!(
        editor.sources().resolve_path(&source),
        Some(&text::value("pasted"))
    );
    assert_eq!(editor.model.selection.as_ref().unwrap().path(), occurrence);
    assert!(editor.delete_selected_edge(Default::default()));
    assert_eq!(editor.sources().resolve_path(&source), None);
    assert!(editor.model.step_history(true));
    let selected = editor.model.selection.as_ref().unwrap();
    assert_eq!(selected.path(), occurrence);
    assert!(
        selected
            .scope()
            .same_location(&scope, &editor.sources(), selected.path())
    );
    assert_eq!(
        selected.value(&editor.sources()),
        Some(&text::value("pasted"))
    );
    assert!(editor.model.step_history(true));
    assert_eq!(
        editor.sources().resolve_path(&source),
        Some(&text::value("old"))
    );
}

#[test]
fn a_missing_scoped_child_uses_the_ordinary_completion_and_continuation() {
    let (mut editor, root, source, mut occurrence, scope) = fixture(Value::record([]));
    let field = new_cell_id();
    occurrence.push(Step::Key(field));
    scope.open(editor.session()).select(&root, &occurrence);
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
    let source = [source, vec![Step::Key(field)]].concat();
    assert_eq!(
        editor.sources().resolve_path(&source),
        Some(&text::value("inserted"))
    );
    let selected = editor.model.selection.as_ref().unwrap();
    assert_eq!(selected.path(), occurrence);
    assert!(
        selected
            .scope()
            .same_location(&scope, &editor.sources(), selected.path())
    );
    assert_eq!(selected.stage(&editor.sources()), selection::Stage::Edge);
}

#[test]
fn grap_handler_uses_the_scope_without_an_opaque_value() {
    let (mut editor, root, source, occurrence, scope) = fixture(text::value("data"));
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
    assert!(scope.open(editor.session()).grap(
        root.clone(),
        occurrence.clone(),
        handler,
        Value::record([])
    ));
    let selected = editor.model.selection.as_ref().unwrap();
    assert_eq!(selected.path(), occurrence);
    assert_eq!(selected.location(), Some(Location::root(source.as_slice())));
    assert!(
        selected
            .scope()
            .same_location(&scope, &editor.sources(), selected.path())
    );
    assert_eq!(
        editor
            .model
            .workspace
            .view(&root)
            .unwrap()
            .annotations
            .at(&occurrence),
        Some(&annotation)
    );
    assert_eq!(
        path_data::read(&path_data::value(&occurrence)),
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
    let (mut editor, root, _source, first, _scope) = fixture(Value::list([text::value("item")]));
    let second = path();
    let document = editor.model.doc.clone();
    assert!(
        editor
            .session()
            .set_folded(&root, &first, false, Some(true))
    );
    let annotations = &editor.model.workspace.view(&root).unwrap().annotations;
    assert!(crate::annotations::folded(annotations.at(&first), false));
    assert!(!crate::annotations::folded(annotations.at(&second), false));
    assert!(Rc::ptr_eq(&document, &editor.model.doc));
    assert!(editor.model.step_history(true));
    assert!(
        editor
            .session()
            .set_folded(&root, &second, false, Some(true))
    );
}

#[test]
fn gesture_writes_share_the_scope_and_one_undo_step() {
    let (mut editor, root, source, occurrence, scope) = fixture(text::value("before"));
    let mut run = Site::new(root, occurrence.into(), scope.clone()).edit_run();
    assert!(run.select(&mut editor.session()));
    assert!(run.write(&mut editor.session(), text::value("first")));
    assert!(run.write(&mut editor.session(), text::value("second")));
    assert!(!run.write(&mut editor.session(), text::value("second")));
    assert!(editor.model.step_history(true));
    assert_eq!(
        editor.sources().resolve_path(&source),
        Some(&text::value("before"))
    );
    let selected = editor.model.selection.as_ref().unwrap();
    assert!(
        selected
            .scope()
            .same_location(&scope, &editor.sources(), selected.path())
    );
    assert!(!editor.model.history.can_undo());
}

#[test]
fn redirected_library_values_remain_read_only_and_failed_writes_do_not_detach() {
    let mut editor = crate::editor_of(text::ID.into());
    let source = vec![Step::Follow(gid::Resolution::Library(text::ID))];
    let occurrence = path();
    let scope = Scope::default().jump(occurrence.clone(), source.clone());
    let document = editor.model.doc.clone();
    assert!(
        scope
            .view(editor.sources())
            .resolve_path(&occurrence)
            .is_some()
    );
    assert!(!scope.view(editor.sources()).writable(&occurrence));
    assert!(
        !scope
            .open(editor.session())
            .replace(&occurrence, text::value("denied"))
    );
    assert!(Rc::ptr_eq(&document, &editor.model.doc));
    assert!(!editor.model.history.can_undo());
}

#[test]
fn inserting_a_scoped_list_sibling_preserves_the_occurrence() {
    let (mut editor, root, source, occurrence, scope) =
        fixture(Value::list([text::value("first")]));
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
    let item = [occurrence.clone(), vec![Step::Element(position)]].concat();
    scope.open(editor.session()).select(&root, &item);
    let selected = editor.model.selection.as_ref().unwrap();
    assert_eq!(&selected.path()[..1], occurrence);
    assert!(
        selected
            .scope()
            .same_location(&scope, &editor.sources(), selected.path())
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

#[test]
fn following_a_cell_starts_a_location_at_its_definition() {
    let (occurrence, field) = (path(), Step::Key(new_cell_id()));
    let (cell, source) = (new_cell_id(), gid::Resolution::Document);
    let reference = [occurrence.clone(), vec![field.clone()]].concat();
    let definition = [reference.clone(), vec![Step::Follow(source)]].concat();
    let computed = Scope::default().detached(occurrence.clone());
    let followed = computed.in_cell(definition.clone(), cell, source);
    assert!(followed.descends());
    assert!(!computed.descends());
    assert!(
        followed.location(&reference).is_none(),
        "the reference is computed"
    );
    assert_eq!(
        followed.location(&definition),
        Some(Location::in_cell(cell, source, Vec::new()))
    );
    let inside = [definition, vec![field.clone()]].concat();
    assert_eq!(
        followed.location(&inside),
        Some(Location::in_cell(cell, source, vec![field]))
    );
    let elsewhere = path();
    assert_eq!(
        followed.location(&elsewhere),
        Some(Location::root(elsewhere.clone()))
    );
}

#[test]
fn a_route_through_a_cell_and_its_definitions_location_are_the_same_location() {
    let (field, inner, cell) = (new_cell_id(), new_cell_id(), new_cell_id());
    let mut cells = Cells::new();
    cells.set_value(cell, Value::record([(inner, text::value("inside"))]));
    let editor = crate::test_editor(Document {
        root: Some(Value::record([(field, cell.into())])),
        cells,
    });
    let follow = Step::Follow(gid::Resolution::Document);
    let definition = vec![Step::Key(field), follow.clone()];
    let within = [definition.clone(), vec![Step::Key(inner)]].concat();
    let restarted = Scope::default().in_cell(definition, cell, gid::Resolution::Document);
    assert_ne!(
        Scope::default().location(&within),
        restarted.location(&within)
    );
    assert!(Scope::default().same_location(&restarted, &editor.sources(), &within));
    assert!(!restarted.same_location(
        &Scope::default().detached(within.clone()),
        &editor.sources(),
        &within
    ));
    assert_eq!(
        restarted.view(editor.sources()).resolve_path(&within),
        Some(&text::value("inside"))
    );
}
