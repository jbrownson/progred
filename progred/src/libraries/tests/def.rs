use super::*;
use crate::driver::Driver;
use crate::libraries::layout::vocabulary as l;
use gid::{Cells, Document, Step, Value};

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
    // A field the record lacks is a place to fill.
    let mut doc = (*driver.editor().model.doc).clone();
    let without = driver
        .value(&[Step::Key(half)])
        .as_record()
        .unwrap()
        .with(under, None);
    doc.root = Some(Value::record([(half, Value::Record(without))]));
    driver.runner.editor.model.doc = Rc::new(doc);
    driver.present();
    assert!(driver.drawn(&[Step::Key(half), Step::Key(under)]));
    driver.select(&[Step::Key(half), Step::Key(under)]);
    driver.type_text("\"two");
    driver.keystroke(
        ui_events::keyboard::Key::Named(ui_events::keyboard::NamedKey::Enter),
        ui_events::keyboard::Modifiers::empty(),
    );
    assert_eq!(
        driver.value(&[Step::Key(half), Step::Key(under)]),
        crate::libraries::text::value("two")
    );
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
    // A part the record lacks is drawn by the template as a place to fill.
    let mut doc = (*driver.editor().model.doc).clone();
    let without = driver
        .value(&[Step::Key(half)])
        .as_record()
        .unwrap()
        .with(under, None);
    doc.root = Some(Value::record([(half, Value::Record(without.clone()))]));
    driver.runner.editor.model.doc = Rc::new(doc);
    driver.present();
    assert!(driver.drawn(&[Step::Key(half), Step::Key(under)]));
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

#[test]
fn a_documents_defs_are_those_of_the_library_its_root_is() {
    let [fraction, angle, data] = std::array::from_fn(|_| gid::new_cell_id());
    let library = |defs: Vec<Value>| {
        ::grap::isa::record(
            LIBRARY,
            [(DEFS, Value::list(defs)), (data, Value::list([]))],
        )
    };
    assert_eq!(
        document_defs(Some(&library(vec![fraction.into(), angle.into()]))),
        [fraction, angle]
    );
    // A root that isn't a library has none, and a library further in isn't
    // looked for.
    assert!(document_defs(None).is_empty());
    assert!(
        document_defs(Some(&Value::record([(
            data,
            library(vec![fraction.into()])
        )])))
        .is_empty()
    );
}

/// A document whose root is a library defining `defs`, with `items`, an
/// empty list of data, beside them.
fn library_of(defs: Vec<Value>, cells: Cells, items: CellId) -> Driver {
    let root = ::grap::isa::record(
        LIBRARY,
        [(DEFS, Value::list(defs)), (items, Value::list([]))],
    );
    Driver::new(
        crate::test_editor(Document {
            root: Some(root),
            cells,
        }),
        Driver::VIEWPORT,
    )
}

/// The first element of the list at `path`, a cell, and its definition.
fn first_cell(driver: &Driver, path: &[Step]) -> (CellId, Value) {
    let cell = driver
        .value(path)
        .as_list()
        .and_then(|list| list.values().next().cloned())
        .and_then(|value| value.as_cell())
        .expect("the list holds a cell");
    let definition = driver
        .editor()
        .sources()
        .resolve(cell)
        .map(|definition| definition.value.clone())
        .expect("the cell is defined");
    (cell, definition)
}

#[test]
fn a_new_def_and_its_fields_are_made_where_they_go() {
    use ui_events::keyboard::{Key, NamedKey};
    let mut driver = library_of(vec![], Cells::new(), gid::new_cell_id());
    // A place among the library's defs offers a new def, named as typed.
    let defs = [Step::Key(DEFS)];
    driver.select(&defs);
    assert!(driver.keystroke(Key::Named(NamedKey::Enter), crate::test_support::command()));
    driver.type_text("shape");
    assert_eq!(driver.picker_detail("shape").as_deref(), Some("new def"));
    driver.press(NamedKey::Enter);
    let (shape, definition) = first_cell(&driver, &defs);
    assert_eq!(
        definition,
        name::record(
            "shape",
            [
                (::grap::isa::ISA, Value::from(DEF)),
                (FIELDS, Value::list([])),
            ]
        )
    );
    // Its fields are declared there, each a new cell named as typed.
    let fields = [
        Step::Key(DEFS),
        Step::Element(first_position(&driver, &defs)),
        Step::Follow,
        Step::Key(FIELDS),
    ];
    driver.select(&fields);
    assert!(driver.keystroke(Key::Named(NamedKey::Enter), crate::test_support::command()));
    driver.type_text("radius");
    assert_eq!(driver.picker_detail("radius").as_deref(), Some("new field"));
    driver.press(NamedKey::Enter);
    let (radius, definition) = first_cell(&driver, &fields);
    assert_eq!(definition, name::record("radius", []));
    assert_eq!(
        document_defs(driver.editor().model.doc.root.as_ref()),
        [shape]
    );
    assert_eq!(
        Def::read(
            &driver
                .editor()
                .sources()
                .resolve(shape)
                .unwrap()
                .value
                .into()
        )
        .unwrap()
        .fields,
        [radius]
    );
}

/// The position of the first element of the list at `path`.
fn first_position(driver: &Driver, path: &[Step]) -> gid::Position {
    driver
        .value(path)
        .as_list()
        .and_then(|list| list.keys().next().cloned())
        .expect("the list has an element")
}

#[test]
fn an_empty_place_offers_a_new_record_of_each_kind_the_library_defines() {
    use ui_events::keyboard::{Key, NamedKey};
    let [fraction, items] = std::array::from_fn(|_| gid::new_cell_id());
    let mut cells = Cells::new();
    cells.set_value(
        fraction,
        name::record("fraction", [(::grap::isa::ISA, Value::from(DEF))]),
    );
    let mut driver = library_of(vec![fraction.into()], cells, items);
    let list = [Step::Key(items)];
    driver.select(&list);
    assert!(driver.keystroke(Key::Named(NamedKey::Enter), crate::test_support::command()));
    driver.type_text("frac");
    assert!(
        driver
            .picker_entries()
            .is_some_and(|entries| entries.iter().any(|entry| entry == "new fraction"))
    );
    driver.type_text("q");
    assert!(
        !driver
            .picker_entries()
            .is_some_and(|entries| entries.iter().any(|entry| entry == "new fraction")),
        "a kind whose name what's typed doesn't begin isn't offered"
    );
    driver.press(NamedKey::Backspace);
    driver.choose("new fraction");
    let position = first_position(&driver, &list);
    assert_eq!(
        driver.value(&[Step::Key(items), Step::Element(position)]),
        ::grap::isa::record(fraction, [])
    );
}
