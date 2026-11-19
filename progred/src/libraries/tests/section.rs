use super::*;
use crate::driver::Driver;
use crate::libraries::f64;
use gid::{Document, Value};
use kurbo::Point;

/// A section of `body` named `spelling`.
pub(crate) fn record(spelling: &str, body: gid::Value) -> gid::Value {
    ::grap::isa::record(
        SECTION,
        [
            (
                name::vocabulary::NAME,
                crate::libraries::text::value(spelling),
            ),
            (BODY, body),
        ],
    )
}

/// A document that is a section named "Numbers" over a list of two.
fn numbers() -> Driver {
    Driver::new(
        crate::test_editor(Document {
            root: Some(record(
                "Numbers",
                Value::list([f64::value(1.0), f64::value(2.0)]),
            )),
            cells: Default::default(),
        }),
        Driver::VIEWPORT,
    )
}

/// Click the section's disclosure mark, at the start of its heading.
fn click_mark(driver: &mut Driver) {
    let rect = driver.rect::<Step>(&[]);
    let point = Point::new(rect.x0 + 3.0, rect.y0 + 6.0);
    driver.click_at(point);
}

#[test]
fn a_sections_mark_folds_its_body_and_selects_it() {
    let mut driver = numbers();
    let body = [Step::Key(BODY)];
    assert!(driver.drawn(&body));
    assert!(driver.drawn(&[Step::Key(name::vocabulary::NAME)]));
    let before = driver.editor().model.doc.clone();
    click_mark(&mut driver);
    assert!(!driver.drawn(&body));
    assert_eq!(driver.selection(), crate::occurrence::of(&[]));
    assert!(
        Rc::ptr_eq(&before, &driver.editor().model.doc),
        "folding changes how it's seen, not the document"
    );
    // Folding is undone and redone as any change to how it's seen is.
    assert!(driver.undo());
    assert!(driver.drawn(&body));
    assert!(driver.redo());
    assert!(!driver.drawn(&body));
    click_mark(&mut driver);
    assert!(driver.drawn(&body));
}

#[test]
fn a_folded_section_opens_around_the_selection() {
    let mut driver = numbers();
    click_mark(&mut driver);
    let body = [Step::Key(BODY)];
    assert!(!driver.drawn(&body));
    // Arriving inside it, as by search or undo, shows it.
    driver
        .runner
        .editor
        .model
        .selection
        .replace(crate::selection::Selection::edge(
            &driver.runner.editor.model.workspace.document,
            crate::occurrence::of(&body),
        ));
    driver.present();
    assert!(driver.drawn(&body));
}

#[test]
fn a_section_goes_where_any_value_does() {
    let label = gid::new_cell_id();
    let driver = Driver::new(
        crate::test_editor(Document {
            root: Some(Value::record([(
                label,
                Value::list([record("Inner", f64::value(3.0))]),
            )])),
            cells: Default::default(),
        }),
        Driver::VIEWPORT,
    );
    let position = driver
        .value(&[Step::Key(label)])
        .as_list()
        .unwrap()
        .keys()
        .next()
        .unwrap()
        .clone();
    assert!(driver.drawn(&[Step::Key(label), Step::Element(position), Step::Key(BODY)]));
}

#[test]
fn an_empty_place_offers_a_new_section_its_name_typed_next() {
    use crate::libraries::notebook::vocabulary::ENTRIES;
    let mut driver = Driver::new(crate::test_editor(Document::default()), Driver::VIEWPORT);
    driver.click::<Step>(&[]);
    driver.choose("notebook");
    driver.choose("section");
    driver.type_text("Notes");
    let section = driver.element(&[Step::Key(ENTRIES)], 0, &[]);
    assert_eq!(
        driver.value(&section),
        ::grap::isa::record(
            SECTION,
            [(
                name::vocabulary::NAME,
                crate::libraries::text::value("Notes")
            )]
        )
    );
}

#[test]
fn space_folds_a_selected_sections_body() {
    let mut driver = numbers();
    let body = [Step::Key(BODY)];
    driver.select::<Step>(&[]);
    assert!(driver.keystroke(
        ui_events::keyboard::Key::Character(" ".into()),
        ui_events::keyboard::Modifiers::empty()
    ));
    assert!(!driver.drawn(&body));
    assert!(driver.keystroke(
        ui_events::keyboard::Key::Character(" ".into()),
        ui_events::keyboard::Modifiers::empty()
    ));
    assert!(driver.drawn(&body));
}
