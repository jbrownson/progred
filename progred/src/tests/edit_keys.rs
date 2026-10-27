//! The editing keys, pressed as a person would.

use crate::driver::Driver;
use crate::libraries::{f64, text};
use crate::selection::Stage;
use crate::test_support::positions;
use gid::{Cells, Document, Path, Step, Value, new_cell_id};
use std::rc::Rc;
use ui_events::keyboard::{Key, Modifiers, NamedKey};

/// A document of three empty lists, the middle one selected.
fn three_lists() -> (Driver, [Path; 3]) {
    let root = Value::list([Value::list([]), Value::list([]), Value::list([])]);
    let paths: [Path; 3] = positions(&root)
        .into_iter()
        .map(|position| vec![Step::Element(position)])
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    let mut driver = Driver::of(root);
    driver.select(&paths[1]);
    (driver, paths)
}

#[test]
fn deleting_empties_a_place_then_removing_it_moves_toward_the_key() {
    for (key, lands) in [(NamedKey::Backspace, 0), (NamedKey::Delete, 2)] {
        let (mut driver, paths) = three_lists();
        driver.press(key);
        assert_eq!(
            driver.selection(),
            paths[1],
            "{key:?} keeps the place selected"
        );
        assert_eq!(driver.stage(), Some(Stage::Pending));
        assert_eq!(driver.value(&[]).as_list().map(|list| list.len()), Some(2));
        driver.press(key);
        assert_eq!(driver.selection(), paths[lands], "{key:?} removes the hole");
        assert_eq!(driver.stage(), Some(Stage::Edge));
    }
}

#[test]
fn removing_a_containers_only_hole_backs_out_or_moves_on() {
    let [list, next] = {
        let mut keys = [new_cell_id(), new_cell_id()];
        keys.sort();
        keys
    };
    for (key, lands) in [(NamedKey::Backspace, list), (NamedKey::Delete, next)] {
        let mut driver = Driver::of(Value::record([
            (list, Value::list([Value::list([])])),
            (next, text::value("next")),
        ]));
        let element = driver.element(&[Step::Key(list)], 0, &[]);
        driver.select(&element);
        driver.press(key);
        assert_eq!(driver.selection(), element, "{key:?} keeps the place");
        driver.press(key);
        assert_eq!(
            driver.selection(),
            [Step::Key(lands)],
            "{key:?} removes the hole"
        );
    }
}

#[test]
fn escape_returns_a_picker_to_where_it_was_opened() {
    let (mut driver, paths) = three_lists();
    let document = driver.editor().model.doc.clone();
    driver.press(NamedKey::Enter);
    assert_eq!(driver.stage(), Some(Stage::Pending));
    driver.press(NamedKey::Escape);
    assert_eq!(driver.selection(), paths[1]);
    assert_eq!(driver.stage(), Some(Stage::Edge));
    assert!(Rc::ptr_eq(&driver.editor().model.doc, &document));
    driver.press(NamedKey::Escape);
    assert!(driver.editor().model.selection.is_none());
}

#[test]
fn typing_over_a_value_replaces_it_and_escape_brings_it_back() {
    let (mut driver, paths) = three_lists();
    let document = driver.editor().model.doc.clone();
    driver.type_text("x");
    assert_eq!(driver.selection(), paths[1]);
    assert_eq!(driver.stage(), Some(Stage::Pending));
    assert!(
        Rc::ptr_eq(&driver.editor().model.doc, &document),
        "nothing changes until a commit"
    );
    assert!(
        driver.picker_entries().is_some(),
        "the hole shows its picker"
    );
    driver.press(NamedKey::Escape);
    assert_eq!(driver.selection(), paths[1]);
    assert_eq!(driver.stage(), Some(Stage::Edge));
    assert!(Rc::ptr_eq(&driver.editor().model.doc, &document));

    driver.type_text("{");
    if driver.stage() == Some(Stage::Pending) {
        driver.press(NamedKey::Enter);
    }
    assert_eq!(driver.value(&paths[1]), Value::record([]));
    assert!(driver.undo());
    assert_eq!(
        driver.value(&paths[1]),
        Value::list([]),
        "one undo step restores the replaced value"
    );
}

#[test]
fn enter_ends_typing_in_a_value_before_it_adds_anything_beside() {
    let x = new_cell_id();
    let mut driver = Driver::of(Value::record([(x, f64::value(2.0))]));
    driver.select(&[Step::Key(x)]);
    // "25e" is still becoming a number, so 25 is what was written.
    driver.type_text("5e");
    assert_eq!(driver.line(), Some("25e"));
    driver.press(NamedKey::Enter);
    assert_eq!(driver.selection(), [Step::Key(x)]);
    assert_eq!(driver.stage(), Some(Stage::Edge));
    assert!(driver.line().is_none(), "the typing ended");
    assert_eq!(
        driver.value(&[]),
        Value::record([(x, f64::value(25.0))]),
        "and nothing was added beside it"
    );
    // Enter on the value itself continues beside it, as before.
    driver.press(NamedKey::Enter);
    assert_eq!(driver.stage(), Some(Stage::Label));
}

#[test]
fn enter_on_a_value_that_fills_a_cell_continues_beside_its_reference() {
    let shared = new_cell_id();
    let items = new_cell_id();
    let program = new_cell_id();
    let mut cells = Cells::new();
    cells.set_value(shared, f64::value(7.0));
    cells.set_value(program, f64::value(1.0));
    let doc = Document {
        root: Some(Value::record([
            (items, Value::list([shared.into(), shared.into()])),
            (crate::libraries::grap::vocabulary::GRAP, program.into()),
        ])),
        cells,
    };
    let mut driver = Driver::new(crate::test_editor(doc), Driver::VIEWPORT);
    let first = driver.element(&[Step::Key(items)], 0, &[]);
    let follow = Step::Follow;
    // A new element pends beside the reference in its list; a new field
    // pends on the record holding the reference.
    for (value, expected, pending_at) in [
        (
            [first.as_slice(), &[follow.clone()]].concat(),
            Stage::Pending,
            2,
        ),
        (
            vec![Step::Key(crate::libraries::grap::vocabulary::GRAP), follow],
            Stage::Label,
            0,
        ),
    ] {
        driver.select(&value);
        driver.press(NamedKey::Enter);
        assert_eq!(driver.stage(), Some(expected));
        assert_eq!(driver.selection().len(), pending_at);
    }
}

#[test]
fn enter_beside_text_in_a_list_adds_an_element_but_nothing_within_the_text() {
    let mut driver = Driver::of(Value::list([text::value("a"), text::value("b")]));
    let first = driver.element(&[], 0, &[]);
    let enter = |driver: &mut Driver, modifiers| {
        driver.select(&first);
        driver.keystroke(Key::Named(NamedKey::Enter), modifiers)
    };
    // The list takes a new element after the text.
    assert!(enter(&mut driver, Modifiers::empty()));
    assert_eq!(driver.stage(), Some(Stage::Pending));
    assert!(matches!(
        (driver.selection().as_slice(), first.as_slice()),
        ([crate::occurrence::Step::Element(position)], [Step::Element(text)]) if position > text
    ));
    // The text, drawn as one piece, takes nothing within it.
    assert!(!enter(&mut driver, crate::test_support::command()));
    assert_eq!(driver.selection(), first);
}

#[test]
fn pasting_a_copied_value_over_a_number_or_text_replaces_it() {
    let mut driver = Driver::of(Value::list([
        f64::value(1.0),
        f64::value(2.0),
        text::value("hello"),
    ]));
    let chord = |driver: &mut Driver, key: &str| {
        driver.keystroke(Key::Character(key.into()), crate::test_support::command())
    };
    driver.select(&driver.element(&[], 0, &[]));
    assert!(chord(&mut driver, "c"));
    // The copy's text is its JSON, not something to type into the
    // selected value's editor.
    for index in [1, 2] {
        let element = driver.element(&[], index, &[]);
        driver.select(&element);
        assert!(chord(&mut driver, "v"));
        assert_eq!(driver.value(&element), f64::value(1.0));
    }
}
