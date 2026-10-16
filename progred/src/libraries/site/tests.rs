use super::*;
use ::grap;
use gid::Value;

#[test]
fn without_an_overlay_they_are_not_callable() {
    let library = library();
    assert_eq!(
        crate::libraries::test_evaluate(
            &grap::call(Value::from(vocabulary::GET), []),
            |cell| library.value(cell).cloned(),
            &library.functions(),
            10,
        )
        .result,
        grap::absent::with_detail(
            grap::absent::NOT_CALLABLE,
            grap::absent::VALUE,
            library.value(vocabulary::GET).unwrap().clone()
        )
    );
}
