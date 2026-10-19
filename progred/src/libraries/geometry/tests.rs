use super::*;
use gid::new_cell_id;

fn read(value: &Value) -> Option<f64> {
    let fields = value.as_record()?;
    let radius = fields
        .get(&vocabulary::CIRCLE)
        .and_then(Value::as_record)
        .and_then(|circle| circle.get(&vocabulary::RADIUS))
        .and_then(f64::read)?;
    (radius.is_finite() && radius >= 0.0).then_some(radius)
}

#[test]
fn circle_is_a_library_call_over_an_f64() {
    let foreign = functions();
    let expression = grap::call(
        Value::from(vocabulary::CIRCLE),
        [(vocabulary::RADIUS, f64::value(20.0))],
    );
    assert_eq!(
        crate::test_support::test_evaluate(&expression, |_| None, &foreign, 10).result,
        value(20.0)
    );
    assert_eq!(read(&value(20.0)), Some(20.0));

    let enriched = Value::record(
        value(20.0)
            .as_record()
            .unwrap()
            .clone()
            .update(new_cell_id(), Value::from(b"now".to_vec())),
    );
    assert_eq!(read(&enriched), Some(20.0));
    let with_extra = Value::record(enriched.as_record().unwrap().clone().update(
        vocabulary::CIRCLE,
        Value::record([
            (vocabulary::RADIUS, f64::value(20.0)),
            (new_cell_id(), Value::from(b"survey".to_vec())),
        ]),
    ));
    assert_eq!(read(&with_extra), Some(20.0));
}

#[test]
fn invalid_radius_is_a_library_sentinel() {
    let foreign = functions();
    let expression = grap::call(
        Value::from(vocabulary::CIRCLE),
        [(vocabulary::RADIUS, f64::value(-1.0))],
    );
    let evaluation = crate::test_support::test_evaluate(&expression, |_| None, &foreign, 10);
    assert_eq!(
        evaluation.result,
        ::grap::absent::value(vocabulary::INVALID_RADIUS)
    );
}
