use super::*;
use crate::libraries::f64;
use gid::Value;

#[test]
fn f32_from_f64_rounds_at_the_explicit_numeric_boundary() {
    let evaluate = |expression: &Value| {
        crate::test_support::test_evaluate(expression, |_| None, &functions(), 20).result
    };
    let convert = |operand| {
        evaluate(&::grap::call(
            vocabulary::F32_FROM_F64.into(),
            [(
                crate::libraries::grap::numbers::vocabulary::OPERAND,
                operand,
            )],
        ))
    };
    for number in [
        0.0,
        -0.0,
        0.1,
        -3.25,
        f64::MAX,
        f64::MIN_POSITIVE,
        f64::INFINITY,
    ] {
        let result = convert(f64::value(number));
        assert_eq!(
            f32::read(&result).unwrap().to_bits(),
            (number as f32).to_bits()
        );
    }
    assert!(f32::read(&convert(f64::value(f64::NAN))).unwrap().is_nan());
    let invalid = f32::value(1.0);
    assert_eq!(
        convert(invalid.clone()),
        ::grap::absent::with_detail(
            vocabulary::INVALID_F32_INPUT,
            crate::libraries::grap::numbers::vocabulary::OPERAND,
            invalid,
        )
    );
    assert!(::grap::absent::is_absent(&evaluate(&::grap::call(
        vocabulary::F32_FROM_F64.into(),
        [],
    ))));
}
