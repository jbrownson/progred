use super::*;
use crate::driver::Driver;
use crate::libraries::{f64, layout, presentation, text};
use gid::{Cells, Document, Value};
use std::cell::RefCell;
use std::rc::Rc;

/// A document whose root records `rows` and `columns`, so neither splits
/// the window, and the parts of each.
fn inside() -> (Driver, [CellId; 2]) {
    let [rows, columns] = [gid::new_cell_id(), gid::new_cell_id()];
    let tall = crate::workspace::column([text::value("upper"), text::value("lower")]);
    let driver = Driver::of(Value::record([
        (rows, row([text::value("short"), tall])),
        (
            columns,
            crate::workspace::column([text::value("one"), text::value("two")]),
        ),
    ]));
    (driver, [rows, columns])
}

#[test]
fn a_row_inside_a_value_sets_its_parts_side_by_side_as_tall_as_the_tallest() {
    let (driver, [rows, _]) = inside();
    let list = [Step::Key(rows), Step::Key(vocabulary::PARTS)];
    let [short, tall] = [0, 1].map(|index| driver.rect(&driver.element(&list, index, &[])));
    assert!(tall.x0 > short.x1);
    assert_eq!(short.y0, tall.y0);
    assert!(tall.height() > short.height());
    // The row's own height is its tallest part's.
    let whole = driver.rect(&list);
    assert_eq!((whole.y0, whole.y1), (tall.y0, tall.y1));
}

#[test]
fn a_column_inside_a_value_stacks_its_parts() {
    let (driver, [_, columns]) = inside();
    let list = [Step::Key(columns), Step::Key(vocabulary::PARTS)];
    let [one, two] = [0, 1].map(|index| driver.rect(&driver.element(&list, index, &[])));
    assert!(two.y0 > one.y1);
    assert_eq!(one.x0, two.x0);
}

#[test]
fn raw_shows_a_row_inside_a_value_as_the_record_it_is() {
    let (mut driver, [rows, _]) = inside();
    let isa = [Step::Key(rows), Step::Key(::grap::isa::ISA)];
    assert!(!driver.drawn(&isa));
    driver
        .runner
        .editor
        .model
        .workspace
        .document_pane_mut()
        .shown = crate::workspace::Shown::RAW;
    driver.present();
    assert!(driver.drawn(&isa));
}

#[test]
fn a_sized_value_in_a_row_draws_at_the_default_size() {
    use crate::library::Definitions;
    let function = gid::new_cell_id();
    let arguments = [
        presentation::vocabulary::VALUE,
        layout::vocabulary::WIDTH,
        layout::vocabulary::HEIGHT,
    ];
    let sized = presentation::sized(
        text::value("source"),
        ::grap::lambda(
            arguments,
            ::grap::call(
                function.into(),
                arguments.map(|label| (label, label.into())),
            ),
        ),
    );
    let sizes = Rc::new(RefCell::new(Vec::new()));
    let mut stack = crate::stack::load();
    stack.libraries.insert(
        gid::new_cell_id(),
        Definitions::from_parts(
            Cells::new(),
            ::grap::ForeignFunctions::default().register(
                function,
                ::grap::ForeignFunction::from_value({
                    let sizes = sizes.clone();
                    move |context, call, environment| {
                        let mut read = |label| {
                            let value = call.field(label).unwrap();
                            context
                                .eval_to_value(value, environment)
                                .map(|value| f64::read(&value).unwrap())
                        };
                        sizes.borrow_mut().push((
                            read(layout::vocabulary::WIDTH)?,
                            read(layout::vocabulary::HEIGHT)?,
                        ));
                        Ok(layout::test_support::selectable_node(
                            layout::test_support::drawing(1.0, 0.0, 1.0, []),
                        ))
                    }
                }),
            ),
        ),
    );
    let key = gid::new_cell_id();
    let _driver = Driver::new(
        crate::test_editor_with_stack(
            Document {
                root: Some(Value::record([(key, row([sized, text::value("beside")]))])),
                cells: Cells::new(),
            },
            stack,
        ),
        Driver::VIEWPORT,
    );
    let default = presentation::DEFAULT_SIZE;
    assert!(sizes.borrow().contains(&(default, default)));
}

#[test]
fn enter_on_a_part_adds_one_beside_it_in_a_row_or_column() {
    use ui_events::keyboard::{Key, Modifiers, NamedKey};
    let (mut driver, keys) = inside();
    for key in keys {
        let list = [Step::Key(key), Step::Key(vocabulary::PARTS)];
        let [first, second] = [0, 1].map(|index| driver.element(&list, index, &[]));
        driver.select(&first);
        assert!(driver.keystroke(Key::Named(NamedKey::Enter), Modifiers::default()));
        // A part to write, between the two, drawn where it will go.
        assert_eq!(driver.stage(), Some(crate::selection::Stage::Pending));
        let pending = driver.selection();
        let [.., crate::occurrence::Step::Element(position)] = pending.as_slice() else {
            panic!("a new part: {pending:?}")
        };
        let [.., Step::Element(before)] = first.as_slice() else {
            unreachable!()
        };
        let [.., Step::Element(after)] = second.as_slice() else {
            unreachable!()
        };
        assert!(before < position && position < after);
        assert!(driver.drawn(&pending));
        driver.keystroke(Key::Named(NamedKey::Escape), Modifiers::default());
    }
}
