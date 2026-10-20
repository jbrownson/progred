//! Tab and Shift+Tab between empty slots, as a person presses them.

use crate::driver::Driver;
use crate::libraries::{name, text};
use crate::occurrence::Step;
use crate::selection::Stage;
use gid::{Cells, Document, Value, new_cell_id};
use ui_events::keyboard::{Key, Modifiers, NamedKey};

#[test]
fn tab_goes_from_empty_slot_to_empty_slot_keeping_what_was_typed() {
    let function = new_cell_id();
    let [x, height] = [new_cell_id(), new_cell_id()];
    let mut cells = Cells::new();
    let tree = ::grap::lambda([x, height], Value::record([]));
    let tree = tree
        .as_record()
        .unwrap()
        .clone()
        .update(name::vocabulary::NAME, text::value("tree"));
    cells.set_value(function, Value::record(tree));
    cells.set_value(x, name::record("x", []));
    cells.set_value(height, name::record("height", []));
    let mut driver = Driver::new(
        crate::test_editor(Document {
            root: Some(::grap::call(function.into(), [])),
            cells,
        }),
        kurbo::Size::new(600.0, 400.0),
    );
    let tab =
        |driver: &mut Driver, modifiers| driver.keystroke(Key::Named(NamedKey::Tab), modifiers);
    let selected = |driver: &Driver| (driver.selection(), driver.stage());
    let cleared = |driver: &Driver| driver.editor().model.selection.is_none();
    let slot = |key, stage| (vec![Step::Key(key)], Some(stage));
    // From nothing, Tab starts at the first empty slot. There's none before
    // it, so Shift+Tab clears the selection, and the next starts at the end.
    assert!(tab(&mut driver, Modifiers::empty()));
    assert_eq!(selected(&driver), slot(x, Stage::Pending));
    assert!(tab(&mut driver, Modifiers::SHIFT));
    assert!(cleared(&driver));
    assert!(tab(&mut driver, Modifiers::SHIFT));
    assert_eq!(selected(&driver), slot(height, Stage::Pending));
    assert!(tab(&mut driver, Modifiers::SHIFT));
    assert_eq!(selected(&driver), slot(x, Stage::Pending));
    // What was typed goes in, and Tab moves on to the next slot.
    driver.type_text("172");
    assert!(tab(&mut driver, Modifiers::empty()));
    assert!(
        driver.value(&[]).as_record().unwrap().get(&x).is_some(),
        "x keeps 172"
    );
    assert_eq!(selected(&driver), slot(height, Stage::Pending));
    // Past the last slot, a window's Tab clears the selection, and the next
    // starts over; a browser moves focus on to its page instead.
    assert!(tab(&mut driver, Modifiers::empty()));
    assert!(cleared(&driver));
    assert!(tab(&mut driver, Modifiers::empty()));
    assert_eq!(selected(&driver), slot(height, Stage::Pending));
    // What's typed in the last slot still goes in, and stays selected.
    driver.type_text("80");
    assert!(tab(&mut driver, Modifiers::empty()));
    assert!(
        driver
            .value(&[])
            .as_record()
            .unwrap()
            .get(&height)
            .is_some(),
        "height keeps 80"
    );
    assert_eq!(selected(&driver), slot(height, Stage::Edge));
    // With every slot filled, Tab clears the selection, then has nowhere to go.
    assert!(tab(&mut driver, Modifiers::empty()));
    assert!(cleared(&driver));
    assert!(!tab(&mut driver, Modifiers::empty()));
}

#[test]
fn tab_stops_at_empty_holes_optional_or_not_but_not_at_markers() {
    // An unnamed value draws its name as an empty hole, which Tab visits
    // though naming is optional. An unnamed lambda draws a λ to click
    // instead, which marks no tab stop.
    let parameter = new_cell_id();
    let mut cells = Cells::new();
    cells.set_value(parameter, name::record("y", []));
    let root = Value::record([(
        ::grap::vocabulary::VALUE,
        ::grap::lambda([parameter], parameter.into()),
    )]);
    let mut driver = Driver::new(
        crate::test_editor(Document {
            root: Some(root),
            cells,
        }),
        kurbo::Size::new(600.0, 400.0),
    );
    let tab = |driver: &mut Driver| driver.keystroke(Key::Named(NamedKey::Tab), Modifiers::empty());
    assert!(tab(&mut driver));
    assert_eq!(
        (driver.selection(), driver.stage()),
        (
            vec![Step::Key(name::vocabulary::NAME)],
            Some(Stage::Pending)
        )
    );
    assert!(tab(&mut driver));
    assert!(driver.editor().model.selection.is_none());
}
