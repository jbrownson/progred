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
        .with(name::vocabulary::NAME, text::value("tree"));
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
fn tab_stops_at_empty_holes_optional_or_not_and_where_things_are_added() {
    // An unnamed value draws its name as an empty hole, which Tab visits
    // though naming is optional; an unnamed lambda draws a λ for its name,
    // which Tab visits as well. A list stops at each place an element can go.
    let parameter = new_cell_id();
    let mut cells = Cells::new();
    cells.set_value(parameter, name::record("y", []));
    let root = Value::record([
        (
            ::grap::isa::ISA,
            gid::Value::from(::grap::vocabulary::kind::VALUE_WRAPPER).into(),
        ),
        (
            ::grap::vocabulary::VALUE,
            ::grap::lambda([parameter], parameter.into()),
        ),
    ]);
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
    // The lambda's missing name, its λ, is a place to fill in too.
    let lambda = gid::Step::Key(::grap::vocabulary::VALUE);
    assert!(tab(&mut driver));
    assert_eq!(
        (driver.selection(), driver.stage()),
        (
            crate::occurrence::of(&[lambda.clone(), gid::Step::Key(name::vocabulary::NAME)]),
            Some(Stage::Pending)
        )
    );
    // Then the places a parameter can be added: before y, and after it.
    let params = [lambda, gid::Step::Key(::grap::vocabulary::PARAMS)];
    let y = driver.positions(&params)[0].clone();
    for (low, high) in [(None, Some(&y)), (Some(&y), None)] {
        assert!(tab(&mut driver));
        let place = gid::Step::Element(gid::position::between(low, high).unwrap());
        assert_eq!(
            (driver.selection(), driver.stage()),
            (
                crate::occurrence::of(&[params.as_slice(), &[place]].concat()),
                Some(Stage::Pending)
            )
        );
    }
    assert!(tab(&mut driver));
    assert!(driver.editor().model.selection.is_none());
}

/// Where Tab goes from what's selected, and with what stage.
fn tabbed(driver: &mut Driver, back: bool) -> Option<(crate::occurrence::Path, Option<Stage>)> {
    let modifiers = if back {
        Modifiers::SHIFT
    } else {
        Modifiers::empty()
    };
    driver.keystroke(Key::Named(NamedKey::Tab), modifiers);
    driver
        .editor()
        .model
        .selection
        .is_some()
        .then(|| (driver.selection(), driver.stage()))
}

#[test]
fn tab_stops_at_each_place_an_element_can_go_and_shift_tab_goes_back() {
    let mut driver = Driver::of(Value::list([
        crate::libraries::f64::value(1.0),
        crate::libraries::f64::value(2.0),
    ]));
    let [one, two] = <[_; 2]>::try_from(driver.positions(&[])).unwrap();
    let place = |low: Option<&gid::Position>, high| {
        (
            crate::occurrence::of(&[gid::Step::Element(
                gid::position::between(low, high).unwrap(),
            )]),
            Some(Stage::Pending),
        )
    };
    let places = [
        place(None, Some(&one)),
        place(Some(&one), Some(&two)),
        place(Some(&two), None),
    ];
    for expected in &places {
        assert_eq!(tabbed(&mut driver, false).as_ref(), Some(expected));
    }
    // Shift+Tab goes back through them; the first has none before it.
    for expected in places[..2].iter().rev() {
        assert_eq!(tabbed(&mut driver, true).as_ref(), Some(expected));
    }
    // Nothing's added by passing through.
    assert_eq!(driver.positions(&[]), [one, two]);
}

#[test]
fn an_empty_list_and_a_list_one_to_a_line_stop_at_each_place_too() {
    let mut driver = Driver::of(Value::list([]));
    assert_eq!(
        tabbed(&mut driver, false),
        Some((
            crate::occurrence::of(&[gid::Step::Element(
                gid::position::between(None, None).unwrap()
            )]),
            Some(Stage::Pending)
        ))
    );
    // Too wide for a line, each element on its own, a place between each.
    let long = || text::value(&"x".repeat(60));
    let mut driver = Driver::new(
        crate::editor_of(Value::list([long(), long()])),
        kurbo::Size::new(400.0, 600.0),
    );
    let mut stops = 0;
    while tabbed(&mut driver, false).is_some() {
        stops += 1;
        assert!(stops < 10, "Tab keeps going");
    }
    assert_eq!(stops, 3);
}

#[test]
fn tab_stops_where_a_field_can_be_added_to_a_record() {
    let label = new_cell_id();
    let mut driver = Driver::of(Value::record([(label, crate::libraries::f64::value(1.0))]));
    assert_eq!(
        tabbed(&mut driver, false),
        Some((crate::occurrence::Path::default(), Some(Stage::Label)))
    );
    // Typed there, it's a new field's label, whose value comes next.
    driver.type_text("width");
    driver.press(NamedKey::Enter);
    let selected = driver.selection();
    let [Step::Key(width)] = selected.as_slice() else {
        panic!("a new field's value, not {selected:?}");
    };
    assert_ne!(*width, label);
    assert_eq!(driver.stage(), Some(Stage::Pending));
    assert_eq!(
        driver
            .editor()
            .sources()
            .resolve(*width)
            .map(|definition| definition.value.clone()),
        Some(name::record("width", []))
    );
    // An empty record has the place too.
    let mut driver = Driver::of(Value::record([]));
    assert_eq!(
        tabbed(&mut driver, false),
        Some((crate::occurrence::Path::default(), Some(Stage::Label)))
    );
    assert_eq!(tabbed(&mut driver, false), None);
}

#[test]
fn tab_reaches_an_empty_notebooks_first_entry() {
    use crate::libraries::notebook::vocabulary::{ENTRIES, kind::NOTEBOOK};
    let mut driver = Driver::of(::grap::isa::record(NOTEBOOK, [(ENTRIES, Value::list([]))]));
    assert_eq!(
        tabbed(&mut driver, false),
        Some((
            crate::occurrence::of(&[
                gid::Step::Key(ENTRIES),
                gid::Step::Element(gid::position::between(None, None).unwrap())
            ]),
            Some(Stage::Pending)
        ))
    );
}

#[test]
fn typing_and_tabbing_fills_a_list_one_element_after_another() {
    let mut driver = Driver::of(Value::list([]));
    for typed in ["1", "2", "3"] {
        tabbed(&mut driver, false);
        driver.type_text(typed);
    }
    driver.press(NamedKey::Enter);
    let numbers: Vec<_> = driver
        .value(&[])
        .as_list()
        .unwrap()
        .values()
        .map(crate::libraries::f32::read)
        .collect();
    assert_eq!(numbers, [Some(1.0), Some(2.0), Some(3.0)]);
    // Each Tab took up after what it committed, which wasn't drawn yet.
}
