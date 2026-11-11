//! Typing and Enter at a number, as a person would, and each number's
//! arithmetic.

mod f32_arithmetic;
mod f64_arithmetic;
mod u64_arithmetic;

use crate::driver::Driver;
use crate::libraries::grap::numbers::vocabulary::{LEFT, RIGHT};
use crate::libraries::{f64, text};
use crate::selection::Stage;
use gid::{Cells, Document, Step, Value, new_cell_id};
use ui_events::keyboard::{Key, Modifiers, NamedKey};

fn editor(root: Value) -> crate::Editor {
    crate::editor_of(root)
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
