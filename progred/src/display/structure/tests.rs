//! Lists and records, edited as a person would.

use crate::driver::Driver;
use crate::libraries::{blob, name, text};
use ui_events::keyboard::NamedKey;

#[test]
fn list_insertion_through_a_comma_and_whole_list_selection() {
    let (doc, _) = crate::gid_text::parse(r#"{"root": ["apples", "pears", "plums"]}"#).unwrap();
    let mut driver = Driver::new(
        crate::test_editor_with_stack(
            doc,
            crate::stack::load_selected(&[name::ID, text::ID, blob::ID]).unwrap(),
        ),
        kurbo::Size::new(900.0, 600.0),
    );
    driver.click_between(&[], 0);
    driver.type_text("\"peaches\"");
    driver.press(NamedKey::Enter);
    assert_eq!(
        driver
            .value(&[])
            .as_list()
            .unwrap()
            .values()
            .filter_map(text::read)
            .collect::<Vec<_>>(),
        ["apples", "peaches", "pears", "plums"]
    );
    let list = driver.rect(&[]);
    driver.click_at(kurbo::Point::new(list.x0 + 2.0, list.center().y));
    assert!(driver.selection().is_empty(), "{:?}", driver.selection());
}
