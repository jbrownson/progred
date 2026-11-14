use super::*;
use crate::driver::Driver;
use crate::libraries::layout::vocabulary as l;
use gid::{Cells, Document, Step};

/// A document of a fraction, `{isa: fraction, over: 1, under: 2}`, whose
/// kind's def draws it with `projection`, where it has one.
fn fraction(projection: Option<Value>) -> (Driver, [CellId; 4]) {
    let [fraction, over, under, half] = std::array::from_fn(|_| gid::new_cell_id());
    let mut cells = Cells::new();
    for (cell, spelling) in [(over, "over"), (under, "under")] {
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
            ]
            .into_iter()
            .chain(projection.map(|projection| (PROJECTION, projection))),
        ),
    );
    let root = Value::record([(
        half,
        ::grap::isa::record(
            fraction,
            [
                (over, crate::libraries::f64::value(1.0)),
                (under, crate::libraries::f64::value(2.0)),
            ],
        ),
    )]);
    let driver = Driver::new(
        crate::test_editor(Document {
            root: Some(root),
            cells,
        }),
        Driver::VIEWPORT,
    );
    (driver, [fraction, over, under, half])
}

#[test]
fn a_record_is_drawn_by_its_kinds_def() {
    let template = |parts: &[CellId]| {
        ::grap::isa::record(
            l::kind::COL,
            [(
                l::CHILDREN,
                Value::list(parts.iter().map(|part| layout::template::part(*part))),
            )],
        )
    };
    let (mut driver, [fraction_cell, over, under, half]) = fraction(None);
    // With no projection, the record is drawn as a record, its kind shown.
    assert!(driver.drawn(&[Step::Key(half), Step::Key(::grap::isa::ISA)]));
    let mut doc = (*driver.editor().model.doc).clone();
    let mut def = doc
        .cells
        .value(fraction_cell)
        .unwrap()
        .as_record()
        .unwrap()
        .clone();
    def.set(PROJECTION, template(&[over, under]));
    doc.cells.set_value(fraction_cell, Value::Record(def));
    driver.runner.editor.model.doc = Rc::new(doc);
    driver.present();
    // The def's projection draws the parts it names, one above the other.
    let [top, bottom] = [over, under].map(|part| driver.rect(&[Step::Key(half), Step::Key(part)]));
    assert!(top.y1 <= bottom.y0, "the parts stack");
    assert!(!driver.drawn(&[Step::Key(half), Step::Key(::grap::isa::ISA)]));
    // Switched off in a view, the def draws nothing and the record is a record.
    driver
        .runner
        .editor
        .model
        .workspace
        .show(None, crate::workspace::Shown::AllBut(vec![super::ID]));
    driver.present();
    assert!(driver.drawn(&[Step::Key(half), Step::Key(::grap::isa::ISA)]));
}

#[test]
fn a_def_that_declines_passes_its_record_on() {
    // A projection that isn't a template or a function draws nothing, so the
    // record is drawn as it otherwise would be.
    let (driver, [_, _, _, half]) = fraction(Some(Value::list([])));
    assert!(driver.drawn(&[Step::Key(half), Step::Key(::grap::isa::ISA)]));
}
