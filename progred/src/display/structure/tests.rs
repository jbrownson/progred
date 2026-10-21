//! Lists and records, edited as a person would.

use crate::driver::Driver;
use crate::libraries::{name, text};
use ui_events::keyboard::NamedKey;

#[test]
fn list_insertion_through_a_comma_and_whole_list_selection() {
    let (doc, _) = crate::gid_text::parse(r#"{"root": ["apples", "pears", "plums"]}"#).unwrap();
    let mut driver = Driver::new(
        crate::test_editor_with_stack(
            doc,
            crate::stack::load_selected(&[name::ID, text::ID]).unwrap(),
        ),
        Driver::VIEWPORT,
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
    let list = driver.rect::<gid::Step>(&[]);
    driver.click_at(kurbo::Point::new(list.x0 + 2.0, list.center().y));
    assert!(driver.selection().is_empty(), "{:?}", driver.selection());
}

#[test]
fn cmd_clicking_a_label_picks_the_label_not_its_value() {
    use crate::frame::Hovered;
    use crate::hover::Hover;
    use crate::occurrence::Step;
    use gid::{Document, Value, new_cell_id};

    let label_at = |driver: &Driver, key| {
        driver.locate(Hovered::Tree(Hover::Label(crate::occurrence::Trail::from(
            [Step::Key(key)],
        ))))
    };

    // A new record's first label, its value still to come.
    let mut driver = Driver::new(crate::test_editor(Document::default()), Driver::VIEWPORT);
    driver.click::<Step>(&[]);
    driver.type_text("{");
    driver.choose("new cell");
    let [Step::Key(label)] = driver.selection()[..] else {
        panic!("the new field's value is pending")
    };
    driver.pick_at(label_at(&driver, label));
    assert_eq!(driver.value(&[]), Value::record([(label, label.into())]));

    // A label whose field has a value.
    let [x, y] = [new_cell_id(), new_cell_id()];
    let mut driver = Driver::of(Value::record([
        (x, new_cell_id().into()),
        (y, Value::list([])),
    ]));
    driver.click_at(label_at(&driver, x));
    assert_eq!(
        driver.selection(),
        [Step::Key(x)],
        "a click selects the field"
    );
    driver.select(&[Step::Key(y)]);
    driver.type_text("q");
    assert_eq!(driver.stage(), Some(crate::selection::Stage::Pending));
    driver.pick_at(label_at(&driver, x));
    assert_eq!(driver.value(&[gid::Step::Key(y)]), x.into());
}
