use super::*;
use crate::libraries::{
    control, line_edit, path as path_data, selection as selection_data, site, text,
};
use gid::{Cells, Document, new_cell_id};

fn path() -> Path {
    vec![Step::Key(new_cell_id())]
}

impl Scope {
    fn jump(&self, occurrence: Path, document: Path) -> Self {
        self.with_conject(occurrence, document, crate::display::Conject::descend())
    }
}

fn fixture(value: Value) -> (Editor, Root, Path, Path, Scope) {
    let source = path();
    let occurrence = path();
    let Step::Key(field) = source[0] else {
        unreachable!()
    };
    let editor = crate::test_editor(Document {
        root: Some(Value::record([(field, value)])),
        cells: Cells::new(),
    });
    let root = editor.model.workspace.document_root().clone();
    let scope = Scope::default().jump(occurrence.clone(), source.clone());
    (editor, root, source, occurrence, scope)
}

fn line(spelling: &str) -> crate::display::LineEdit {
    crate::display::LineEdit {
        text: spelling.into(),
        placeholder: None,
        update: line_edit::native(text::edit),
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
    let inner = scope.jump(
        second.clone(),
        scope.source(&inner_source).unwrap().into_owned(),
    );
    let expected = [source.clone(), vec![child]].concat();
    assert_eq!(inner.source(&second).unwrap().as_ref(), expected);
    assert_eq!(inner.source(&first).unwrap().as_ref(), source);
    assert_eq!(inner.source(&sibling).unwrap().as_ref(), sibling);
    assert!(matches!(
        Scope::default().source(&source),
        Some(Cow::Borrowed(_))
    ));
}

#[test]
fn shared_source_edits_keep_selection_and_annotations_on_the_chosen_occurrence() {
    let (mut editor, root, source, first, a) = fixture(text::value("before"));
    let second = path();
    let b = Scope::default().jump(second.clone(), source.clone());
    let annotation = text::value("local state");
    a.open(Access::new(&mut editor))
        .annotate(&root, &first, annotation.clone());
    b.open(Access::new(&mut editor)).select(&root, &second);
    assert!(
        b.open(Access::new(&mut editor))
            .edit_line(&root, &second, &line("before"), &|edit| {
                edit.state.set_text("after");
                true
            })
    );
    assert_eq!(
        a.read(&editor.sources(), &first),
        Some(&text::value("after"))
    );
    assert_eq!(
        b.read(&editor.sources(), &second),
        Some(&text::value("after"))
    );
    assert_eq!(editor.model.selection.as_ref().unwrap().path(), second);
    let annotations = &editor.model.workspace.view(&root).unwrap().annotations;
    assert_eq!(annotations.at(&first), Some(&annotation));
    assert_eq!(annotations.at(&second), None);
    assert_eq!(annotations.at(&source), None);
    assert!(editor.model.step_history(true));
    assert_eq!(
        b.read(&editor.sources(), &second),
        Some(&text::value("before"))
    );
    let selected = editor.model.selection.as_ref().unwrap();
    assert!(selected.scope().same_location(&b, selected.path()));
    assert!(
        b.open(Access::new(&mut editor))
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
    scope
        .open(Access::new(&mut editor))
        .select(&root, &occurrence);
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
    assert!(selected.scope().same_location(&scope, selected.path()));
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
    scope
        .open(Access::new(&mut editor))
        .select(&root, &occurrence);
    assert_eq!(
        editor
            .model
            .selection
            .as_ref()
            .unwrap()
            .stage(&editor.sources()),
        selection::Stage::Pending
    );
    assert!(editor.commit_completion(
        text::value("inserted"),
        None,
        Some(Rc::new(|view, path, changes| {
            assert_eq!(view.resolve_path(path), Some(&text::value("inserted")));
            changes.select(path.to_vec(), selection_data::edge());
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
    assert!(selected.scope().same_location(&scope, selected.path()));
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
                            (site::vocabulary::VALUE, selection_data::edge()),
                        ],
                    ),
                ]),
            )],
        ),
    );
    assert!(scope.open(Access::new(&mut editor)).grap(
        root.clone(),
        occurrence.clone(),
        handler,
        Value::record([])
    ));
    let selected = editor.model.selection.as_ref().unwrap();
    assert_eq!(selected.path(), occurrence);
    assert_eq!(selected.source_path().unwrap().as_ref(), source);
    assert!(selected.scope().same_location(&scope, selected.path()));
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
    assert!(editor.set_collapsed(&root, &first, false, Some(true)));
    let annotations = &editor.model.workspace.view(&root).unwrap().annotations;
    assert!(crate::annotations::collapsed(annotations, &first, false));
    assert!(!crate::annotations::collapsed(annotations, &second, false));
    assert!(Rc::ptr_eq(&document, &editor.model.doc));
    assert!(editor.model.step_history(true));
    assert!(editor.set_collapsed(&root, &second, false, Some(true)));
}

#[test]
fn gesture_writes_share_the_scope_and_one_undo_step() {
    let (mut editor, root, source, occurrence, scope) = fixture(text::value("before"));
    let mut run = crate::gesture::scoped_value_edit(root, occurrence, scope.clone());
    assert!(run.select(&mut editor));
    assert!(run.write(&mut editor, text::value("first")));
    assert!(run.write(&mut editor, text::value("second")));
    assert!(!run.write(&mut editor, text::value("second")));
    assert!(editor.model.step_history(true));
    assert_eq!(
        editor.sources().resolve_path(&source),
        Some(&text::value("before"))
    );
    let selected = editor.model.selection.as_ref().unwrap();
    assert!(selected.scope().same_location(&scope, selected.path()));
    assert!(!editor.model.history.can_undo());
}

#[test]
fn redirected_library_values_remain_read_only_and_failed_writes_do_not_detach() {
    let mut editor = crate::test_editor(Document {
        root: Some(text::ID.into()),
        cells: Cells::new(),
    });
    let source = vec![Step::Follow(gid::Resolution::Library(text::ID))];
    let occurrence = path();
    let scope = Scope::default().jump(occurrence.clone(), source.clone());
    let document = editor.model.doc.clone();
    assert!(scope.read(&editor.sources(), &occurrence).is_some());
    assert!(!scope.writable(&editor.sources(), &occurrence));
    assert!(
        !scope
            .open(Access::new(&mut editor))
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
    scope.open(Access::new(&mut editor)).select(&root, &item);
    let selected = editor.model.selection.as_ref().unwrap();
    assert_eq!(&selected.path()[..1], occurrence);
    assert!(selected.scope().same_location(&scope, selected.path()));
    assert!(editor.commit_completion(
        text::value("second"),
        None,
        Some(selection_data::at(&[], selection_data::edge()))
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
