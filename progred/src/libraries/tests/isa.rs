use super::*;
use crate::driver::Driver;
use crate::libraries::def::vocabulary::{DEF, DEFS, FIELDS, LIBRARY};
use gid::{Cells, Document, Step, Value};
use ui_events::keyboard::{Key, Modifiers, NamedKey};

#[test]
fn an_empty_isa_offers_the_kinds_the_documents_library_defines() {
    let [fraction, over, half] = std::array::from_fn(|_| gid::new_cell_id());
    let mut cells = Cells::new();
    cells.set_value(over, name::record("over", []));
    cells.set_value(
        fraction,
        ::grap::isa::record(
            DEF,
            [
                (
                    name::vocabulary::NAME,
                    crate::libraries::text::value("fraction"),
                ),
                (FIELDS, Value::list([over.into()])),
            ],
        ),
    );
    // A record whose kind is still to be said.
    let unfinished = gid::Record::default().with(over, crate::libraries::f64::value(1.0));
    let root = ::grap::isa::record(
        LIBRARY,
        [
            (DEFS, Value::list([fraction.into()])),
            (half, Value::Record(unfinished)),
        ],
    );
    let mut driver = Driver::new(
        crate::test_editor(Document {
            root: Some(root),
            cells,
        }),
        Driver::VIEWPORT,
    );
    // Opening a place for the half's kind, and typing, offers the kind.
    driver.select(&[Step::Key(half)]);
    let isa = [Step::Key(half), Step::Key(vocabulary::ISA)];
    driver
        .runner
        .editor
        .model
        .selection
        .replace(crate::selection::pending_value(
            &driver.runner.editor.model.workspace.document,
            crate::occurrence::of(&isa),
        ));
    driver.present();
    driver.type_text("frac");
    driver.keystroke(Key::Named(NamedKey::Enter), Modifiers::empty());
    assert_eq!(driver.value(&isa), Value::from(fraction));
}

/// A record of a kind whose def lists `over` and `under`, named "half", with
/// `under` and a field the def doesn't list, `note`.
fn half() -> (Driver, [gid::CellId; 3]) {
    let [fraction, over, under, note] = std::array::from_fn(|_| gid::new_cell_id());
    let mut cells = Cells::new();
    for (cell, spelling) in [(over, "over"), (under, "under"), (note, "note")] {
        cells.set_value(cell, name::record(spelling, []));
    }
    cells.set_value(
        fraction,
        ::grap::isa::record(
            DEF,
            [
                (
                    name::vocabulary::NAME,
                    crate::libraries::text::value("fraction"),
                ),
                (FIELDS, Value::list([over.into(), under.into()])),
            ],
        ),
    );
    let root = ::grap::isa::record(
        fraction,
        [
            (
                name::vocabulary::NAME,
                crate::libraries::text::value("half"),
            ),
            (note, crate::libraries::text::value("a note")),
            (under, crate::libraries::f64::value(2.0)),
        ],
    );
    let driver = Driver::new(
        crate::test_editor(Document {
            root: Some(root),
            cells,
        }),
        Driver::VIEWPORT,
    );
    (driver, [over, under, note])
}

#[test]
fn a_record_reads_by_its_kind_and_name_then_its_kinds_fields_then_others() {
    let (driver, [over, under, note]) = half();
    let rect = |key| driver.rect(&[Step::Key(key)]);
    // Reading order: on an earlier line, or earlier on the same one.
    let before = |a: kurbo::Rect, b: kurbo::Rect| {
        a.y1 <= b.y0 || (a.y0 < b.y1 && b.y0 < a.y1 && a.x1 <= b.x0)
    };
    let order = [
        rect(vocabulary::ISA),
        rect(name::vocabulary::NAME),
        // The def's fields in its order, the missing one a place to fill.
        rect(over),
        rect(under),
        rect(note),
    ];
    for pair in order.windows(2) {
        assert!(before(pair[0], pair[1]), "{order:?}");
    }
}

#[test]
fn a_field_being_added_offers_the_kinds_fields_the_record_lacks() {
    let (mut driver, [over, ..]) = half();
    driver.select::<Step>(&[]);
    assert!(driver.keystroke(Key::Named(NamedKey::Enter), crate::test_support::command()));
    assert_eq!(driver.picker_detail("over").as_deref(), Some("field"));
    assert!(
        driver.picker_detail("under").is_none(),
        "the record has it already"
    );
    driver.choose("over");
    driver.type_text("1");
    driver.press(NamedKey::Enter);
    assert_eq!(
        driver.value(&[Step::Key(over)]),
        crate::libraries::f32::value(1.0)
    );
}

#[test]
fn a_kind_that_lists_a_name_shows_a_place_for_it() {
    let [kind, over] = std::array::from_fn(|_| gid::new_cell_id());
    let mut cells = Cells::new();
    cells.set_value(over, name::record("over", []));
    cells.set_value(
        kind,
        ::grap::isa::record(
            DEF,
            [(
                FIELDS,
                Value::list([name::vocabulary::NAME.into(), over.into()]),
            )],
        ),
    );
    let mut driver = Driver::new(
        crate::test_editor(Document {
            root: Some(::grap::isa::record(kind, [])),
            cells,
        }),
        Driver::VIEWPORT,
    );
    let name = [Step::Key(name::vocabulary::NAME)];
    // The name is in the heading, a place to type one, and not among the
    // fields after it.
    assert!(driver.drawn(&name));
    assert!(driver.rect(&name).x1 <= driver.rect(&[Step::Key(over)]).x0);
    driver.select(&name);
    driver.type_text("half");
    assert_eq!(driver.value(&name), crate::libraries::text::value("half"));
}

#[test]
fn a_def_without_a_name_shows_a_place_for_one() {
    let driver = Driver::new(
        crate::test_editor(Document {
            root: Some(::grap::isa::record(DEF, [])),
            cells: Cells::new(),
        }),
        Driver::VIEWPORT,
    );
    assert!(driver.drawn(&[Step::Key(name::vocabulary::NAME)]));
}
