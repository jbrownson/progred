//! Parameters with no name of their own, called what their list calls them.

use super::*;
use crate::driver::Driver;
use crate::libraries::grap::numbers;
use ::grap::vocabulary::{BODY, PARAMS};

#[test]
fn a_list_calls_its_unnamed_parameters_by_letters_its_named_ones_leave() {
    let (named, first, second, nameless) =
        (new_cell_id(), new_cell_id(), new_cell_id(), new_cell_id());
    let mut cells = Cells::new();
    cells.set_value(named, name::record("a", []));
    cells.set_value(first, name::record("", []));
    cells.set_value(second, name::record("", []));
    cells.set_value(nameless, Value::record([]));
    let computed = unnamed_parameters(&[first, named, second, nameless], &|cell| cells.value(cell));
    assert_eq!(
        spelled(&computed),
        [first, second, nameless]
            .into_iter()
            .zip(["b", "c", "d"].map(|name| (name.to_owned(), Vec::new())))
            .collect()
    );
    assert_eq!(
        [0, 25, 26, 27, 701, 702].map(letters),
        ["a", "z", "aa", "ab", "zz", "aaa"].map(String::from)
    );
}

#[test]
fn an_environment_calls_its_unnamed_bindings_e_subscript_numbered_its_named_ones_leave() {
    let (named, first, second) = (new_cell_id(), new_cell_id(), new_cell_id());
    let mut cells = Cells::new();
    cells.set_value(named, name::record("e0", []));
    cells.set_value(first, Value::record([]));
    cells.set_value(second, name::record("", []));
    let computed = unnamed_bindings(&[first, named, second], &|cell| cells.value(cell));
    assert_eq!(
        spelled(&computed),
        [first, second]
            .into_iter()
            .zip(["e1", "e2"].map(|name| (name.to_owned(), vec![1..2])))
            .collect(),
        "their numerals subscripts"
    );
}

/// Computed names as their spellings and where their subscripts are.
fn spelled(
    computed: &std::collections::BTreeMap<CellId, crate::display::Name<'static>>,
) -> std::collections::BTreeMap<CellId, (String, Vec<std::ops::Range<usize>>)> {
    computed
        .iter()
        .map(|(cell, name)| (*cell, (name.spelling().into_owned(), name.subscripts())))
        .collect()
}

/// `{grap: λ [(width) () ()] → second * …}`, with the call's right empty.
fn lambda_with_unnamed_parameters() -> (Driver, [CellId; 3]) {
    let (function, width, first, second) =
        (new_cell_id(), new_cell_id(), new_cell_id(), new_cell_id());
    let mut cells = Cells::new();
    cells.set_value(width, name::record("width", []));
    cells.set_value(first, Value::record([]));
    cells.set_value(second, Value::record([]));
    cells.set_value(
        function,
        ::grap::lambda(
            [width, first, second],
            ::grap::call(
                numbers::f64::MULTIPLY.into(),
                [(numbers::vocabulary::LEFT, second.into())],
            ),
        ),
    );
    let driver = Driver::new(
        crate::test_editor(Document {
            root: Some(Value::record([(vocabulary::GRAP, function.into())])),
            cells,
        }),
        Driver::VIEWPORT,
    );
    (driver, [width, first, second])
}

fn in_lambda(rest: &[Step]) -> Vec<Step> {
    [
        &[
            Step::Key(vocabulary::GRAP),
            Step::Follow(gid::Resolution::Document),
        ],
        rest,
    ]
    .concat()
}

#[test]
fn an_unnamed_parameter_reads_as_its_letter_where_it_is_declared_and_used() {
    let (driver, _) = lambda_with_unnamed_parameters();
    let parameter = |index| driver.element(&in_lambda(&[Step::Key(PARAMS)]), index, &[]);
    let placeholder = |index| driver.glyphs_at(&parameter(index));
    let used = driver.glyphs_at(&in_lambda(&[
        Step::Key(BODY),
        Step::Key(numbers::vocabulary::LEFT),
    ]));
    assert_eq!(placeholder(2).len(), 1, "one letter, in one face");
    assert_eq!(used, placeholder(2), "the use reads as its declaration");
    assert_ne!(used, placeholder(1), "and not as its unnamed sibling");
    let (_, named) = &driver.glyphs_at(&parameter(0))[0];
    assert_ne!(&used[0].1, named, "a computed name is grey, a name isn't");
}

#[test]
fn a_slot_offers_unnamed_parameters_by_their_letters() {
    let (mut driver, [_, _, second]) = lambda_with_unnamed_parameters();
    driver.click(&in_lambda(&[
        Step::Key(BODY),
        Step::Key(numbers::vocabulary::RIGHT),
    ]));
    let entries = driver.picker_entries().expect("the empty right offers");
    let bindings: Vec<_> = entries
        .iter()
        .filter(|entry| driver.picker_detail(entry).as_deref() == Some("binding"))
        .cloned()
        .collect();
    assert_eq!(bindings, ["a", "b", "width"]);
    driver.type_text("b");
    driver.press(ui_events::keyboard::NamedKey::Enter);
    assert_eq!(
        driver.value(&in_lambda(&[
            Step::Key(BODY),
            Step::Key(numbers::vocabulary::RIGHT),
        ])),
        Value::from(second),
        "`b` is the second unnamed parameter"
    );
}

#[test]
fn clicking_a_missing_name_picks_one() {
    let (mut driver, [_, first, _]) = lambda_with_unnamed_parameters();
    let name = driver.element(
        &in_lambda(&[Step::Key(PARAMS)]),
        1,
        &[
            Step::Follow(gid::Resolution::Document),
            Step::Key(name::vocabulary::NAME),
        ],
    );
    driver.click(&name);
    assert_eq!(driver.stage(), Some(crate::selection::Stage::Pending));
    driver.type_text("height");
    driver.press(ui_events::keyboard::NamedKey::Enter);
    assert_eq!(
        driver
            .editor()
            .sources()
            .resolve(first)
            .map(|definition| definition.value.clone()),
        Some(name::record("height", []))
    );
}

#[test]
fn arrowing_left_out_of_a_name_stops_on_its_parameter() {
    let (mut driver, _) = lambda_with_unnamed_parameters();
    let cell = driver.element(&in_lambda(&[Step::Key(PARAMS)]), 1, &[]);
    let name = [
        &cell[..],
        &[
            Step::Follow(gid::Resolution::Document),
            Step::Key(name::vocabulary::NAME),
        ],
    ]
    .concat();
    driver.click(&name);
    driver.type_text("w");
    driver.press(ui_events::keyboard::NamedKey::ArrowLeft);
    assert_eq!(
        driver.selection(),
        crate::occurrence::of(&name),
        "the caret first"
    );
    // The cell and its definition are one thing.
    driver.press(ui_events::keyboard::NamedKey::ArrowLeft);
    assert_eq!(driver.selection(), crate::occurrence::of(&cell));
}

#[test]
fn backspacing_a_parameter_s_name_unnames_it_then_removes_it_never_its_definition_alone() {
    use ui_events::keyboard::{Key, Modifiers, NamedKey};
    let (mut driver, [width, _, _]) = lambda_with_unnamed_parameters();
    let params = in_lambda(&[Step::Key(PARAMS)]);
    let cell = driver.element(&params, 0, &[]);
    let name = [
        &cell[..],
        &[
            Step::Follow(gid::Resolution::Document),
            Step::Key(name::vocabulary::NAME),
        ],
    ]
    .concat();
    let defined = |driver: &Driver| {
        driver
            .editor()
            .sources()
            .resolve(width)
            .map(|definition| definition.value.clone())
    };
    let backspace = |driver: &mut Driver| {
        assert!(driver.keystroke(Key::Named(NamedKey::Backspace), Modifiers::empty()));
    };
    driver.select(&name);
    driver.press(NamedKey::End);
    for _ in "width".chars() {
        backspace(&mut driver);
    }
    backspace(&mut driver);
    assert_eq!(
        defined(&driver),
        Some(Value::record([])),
        "unnamed, still defined"
    );
    backspace(&mut driver);
    assert_eq!(
        driver.selection(),
        crate::occurrence::of(&cell),
        "on the parameter"
    );
    backspace(&mut driver);
    assert_eq!(driver.positions(&params).len(), 2, "gone");
    assert_eq!(
        defined(&driver),
        Some(Value::record([])),
        "its definition never alone"
    );
}

#[test]
fn a_parameter_with_no_definition_is_a_hole_that_typing_a_name_defines() {
    let (function, defined, undefined) = (new_cell_id(), new_cell_id(), new_cell_id());
    let mut cells = Cells::new();
    cells.set_value(defined, Value::record([]));
    cells.set_value(
        function,
        ::grap::lambda([defined, undefined], defined.into()),
    );
    let mut driver = Driver::new(
        crate::test_editor(Document {
            root: Some(Value::record([(vocabulary::GRAP, function.into())])),
            cells,
        }),
        Driver::VIEWPORT,
    );
    let params = in_lambda(&[Step::Key(PARAMS)]);
    let unnamed = driver.element(
        &params,
        0,
        &[
            Step::Follow(gid::Resolution::Document),
            Step::Key(name::vocabulary::NAME),
        ],
    );
    let missing = driver.element(&params, 1, &[Step::Follow(gid::Resolution::Document)]);
    assert_eq!(
        driver.glyphs_at(&unnamed).len(),
        1,
        "an unnamed one shows its letter"
    );
    assert!(
        driver.glyphs_at(&missing).is_empty(),
        "one with no definition is a hole"
    );
    driver.click(&missing);
    driver.type_text("depth");
    driver.press(ui_events::keyboard::NamedKey::Enter);
    assert_eq!(
        driver
            .editor()
            .sources()
            .resolve(undefined)
            .map(|definition| definition.value.clone()),
        Some(name::record("depth", []))
    );
}
