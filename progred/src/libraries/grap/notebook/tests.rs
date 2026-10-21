//! A notebook's entries, from the starting point on.

use super::vocabulary::NOTEBOOK;
use crate::driver::Driver;
use crate::libraries::grap::numbers;
use crate::libraries::{f32, f64, presentation};
use crate::occurrence;
use crate::selection::Stage;
use gid::{CellId, Cells, Document, Resolution, Step, Value, new_cell_id};
use ui_events::keyboard::{Key, Modifiers, NamedKey};

/// `rest` within entry `index`.
fn entry(driver: &Driver, index: usize, rest: &[Step]) -> Vec<Step> {
    driver.element(&[Step::Key(NOTEBOOK)], index, rest)
}

/// Where entry `index`'s expression is: its cell's definition.
fn expression(driver: &Driver, index: usize) -> Vec<Step> {
    entry(driver, index, &[Step::Follow(Resolution::Document)])
}

fn result_path(driver: &Driver, index: usize) -> occurrence::Path {
    occurrence::below(
        &occurrence::below(&[], &entry(driver, index, &[])),
        &[Step::Key(presentation::vocabulary::RESULT)],
    )
    .into_iter()
    .chain([occurrence::Step::Computed])
    .collect()
}

/// What entry `index` draws as its result, read by copying it; `None` when
/// it draws none.
fn result(driver: &mut Driver, index: usize) -> Option<Value> {
    let path = result_path(driver, index);
    driver.drawn(&path).then_some(())?;
    driver.select(&path);
    assert!(driver.keystroke(Key::Character("c".into()), crate::test_support::command()));
    driver.copied()
}

fn notebook(entries: &[CellId], cells: Cells) -> Driver {
    Driver::new(
        crate::test_editor(Document {
            root: Some(Value::record([(
                NOTEBOOK,
                Value::list(entries.iter().map(|&entry| entry.into())),
            )])),
            cells,
        }),
        Driver::VIEWPORT,
    )
}

#[test]
fn an_empty_document_starts_as_a_notebook_whose_entries_show_their_results() {
    let mut driver = Driver::new(crate::test_editor(Document::default()), Driver::VIEWPORT);
    driver.click::<Step>(&[]);
    driver.choose("notebook");
    assert_eq!(
        driver.selection(),
        occurrence::below(&[], &expression(&driver, 0))
    );
    assert_eq!(driver.stage(), Some(Stage::Pending));
    assert_eq!(result(&mut driver, 0), None, "nothing to show yet");

    driver.type_text("2");
    driver.press(NamedKey::Enter);
    let typed = driver.value(&expression(&driver, 0));
    assert_eq!(result(&mut driver, 0), Some(typed));

    // Enter on an entry starts the next, its expression to pick.
    driver.select(&expression(&driver, 0));
    driver.press(NamedKey::Enter);
    assert_eq!(driver.positions(&[Step::Key(NOTEBOOK)]).len(), 2);
    assert_eq!(
        driver.selection(),
        occurrence::below(&[], &expression(&driver, 1))
    );
    assert_eq!(driver.stage(), Some(Stage::Pending));
    assert!(
        driver.value(&entry(&driver, 1, &[])).as_cell().is_some(),
        "the entry is a cell"
    );
}

#[test]
fn shift_enter_starts_an_entry_before() {
    let first = new_cell_id();
    let mut cells = Cells::new();
    cells.set_value(first, f64::value(1.0));
    let mut driver = notebook(&[first], cells);
    driver.select(&expression(&driver, 0));
    assert!(driver.keystroke(Key::Named(NamedKey::Enter), Modifiers::SHIFT));
    assert_eq!(
        driver.value(&entry(&driver, 1, &[])),
        Value::from(first),
        "the first entry moved down one"
    );
    assert_eq!(
        driver.selection(),
        occurrence::below(&[], &expression(&driver, 0))
    );
}

#[test]
fn an_entry_that_reads_another_follows_it() {
    let (width, area) = (new_cell_id(), new_cell_id());
    let mut cells = Cells::new();
    cells.set_value(width, f64::value(3.0));
    cells.set_value(
        area,
        ::grap::call(
            numbers::f64::MULTIPLY.into(),
            [
                (numbers::vocabulary::LEFT, width.into()),
                (numbers::vocabulary::RIGHT, f64::value(2.0)),
            ],
        ),
    );
    let mut driver = notebook(&[width, area], cells);
    assert_eq!(result(&mut driver, 1), Some(f64::value(6.0)));

    driver.double_click(&expression(&driver, 0));
    driver.type_text("4");
    driver.press(NamedKey::Enter);
    assert_eq!(driver.value(&expression(&driver, 0)), f64::value(4.0));
    assert_eq!(result(&mut driver, 1), Some(f64::value(8.0)));
}

#[test]
fn an_empty_notebook_offers_its_first_entry() {
    let mut driver = notebook(&[], Cells::new());
    driver.click_at(
        driver.locate(crate::frame::Hovered::Tree(crate::hover::Hover::Value(
            occurrence::Trail::from([occurrence::Step::Key(NOTEBOOK)]),
        ))),
    );
    assert_eq!(driver.positions(&[Step::Key(NOTEBOOK)]).len(), 1);
    assert_eq!(
        driver.selection(),
        occurrence::below(&[], &expression(&driver, 0))
    );
    assert_eq!(driver.stage(), Some(Stage::Pending));
}

#[test]
fn the_arrow_selects_an_entry_to_delete() {
    let (first, second) = (new_cell_id(), new_cell_id());
    let mut cells = Cells::new();
    cells.set_value(first, f64::value(1.0));
    cells.set_value(second, f64::value(2.0));
    let mut driver = notebook(&[first, second], cells);
    let first_entry = occurrence::below(&[], &entry(&driver, 0, &[]));
    driver.click_at(
        driver.locate(crate::frame::Hovered::Tree(crate::hover::Hover::Value(
            occurrence::Trail::of(&first_entry),
        ))),
    );
    assert_eq!(driver.selection(), first_entry);
    driver.press(NamedKey::Backspace);
    assert_eq!(driver.positions(&[Step::Key(NOTEBOOK)]).len(), 1);
    assert_eq!(driver.value(&entry(&driver, 0, &[])), Value::from(second));
}

#[test]
fn a_closure_reads_as_its_lambda_then_what_it_closes_over_then_its_origin_shut() {
    use ::grap::source::vocabulary::BODY_ORIGIN;
    use ::grap::vocabulary::{ENVIRONMENT, LAMBDA};
    let (entry, x, y) = (new_cell_id(), new_cell_id(), new_cell_id());
    let mut cells = Cells::new();
    cells.set_value(x, crate::libraries::name::record("x", []));
    cells.set_value(y, crate::libraries::name::record("y", []));
    // `(λ [x] → λ [y] → x) {x: 1}`: a closure over `x`.
    cells.set_value(
        entry,
        ::grap::call(
            ::grap::lambda([x], ::grap::lambda([y], x.into())),
            [(x, f64::value(1.0))],
        ),
    );
    let mut driver = notebook(&[entry], cells);
    let closure = result_path(&driver, 0);
    let at = |steps: &[Step]| occurrence::below(&closure, steps);
    let before = |first: &[Step], second: &[Step]| {
        let (first, second) = (driver.rect(&at(first)), driver.rect(&at(second)));
        first.y1 <= second.y0 || first.x1 <= second.x0
    };
    assert!(before(&[Step::Key(LAMBDA)], &[Step::Key(ENVIRONMENT)]));
    assert!(before(&[Step::Key(ENVIRONMENT)], &[Step::Key(BODY_ORIGIN)]));
    assert!(
        driver.drawn(&at(&[
            Step::Key(LAMBDA),
            Step::Key(crate::libraries::name::vocabulary::NAME)
        ])),
        "the lambda reads as a lambda, its unnamed marker drawn"
    );
    assert!(driver.drawn(&at(&[Step::Key(ENVIRONMENT), Step::Key(x)])));
    let origin = at(&[Step::Key(BODY_ORIGIN)]);
    assert_eq!(driver.drawn_under(&origin).len(), 1, "shut, nothing within");
    driver.select(&origin);
    assert!(driver.keystroke(Key::Character(" ".into()), Modifiers::empty()));
    assert!(driver.drawn_under(&origin).len() > 1, "open, its fields");
}

#[test]
fn a_closure_in_a_result_reads_its_lambda_as_the_source_does() {
    use ::grap::vocabulary::{BODY, LAMBDA};
    let (entry, parameter) = (new_cell_id(), new_cell_id());
    let mut cells = Cells::new();
    cells.set_value(parameter, Value::record([]));
    cells.set_value(entry, ::grap::lambda([parameter], parameter.into()));
    let driver = notebook(&[entry], cells);
    let in_source = occurrence::below(
        &[],
        &[expression(&driver, 0), vec![Step::Key(BODY)]].concat(),
    );
    let in_result = occurrence::below(
        &result_path(&driver, 0),
        &[Step::Key(LAMBDA), Step::Key(BODY)],
    );
    assert_eq!(
        driver.glyphs_at(&in_source).len(),
        1,
        "the unnamed parameter reads as its letter"
    );
    assert_eq!(driver.glyphs_at(&in_result), driver.glyphs_at(&in_source));
}

#[test]
fn a_closure_calls_the_unnamed_bindings_it_holds_e_subscript_numbered_and_its_lambda_reads_them_so()
{
    use ::grap::vocabulary::{BODY, ENVIRONMENT, LAMBDA};
    let (entry, outer, inner) = (new_cell_id(), new_cell_id(), new_cell_id());
    let mut cells = Cells::new();
    cells.set_value(outer, Value::record([]));
    cells.set_value(inner, crate::libraries::name::record("y", []));
    // `(λ [a] → λ [y] → a) {a: 2}`: a closure holding the unnamed `a`, which
    // the list that named it doesn't come with.
    cells.set_value(
        entry,
        ::grap::call(
            ::grap::lambda([outer], ::grap::lambda([inner], outer.into())),
            [(outer, f64::value(2.0))],
        ),
    );
    let driver = notebook(&[entry], cells);
    let closure = result_path(&driver, 0);
    let used = driver.glyphs_at(&occurrence::below(
        &closure,
        &[Step::Key(LAMBDA), Step::Key(BODY)],
    ));
    let [(letter, _), (numeral, _)] = &used[..] else {
        panic!("the reference reads as e₀, its numeral a subscript: {used:?}")
    };
    assert_eq!((letter.len(), numeral.len()), (1, 1));
    let held = driver.glyphs_at(&occurrence::below(&closure, &[Step::Key(ENVIRONMENT)]));
    assert!(
        used.iter().all(|run| held.contains(run)),
        "the environment labels it as its lambda reads it"
    );
}

#[test]
fn arrows_go_into_a_notebook_from_its_whole_down_to_an_entry_s_expression() {
    let entry_cell = new_cell_id();
    let mut cells = Cells::new();
    cells.set_value(entry_cell, f64::value(42.0));
    let mut driver = notebook(&[entry_cell], cells);
    let arrow = |driver: &mut Driver, key| {
        assert!(driver.keystroke(Key::Named(key), Modifiers::empty()));
        driver.selection()
    };
    assert_eq!(arrow(&mut driver, NamedKey::ArrowDown), occurrence::of(&[]));
    for stop in [
        vec![Step::Key(NOTEBOOK)],
        entry(&driver, 0, &[]),
        expression(&driver, 0),
    ] {
        assert_eq!(
            arrow(&mut driver, NamedKey::ArrowRight),
            occurrence::of(&stop)
        );
    }
}

#[test]
fn a_deleted_entry_leaves_its_place_to_start_another() {
    let (first, second) = (new_cell_id(), new_cell_id());
    let mut cells = Cells::new();
    cells.set_value(first, f64::value(1.0));
    cells.set_value(second, f64::value(2.0));
    let mut driver = notebook(&[first, second], cells);
    driver.select(&entry(&driver, 0, &[]));
    driver.press(NamedKey::Backspace);
    assert_eq!(driver.positions(&[Step::Key(NOTEBOOK)]).len(), 1);
    assert_eq!(driver.stage(), Some(Stage::Pending));
    assert!(driver.drawn(&driver.selection()), "the place is drawn");
    assert_eq!(driver.picker_entries(), Some(vec!["new entry".into()]));

    driver.type_text("7");
    driver.press(NamedKey::Enter);
    driver.press(NamedKey::Enter);
    assert_eq!(driver.positions(&[Step::Key(NOTEBOOK)]).len(), 2);
    assert_eq!(driver.value(&expression(&driver, 0)), f32::value(7.0));
    assert_eq!(driver.value(&entry(&driver, 1, &[])), Value::from(second));
}

#[test]
fn a_click_between_entries_starts_one_there() {
    let (first, second) = (new_cell_id(), new_cell_id());
    let mut cells = Cells::new();
    cells.set_value(first, f64::value(1.0));
    cells.set_value(second, f64::value(2.0));
    let mut driver = notebook(&[first, second], cells);
    driver.click_between(&[Step::Key(NOTEBOOK)], 0);
    assert_eq!(driver.picker_entries(), Some(vec!["new entry".into()]));

    driver.type_text("7");
    driver.press(NamedKey::Enter);
    driver.press(NamedKey::Enter);
    assert_eq!(driver.positions(&[Step::Key(NOTEBOOK)]).len(), 3);
    assert_eq!(driver.value(&entry(&driver, 0, &[])), Value::from(first));
    assert_eq!(driver.value(&expression(&driver, 1)), f32::value(7.0));
    assert_eq!(result(&mut driver, 1), Some(f32::value(7.0)));
    assert_eq!(driver.value(&entry(&driver, 2, &[])), Value::from(second));
}
