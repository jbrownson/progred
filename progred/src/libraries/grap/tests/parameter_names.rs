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
        computed,
        [first, second, nameless]
            .into_iter()
            .zip(["b", "c", "d"].map(String::from))
            .collect()
    );
    assert_eq!(
        [0, 25, 26, 27, 701, 702].map(letters),
        ["a", "z", "aa", "ab", "zz", "aaa"].map(String::from)
    );
}

/// `{grap: λ [(width) () ()] → second * …}`, with the call's right empty.
fn lambda_with_unnamed_parameters() -> (Driver, [CellId; 3]) {
    let (function, width, first, second) =
        (new_cell_id(), new_cell_id(), new_cell_id(), new_cell_id());
    let mut cells = Cells::new();
    cells.set_value(width, name::record("width", []));
    cells.set_value(first, name::record("", []));
    cells.set_value(second, name::record("", []));
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
