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
    assert_eq!(library.value(vocabulary::NAME).and_then(read), Some("name"));
}
