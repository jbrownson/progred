pub(crate) fn grap_at(path: &[gid::Step], payload: gid::Value) -> gid::Value {
    use ::grap::{call, vocabulary as g};
    use gid::Value;
    Value::record([(
        g::CLOSURE,
        Value::record([
            (g::PARAMS, Value::list([])),
            (
                g::ENVIRONMENT,
                Value::record([(crate::libraries::site::vocabulary::VALUE, payload)]),
            ),
            (
                g::BODY,
                call(
                    vocabulary::SET.into(),
                    [
                        (
                            vocabulary::PATH,
                            call(
                                crate::libraries::list::vocabulary::CONCAT.into(),
                                [
                                    (
                                        crate::libraries::number::vocabulary::LEFT,
                                        call(crate::libraries::site::vocabulary::PATH.into(), []),
                                    ),
                                    (
                                        crate::libraries::number::vocabulary::RIGHT,
                                        crate::libraries::path::value(path),
                                    ),
                                ],
                            ),
                        ),
                        (
                            crate::libraries::site::vocabulary::VALUE,
                            crate::libraries::site::vocabulary::VALUE.into(),
                        ),
                    ],
                ),
            ),
        ]),
    )])
}

use super::*;
use gid::Value;

#[test]
fn the_vocabulary_alone_grants_no_selection_access() {
    let library = library::<crate::Editor>();
    assert_eq!(
        crate::libraries::test_evaluate(
            &::grap::call(Value::from(vocabulary::GET), []),
            |cell| library.value(cell).cloned(),
            &library.functions(),
            10,
        )
        .result,
        ::grap::absent::with_detail(
            ::grap::absent::NOT_CALLABLE,
            ::grap::absent::VALUE,
            library.value(vocabulary::GET).unwrap().clone()
        )
    );
}

#[test]
fn a_grap_selection_effect_can_run_with_a_different_interpreter() {
    use std::cell::RefCell;
    let site = vec![gid::Step::Element(
        gid::position::between(None, None).unwrap(),
    )];
    let child = gid::Step::Key(gid::new_cell_id());
    let writes = RefCell::new(Vec::new());
    let interpret = |function,
                     context: &mut ::grap::Context,
                     call: &::grap::Expression,
                     environment: &::grap::Environment| {
        if function == crate::libraries::site::vocabulary::PATH {
            Ok(crate::libraries::path::value(&site))
        } else {
            let path = context.field(call, vocabulary::PATH).unwrap();
            let path = context.eval_to_value(path, environment)?;
            let value = context
                .field(call, crate::libraries::site::vocabulary::VALUE)
                .unwrap();
            let value = context.eval_to_value(value, environment)?;
            Ok(context.effect(|| {
                writes
                    .borrow_mut()
                    .push((crate::libraries::path::read(&path).unwrap(), value));
                Value::record([])
            }))
        }
    };
    let functions = crate::libraries::list::library::<crate::Editor>().functions();
    let overlay = ::grap::ForeignOverlay::from_value(
        &[crate::libraries::site::vocabulary::PATH, vocabulary::SET],
        &interpret,
    );
    let result = ::grap::apply_value(
        &grap_at(
            std::slice::from_ref(&child),
            crate::selection::payload::empty_pending(),
        ),
        [],
        &crate::libraries::TestHost(|cell| {
            functions
                .get(cell)
                .cloned()
                .map(|function| {
                    (
                        gid::Resolution::Document,
                        ::grap::Definition::foreign(gid::Value::record([]), function),
                    )
                })
                .into_iter()
                .collect()
        }),
        Some(&overlay),
        100,
    );
    assert_eq!(result.result, Value::record([]));
    assert_eq!(
        writes.into_inner(),
        vec![(
            site.into_iter().chain([child]).collect(),
            Value::record([(vocabulary::STAGE, vocabulary::PENDING.into())])
        )],
    );
}
