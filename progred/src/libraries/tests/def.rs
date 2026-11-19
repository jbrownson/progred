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

/// The defs of a document of `root` and `cells`, read as the editor reads
/// them, the loaded libraries' kinds among what it resolves.
fn defs_of(root: Value, cells: Cells) -> Vec<CellId> {
    let editor = crate::test_editor(Document {
        root: Some(root),
        cells,
    });
    let sources = editor.sources();
    document_defs(sources.root(), &|cell| Some(sources.resolve(cell)?.value))
}

#[test]
fn a_documents_defs_are_its_libraries_wherever_its_collections_hold_them() {
    use crate::libraries::notebook::vocabulary::{ENTRIES, kind::NOTEBOOK};
    let [fraction, angle, shared, data] = std::array::from_fn(|_| gid::new_cell_id());
    let library = |defs: Vec<Value>| ::grap::isa::record(LIBRARY, [(DEFS, Value::list(defs))]);
    let notebook =
        |entries: Vec<Value>| ::grap::isa::record(NOTEBOOK, [(ENTRIES, Value::list(entries))]);
    assert_eq!(
        defs_of(library(vec![fraction.into(), angle.into()]), Cells::new()),
        [fraction, angle]
    );
    // A notebook's entries, a section's body and the cells they hold are
    // looked through, in the document's order, notebooks within notebooks
    // too.
    let mut cells = Cells::new();
    cells.set_value(
        shared,
        crate::libraries::section::tests::record("Kinds", library(vec![angle.into()])),
    );
    let root = notebook(vec![
        library(vec![fraction.into()]),
        Value::list([]),
        notebook(vec![shared.into()]),
    ]);
    assert_eq!(defs_of(root, cells), [fraction, angle]);
    // So is a row's parts, as a document beside a preview is.
    let row = crate::libraries::editor::workspace::row([
        Value::record([]),
        notebook(vec![library(vec![fraction.into()])]),
    ]);
    assert_eq!(defs_of(row, Cells::new()), [fraction]);
    // Nothing else is: not a record's fields, nor a list's elements.
    for root in [
        Value::record([(data, library(vec![fraction.into()]))]),
        notebook(vec![Value::list([library(vec![fraction.into()])])]),
    ] {
        assert!(defs_of(root, Cells::new()).is_empty());
    }
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
    assert_eq!(driver.picker_kind("shape").as_deref(), Some("new def"));
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
    assert_eq!(driver.picker_kind("radius").as_deref(), Some("new field"));
    driver.press(NamedKey::Enter);
    let (radius, definition) = first_cell(&driver, &fields);
    assert_eq!(definition, name::record("radius", []));
    let doc = &driver.editor().model.doc;
    assert_eq!(
        defs_of(doc.root.clone().unwrap(), doc.cells.clone()),
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
            .is_some_and(|entries| entries.iter().any(|entry| entry == "fraction"))
    );
    driver.type_text("q");
    assert!(
        !driver
            .picker_entries()
            .is_some_and(|entries| entries.iter().any(|entry| entry == "fraction")),
        "a kind whose name what's typed doesn't begin isn't offered"
    );
    driver.press(NamedKey::Backspace);
    driver.choose("fraction");
    let position = first_position(&driver, &list);
    assert_eq!(
        driver.value(&[Step::Key(items), Step::Element(position)]),
        ::grap::isa::record(fraction, [])
    );
}

#[test]
fn an_empty_document_starts_as_a_notebook_of_a_library_its_first_def_made_next() {
    use crate::libraries::notebook::vocabulary::{ENTRIES, kind::NOTEBOOK};
    let mut driver = Driver::new(crate::test_editor(Document::default()), Driver::VIEWPORT);
    driver.click::<Step>(&[]);
    driver.choose("library");
    driver.type_text("shape");
    driver.press(ui_events::keyboard::NamedKey::Enter);
    let root = driver.value(&[]);
    assert_eq!(::grap::isa::of(root.as_record().unwrap()), Some(NOTEBOOK));
    let library = [
        Step::Key(ENTRIES),
        Step::Element(first_position(&driver, &[Step::Key(ENTRIES)])),
    ];
    assert_eq!(
        ::grap::isa::of(driver.value(&library).as_record().unwrap()),
        Some(LIBRARY)
    );
    let (shape, definition) = first_cell(&driver, &[&library[..], &[Step::Key(DEFS)]].concat());
    let doc = &driver.editor().model.doc;
    assert_eq!(defs_of(root.clone(), doc.cells.clone()), [shape]);
    assert_eq!(name::read(&definition), Some("shape"));
}

/// A record of a kind whose one field, `size`, offers what `offers` says.
fn sized(offers: Value) -> (Driver, CellId, CellId) {
    let [kind, size] = std::array::from_fn(|_| gid::new_cell_id());
    let mut cells = Cells::new();
    cells.set_value(size, name::record("size", [(OFFERS, offers)]));
    cells.set_value(
        kind,
        ::grap::isa::record(
            DEF,
            [
                (name::vocabulary::NAME, crate::libraries::text::value("box")),
                (FIELDS, Value::list([size.into()])),
            ],
        ),
    );
    let driver = Driver::new(
        crate::test_editor(Document {
            root: Some(::grap::isa::record(kind, [])),
            cells,
        }),
        Driver::VIEWPORT,
    );
    (driver, kind, size)
}

#[test]
fn a_fields_place_offers_the_values_its_definition_lists() {
    let (mut driver, _, size) = sized(Value::list([
        crate::libraries::f64::value(1.0),
        crate::libraries::f64::value(2.0),
    ]));
    let place = [Step::Key(size)];
    // Selected, the place opens its picker.
    driver.select(&place);
    let entries = driver.picker_entries().expect("the place's picker");
    assert!(entries.iter().any(|entry| entry == "1"), "{entries:?}");
    assert_eq!(driver.picker_detail("2").as_deref(), Some("size"));
    // What's typed narrows them.
    driver.type_text("2");
    assert!(
        !driver
            .picker_entries()
            .unwrap()
            .iter()
            .any(|entry| entry == "1")
    );
    driver.choose("2");
    assert_eq!(driver.value(&place), crate::libraries::f64::value(2.0));
}

#[test]
fn a_fields_offers_can_be_a_function_of_the_record_being_filled() {
    use crate::libraries::grap::control::vocabulary as control;
    // Offers the record itself, read as its kind.
    let value = crate::libraries::presentation::vocabulary::VALUE;
    let (mut driver, _, size) = sized(::grap::isa::record(
        ::grap::vocabulary::kind::LAMBDA,
        [
            (::grap::vocabulary::PARAMS, Value::list([value.into()])),
            (
                ::grap::vocabulary::BODY,
                // `[value]`, quoted, its one element computed.
                ::grap::call(
                    control::QUOTE.into(),
                    [(
                        ::grap::vocabulary::EXPRESSION,
                        Value::list([::grap::isa::record(
                            control::kind::UNQUOTE,
                            [(control::UNQUOTE, value.into())],
                        )]),
                    )],
                ),
            ),
        ],
    ));
    driver.select(&[Step::Key(size)]);
    let entries = driver.picker_entries().expect("the place's picker");
    assert!(entries.iter().any(|entry| entry == "box"), "{entries:?}");
}

#[test]
fn a_librarys_foreign_functions_are_the_loaded_stubs_it_lists() {
    use ui_events::keyboard::{Key, NamedKey};
    let mut driver = Driver::new(
        crate::test_editor(Document {
            root: Some(::grap::isa::record(LIBRARY, [(FOREIGN, Value::list([]))])),
            cells: Cells::new(),
        }),
        Driver::VIEWPORT,
    );
    let foreign = [Step::Key(FOREIGN)];
    driver.select(&foreign);
    assert!(driver.keystroke(Key::Named(NamedKey::Enter), crate::test_support::command()));
    driver.type_text("list len");
    assert_eq!(
        driver.picker_detail("list length").as_deref(),
        Some("foreign function")
    );
    driver.choose("list length");
    let (stub, definition) = first_cell(&driver, &foreign);
    assert_eq!(
        definition.as_record().and_then(::grap::isa::of),
        Some(::grap::vocabulary::kind::FOREIGN_FUNCTION)
    );
    let doc = &driver.editor().model.doc;
    let sources = crate::sources::Sources {
        doc,
        libraries: &driver.editor().stack.libraries,
    };
    assert_eq!(
        document_functions(sources.root(), &|cell| Some(sources.resolve(cell)?.value)),
        [stub]
    );
}

#[test]
fn space_folds_a_defs_and_a_librarys_lists() {
    let [kind, over] = std::array::from_fn(|_| gid::new_cell_id());
    let mut cells = Cells::new();
    cells.set_value(over, name::record("over", []));
    cells.set_value(
        kind,
        name::record(
            "fraction",
            [
                (::grap::isa::ISA, Value::from(DEF)),
                (FIELDS, Value::list([over.into()])),
            ],
        ),
    );
    let mut driver = Driver::new(
        crate::test_editor(Document {
            root: Some(::grap::isa::record(
                LIBRARY,
                [(DEFS, Value::list([kind.into()]))],
            )),
            cells,
        }),
        Driver::VIEWPORT,
    );
    let space = |driver: &mut Driver| {
        assert!(driver.keystroke(
            ui_events::keyboard::Key::Character(" ".into()),
            ui_events::keyboard::Modifiers::empty()
        ));
    };
    let defs = [Step::Key(DEFS)];
    let fields = [
        Step::Key(DEFS),
        Step::Element(first_position(&driver, &defs)),
        Step::Follow,
        Step::Key(FIELDS),
    ];
    let field = [
        &fields[..],
        &[Step::Element(first_position(&driver, &fields))],
    ]
    .concat();
    // The def's fields, drawn flowing.
    driver.select(&fields);
    space(&mut driver);
    assert!(!driver.drawn(&field));
    space(&mut driver);
    assert!(driver.drawn(&field));
    // The library's defs, drawn one to a line.
    driver.select(&defs);
    space(&mut driver);
    assert!(!driver.drawn(&fields));
}
