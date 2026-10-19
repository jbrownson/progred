use super::*;
use gid::new_cell_id;

#[test]
fn names_are_extensible_ordinary_record_data() {
    let mut fields = record("roof", []).as_record().unwrap().clone();
    fields.insert(new_cell_id(), text::value("anything"));
    assert_eq!(read(&Value::Record(fields)), Some("roof"));
    assert_eq!(read(&text::value("roof")), None);

    let empty = record("", []);
    assert_eq!(read(&empty), Some(""));
    assert_eq!(
        empty
            .as_record()
            .unwrap()
            .get(&vocabulary::NAME)
            .and_then(text::read),
        Some("")
    );
}

#[test]
fn the_name_relation_describes_itself_without_core_support() {
    let library = library::<crate::Editor>();
    assert_eq!(
        library.definitions.value(vocabulary::NAME).and_then(read),
        Some("name")
    );
}

#[test]
fn a_new_label_is_the_typed_text_as_a_cell_name_and_nothing_else() {
    use crate::display::{CompletionKind, CompletionRequest, CompletionScope};
    let ask = |kind, scope, query| {
        completions(&CompletionRequest {
            raw: false,
            query,
            kind,
            scope,
            path: &[],
            value_at: &|_| None,
            resolve: &|_| None,
            cells: &Vec::new,
            name: &|_| None,
            libraries: None,
        })
    };
    let [label] = ask(CompletionKind::Field, CompletionScope::Literal, "\"width\"")
        .unwrap()
        .try_into()
        .ok()
        .unwrap();
    assert_eq!(label.display, "width".into());
    assert_eq!(label.detail, Some("new label".into()));
    assert!(label.preview.is_none());
    assert!(ask(CompletionKind::Field, CompletionScope::Literal, " ").is_none());
    assert!(ask(CompletionKind::Value, CompletionScope::Literal, "width").is_none());
    assert!(ask(CompletionKind::Field, CompletionScope::Everything, "width").is_none());
}
