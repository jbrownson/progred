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
