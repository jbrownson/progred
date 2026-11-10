//! Typing and Enter at a number, as a person would, and each number's
//! arithmetic.

mod f32_arithmetic;
mod f64_arithmetic;
mod u64_arithmetic;

use crate::driver::Driver;
use crate::libraries::grap::numbers::vocabulary::{LEFT, RIGHT};
use crate::libraries::{f64, text, u64};
use crate::selection::Stage;
use gid::{Cells, Document, Step, Value, new_cell_id};
use ui_events::keyboard::{Key, Modifiers, NamedKey};

fn editor(root: Value) -> crate::Editor {
    crate::editor_of(root)
}

#[test]
fn typing_an_operator_after_a_number_makes_it_the_left_operand() {
    let typed = |root: Value, typed: &str| {
        let mut driver = Driver::of(root);
        driver.select::<Step>(&[]);
        driver.type_text(typed);
        driver
    };
    for (number, operator, symbol) in [
        (
            f64::value(2.0),
            crate::libraries::grap::numbers::f64::MULTIPLY,
            "*",
        ),
        (
            f64::value(2.0),
            crate::libraries::grap::numbers::f64::SUBTRACT,
            "-",
        ),
        (
            u64::value(6),
            crate::libraries::grap::numbers::u64::DIVIDE,
            "/",
        ),
    ] {
        let driver = typed(number.clone(), symbol);
        assert_eq!(
            driver.value(&[]),
            ::grap::call(operator.into(), [(LEFT, number)])
        );
        assert_eq!(driver.selection(), [Step::Key(RIGHT)]);
        assert_eq!(driver.stage(), Some(Stage::Pending));
    }
    // An exponent's sign belongs to the number it is still becoming.
    let driver = typed(f64::value(2.0), "e-");
    assert_eq!(driver.value(&[]), f64::value(2.0));
    assert_eq!(driver.line(), Some("2e-"));
}

#[test]
fn operators_typed_in_a_line_group_as_the_line_reads() {
    use crate::libraries::f32;
    use crate::libraries::grap::numbers::f32::{MULTIPLY, SUM as ADD};
    let call = |operator: gid::CellId, left: Value, right: Value| {
        ::grap::call(operator.into(), [(LEFT, left), (RIGHT, right)])
    };
    let n = f32::value;
    let typed = |text: &str| {
        let mut driver = Driver::of(n(1.0));
        driver.select::<Step>(&[]);
        for part in text.split(' ') {
            driver.type_text(part);
            if part.parse::<f64>().is_ok() {
                driver.keystroke(Key::Named(NamedKey::Enter), Modifiers::empty());
            }
        }
        driver.value(&[])
    };
    // A product binds tighter than the sum it follows, and takes its operand.
    assert_eq!(
        typed("+ 2 * 3"),
        call(ADD, n(1.0), call(MULTIPLY, n(2.0), n(3.0)))
    );
    // A sum after a product, or after another sum, takes all that came before.
    assert_eq!(
        typed("* 2 + 3"),
        call(ADD, call(MULTIPLY, n(1.0), n(2.0)), n(3.0))
    );
    assert_eq!(
        typed("+ 2 * 3 + 4"),
        call(
            ADD,
            call(ADD, n(1.0), call(MULTIPLY, n(2.0), n(3.0))),
            n(4.0)
        )
    );
    // A left operand ends nothing: an operator after it takes it alone.
    let mut driver = Driver::of(call(ADD, n(1.0), n(2.0)));
    driver.select(&[Step::Key(LEFT)]);
    driver.type_text("*");
    assert_eq!(
        driver.value(&[Step::Key(LEFT)]),
        ::grap::call(MULTIPLY.into(), [(LEFT, n(1.0))])
    );
}

#[test]
fn an_operation_chosen_over_a_value_takes_it_as_its_left_operand() {
    use crate::libraries::f32;
    use crate::libraries::grap::numbers::f32::{MULTIPLY, SUM};
    let x = Value::from(new_cell_id());
    let mut driver = Driver::of(::grap::call(
        SUM.into(),
        [(LEFT, x.clone()), (RIGHT, f32::value(1.0))],
    ));
    driver.select(&[Step::Key(LEFT)]);
    driver.type_text("*");
    driver.keystroke(Key::Named(NamedKey::Enter), Modifiers::empty());
    assert_eq!(
        driver.value(&[Step::Key(LEFT)]),
        ::grap::call(MULTIPLY.into(), [(LEFT, x)])
    );
    assert_eq!(driver.selection(), [Step::Key(LEFT), Step::Key(RIGHT)]);
    assert_eq!(driver.stage(), Some(Stage::Pending));
}

#[test]
fn an_operand_typed_as_a_number_is_one_of_its_operations_kind() {
    use crate::libraries::grap::numbers::{f64::SUM, u64};
    use crate::libraries::{f32, u64 as unsigned};
    // Typed after each kind of number, an operator's right operand is the
    // first reading of what's typed, and of that kind.
    for (left, right, operator) in [
        (f64::value(1.0), f64::value(2.0), SUM),
        (
            f32::value(1.0),
            f32::value(2.0),
            crate::libraries::grap::numbers::f32::SUM,
        ),
        (unsigned::value(1), unsigned::value(2), u64::SUM),
    ] {
        let mut driver = Driver::of(left.clone());
        driver.select::<Step>(&[]);
        driver.type_text("+");
        driver.type_text("2");
        driver.keystroke(Key::Named(NamedKey::Enter), Modifiers::empty());
        assert_eq!(
            driver.value(&[]),
            ::grap::call(
                operator.into(),
                [(LEFT, left.clone()), (RIGHT, right.clone())]
            )
        );
        // A number typed over an operand that isn't one is of that kind too.
        let mut driver = Driver::of(::grap::call(
            operator.into(),
            [(LEFT, left), (RIGHT, Value::from(new_cell_id()))],
        ));
        driver.select(&[Step::Key(RIGHT)]);
        driver.type_text("2");
        driver.keystroke(Key::Named(NamedKey::Enter), Modifiers::empty());
        assert_eq!(driver.value(&[Step::Key(RIGHT)]), right);
    }
}

#[test]
fn a_parenthesis_selects_the_operand_it_groups() {
    use crate::libraries::f32;
    use crate::libraries::grap::numbers::f32::{MULTIPLY, SUM};
    let n = f32::value;
    let sum = ::grap::call(SUM.into(), [(LEFT, n(3.0)), (RIGHT, n(4.0))]);
    let mut driver = Driver::of(::grap::call(
        MULTIPLY.into(),
        [(LEFT, n(2.0)), (RIGHT, sum)],
    ));
    let grouped = driver.rect(&[Step::Key(RIGHT)]);
    let first = driver.rect(&[Step::Key(RIGHT), Step::Key(LEFT)]);
    // The operand reaches out to its parentheses, past its first number.
    assert!(grouped.x0 < first.x0);
    driver.clicks_at(
        kurbo::Point::new((grouped.x0 + first.x0) / 2.0, grouped.center().y),
        1,
    );
    assert_eq!(driver.selection(), [Step::Key(RIGHT)]);
}

#[test]
fn a_numbers_other_fields_follow_it_but_no_key_adds_one() {
    let (x, note) = (new_cell_id(), new_cell_id());
    let number = f64::value(1.0)
        .as_record()
        .unwrap()
        .clone()
        .with(note, text::value("hi"));
    let mut driver = Driver::of(Value::record([(x, Value::record(number))]));
    // The number view leaves its own field undrawn, unlike the record view.
    assert!(driver.drawn(&[Step::Key(x), Step::Key(note)]));
    assert!(!driver.drawn(&[Step::Key(x), Step::Key(f64::vocabulary::F64)]));
    // Drawn as one piece, the number takes no field from Enter, within it or
    // beside one of the others it has; a rawer view of it can add one.
    let before = driver.value(&[]);
    for (path, modifiers) in [
        (vec![Step::Key(x)], crate::test_support::command()),
        (vec![Step::Key(x), Step::Key(note)], Modifiers::empty()),
    ] {
        driver.select(&path);
        assert!(!driver.keystroke(Key::Named(NamedKey::Enter), modifiers));
        assert_eq!(
            (driver.selection(), driver.stage()),
            (crate::occurrence::of(&path), Some(Stage::Edge))
        );
    }
    assert_eq!(driver.value(&[]), before);
    assert!(driver.drawn(&[Step::Key(x), Step::Key(note)]));
}

#[test]
fn enter_adds_no_field_within_a_number_unless_its_view_shows_a_record() {
    let enter = |editor: crate::Editor, command: bool| {
        let mut driver = Driver::new(editor, Driver::VIEWPORT);
        driver.select::<Step>(&[]);
        let modifiers = if command {
            crate::test_support::command()
        } else {
            Modifiers::empty()
        };
        driver.keystroke(Key::Named(NamedKey::Enter), modifiers);
        driver.stage()
    };
    // A number at the root has nothing beside it, and nothing within.
    for command in [false, true] {
        assert_eq!(enter(editor(f64::value(5.0)), command), Some(Stage::Edge));
    }
    // Without the number library it reads as the record it is, which takes one.
    let raw = || {
        crate::test_editor_with_stack(
            Document {
                root: Some(f64::value(5.0)),
                cells: Cells::new(),
            },
            crate::stack::load_selected(&[]).unwrap(),
        )
    };
    for command in [false, true] {
        assert_eq!(enter(raw(), command), Some(Stage::Label));
    }
}
