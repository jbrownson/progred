use super::*;
use ::grap;
use gid::Value;

#[test]
fn without_an_overlay_they_are_not_callable() {
    let library = crate::libraries::editor::library::<crate::Editor>();
    assert_eq!(
        crate::test_support::test_evaluate(
            &grap::call(Value::from(vocabulary::GET), []),
            |cell| library.definitions.value(cell).cloned(),
            &crate::test_support::foreign_functions(&library),
            10,
        )
        .result
        .to_value(),
        grap::absent::with_detail(
            grap::absent::NOT_CALLABLE,
            grap::absent::VALUE,
            library.definitions.value(vocabulary::GET).unwrap().clone()
        )
    );
}
