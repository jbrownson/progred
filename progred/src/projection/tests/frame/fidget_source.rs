use super::*;
use progred_libraries::fidget::vocabulary::*;

fn sum(left: Value, right: Value) -> Value {
    Value::record([(SUM, Value::record([(LEFT, left), (RIGHT, right)]))])
}

#[test]
fn fidget_operands_keep_editable_paths_and_operator_hover_selects_the_expression() {
    let mut doc = Rc::new(Document {
        root: Some(sum(
            Value::record([(AXIS, X.into())]),
            progred_libraries::f32::value(25.0),
        )),
        cells: Cells::new(),
    });
    let coordinate = [Step::Key(SUM), Step::Key(LEFT), Step::Key(AXIS)];
    let number = vec![Step::Key(SUM), Step::Key(RIGHT)];
    let (bench, _) = place(&doc, None, 400.0);
    let rect = |path: &[Step]| {
        bench
            .descends
            .iter()
            .find(|d| d.path.as_ref() == path)
            .unwrap()
            .rect
    };
    let coordinate_rect = rect(&coordinate);
    let number_rect = rect(&number);
    assert!(
        !bench
            .descends
            .iter()
            .any(|d| d.path.iter().any(|s| matches!(s, Step::Follow(_))))
    );
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
        let (hovered, _) = place_with_pointer(&doc, None, 400.0, Some(point));
        assert!(matches!(hovered.hit,
            Some(Claim::Direct(Hovered::Tree(Hover::Value(found)))) if *found == path));
    }
    let libraries = core_libraries();
    let mut selected = make_projected_editing_selection(&doc, &libraries, number.clone());
    assert_eq!(selected.edit().unwrap().text(), "25");
    selected.edit_mut().unwrap().set_text("30");
    assert!(write_through(&mut doc, &libraries, &mut selected));
    assert_eq!(
        src(&doc, &libraries).resolve_path(&number),
        Some(&progred_libraries::f32::value(30.0))
    );
}

#[test]
fn fidget_alternatives_wrap_without_changing_operand_locations() {
    let doc = Document {
        root: Some(
            (1..8).fold(progred_libraries::f32::value(0.0), |left, right| {
                sum(left, progred_libraries::f32::value(right as f32))
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
            .descends
            .into_iter()
            .map(|d| d.path)
            .collect::<Vec<_>>()
    };
    assert_eq!(paths(wide), paths(narrow));
}
