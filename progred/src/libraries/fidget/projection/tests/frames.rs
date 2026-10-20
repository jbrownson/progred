use super::*;
use crate::display::test_support::Landmarks;
use crate::driver::Driver;
use crate::frame::Hovered;
use crate::hover::Hover;
use crate::projection::test_support::*;
use gid::{Cells, Document};
use kurbo::Point;
use puri::hover::Claim;
use ui_events::keyboard::{Key, Modifiers, NamedKey};

fn sum(left: Value, right: Value) -> Value {
    Value::record([(SUM, Value::record([(LEFT, left), (RIGHT, right)]))])
}

#[test]
fn fidget_operands_keep_editable_paths_and_operator_hover_selects_the_expression() {
    let doc = Document {
        root: Some(sum(
            Value::record([(AXIS, X.into())]),
            crate::libraries::f32::value(25.0),
        )),
        cells: Cells::new(),
    };
    let coordinate = [Step::Key(SUM), Step::Key(LEFT), Step::Key(AXIS)];
    let number = vec![Step::Key(SUM), Step::Key(RIGHT)];
    let (bench, _) = place(&doc, None, 400.0);
    let rect = |path: &[Step]| bench.landmarks.landmark(path).unwrap().rect;
    let coordinate_rect = rect(&coordinate);
    let number_rect = rect(&number);
    assert!(!bench.landmarks.iter().any(|d| {
        d.path
            .iter()
            .any(|s| matches!(s, crate::occurrence::Step::Follow(_)))
    }));
    for (point, path) in [
        (coordinate_rect.center(), coordinate.to_vec()),
        (number_rect.center(), number.clone()),
        (
            Point::new(
                (coordinate_rect.x1 + number_rect.x0) / 2.0,
                number_rect.center().y,
            ),
            vec![],
        ),
    ] {
        let (hovered, _) = place_with(BenchFrame {
            pointer: Some(point),
            ..BenchFrame::of(&doc, 400.0)
        });
        assert!(matches!(hovered.hit,
            Some(Claim::Direct(Hovered::Tree(Hover::Value(found)))) if *found == path));
    }
    let mut driver = Driver::new(crate::test_editor(doc), Driver::VIEWPORT);
    driver.select(&number);
    driver.press(NamedKey::End);
    assert_eq!(driver.line(), Some("25"));
    assert!(driver.keystroke(Key::Named(NamedKey::Home), Modifiers::SHIFT));
    driver.type_text("30");
    assert_eq!(driver.value(&number), crate::libraries::f32::value(30.0));
}

#[test]
fn fidget_alternatives_wrap_without_changing_operand_locations() {
    let doc = Document {
        root: Some(
            (1..8).fold(crate::libraries::f32::value(0.0), |left, right| {
                sum(left, crate::libraries::f32::value(right as f32))
            }),
        ),
        cells: Cells::new(),
    };
    let (wide, wide_size) = place(&doc, None, 900.0);
    let (narrow, narrow_size) = place(&doc, None, 200.0);
    assert!(narrow_size.height() > wide_size.height());
    assert!(narrow_size.width <= 200.0 - 48.0);
    let paths = |bench: Bench| {
        bench
            .landmarks
            .into_iter()
            .map(|d| d.path)
            .collect::<Vec<_>>()
    };
    assert_eq!(paths(wide), paths(narrow));
}
