//! Typing and Enter at a number, as a person would.

use crate::driver::Driver;
use crate::libraries::number::vocabulary::{LEFT, RIGHT};
use crate::libraries::{f64, text, u64};
use crate::selection::Stage;
use gid::{Cells, Document, Step, Value, new_cell_id};
use kurbo::Size;
use ui_events::keyboard::{Key, Modifiers, NamedKey};

const VIEWPORT: Size = Size::new(600.0, 400.0);

fn driver(editor: crate::Editor) -> Driver {
    Driver::new(editor, VIEWPORT)
}

fn editor(root: Value) -> crate::Editor {
    crate::test_editor(Document {
        root: Some(root),
        cells: Cells::new(),
    })
}

#[test]
fn typing_an_operator_after_a_number_makes_it_the_left_operand() {
    let typed = |root: Value, typed: &str| {
        let mut driver = driver(editor(root));
        driver.select(&[]);
        driver.type_text(typed);
        driver
    };
    for (number, operator, symbol) in [
        (f64::value(2.0), f64::vocabulary::MULTIPLY, "*"),
        (f64::value(2.0), f64::vocabulary::SUBTRACT, "-"),
        (u64::value(6), u64::vocabulary::DIVIDE, "/"),
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
fn a_numbers_other_fields_follow_it_but_no_key_adds_one() {
    let (x, note) = (new_cell_id(), new_cell_id());
    let number = f64::value(1.0)
        .as_record()
        .unwrap()
        .clone()
        .update(note, text::value("hi"));
    let mut driver = driver(editor(Value::record([(x, Value::record(number))])));
    // The number view leaves its own field undrawn, unlike the record view.
    assert!(driver.drawn(&[Step::Key(x), Step::Key(note)]));
    assert!(!driver.drawn(&[Step::Key(x), Step::Key(f64::vocabulary::F64)]));
    // Drawn as one piece, the number takes no field from Enter, within it or
    // beside one of the others it has; a rawer view of it can add one.
    let before = driver.value(&[]);
    for (path, modifiers) in [
        (vec![Step::Key(x)], crate::modifiers::command()),
        (vec![Step::Key(x), Step::Key(note)], Modifiers::empty()),
    ] {
        driver.select(&path);
        assert!(!driver.keystroke(Key::Named(NamedKey::Enter), modifiers));
        assert_eq!(
            (driver.selection(), driver.stage()),
            (path, Some(Stage::Edge))
        );
    }
    assert_eq!(driver.value(&[]), before);
    assert!(driver.drawn(&[Step::Key(x), Step::Key(note)]));
}

#[test]
fn enter_adds_no_field_within_a_number_unless_its_view_shows_a_record() {
    let enter = |editor: crate::Editor, command: bool| {
        let mut driver = driver(editor);
        driver.select(&[]);
        let modifiers = if command {
            crate::modifiers::command()
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
