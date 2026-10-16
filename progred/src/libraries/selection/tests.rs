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
fn native_selection_needs_no_evaluator_and_agrees_with_grap() {
    let document = gid::Document {
        root: None,
        cells: gid::Cells::new(),
    };
    let empty = crate::libraries::Libraries::default();
    let stack = crate::stack::load();
    let site = vec![gid::Step::Key(gid::new_cell_id())];
    let child = vec![gid::Step::Element(
        gid::position::between(None, None).unwrap(),
    )];
    let destination: Vec<_> = site.iter().chain(&child).cloned().collect();
    for payload in [
        edge(),
        crate::libraries::absent::with_reason(vocabulary::NO_SELECTION),
    ] {
        for selected in [None, Some(destination.clone()), Some(site.clone())] {
            let initial = || crate::site::PendingChanges {
                annotation: Some(Value::from(vec![42])),
                annotation_changed: true,
                selection: selected.clone().map(|path| (path, pending())),
                selection_changed: true,
            };
            let mut native = initial();
            assert!(at(&child, payload.clone())(
                &crate::editing::Scope::default().view(crate::sources::Sources {
                    doc: &document,
                    libraries: &empty
                }),
                &site,
                &mut native,
            ));
            let mut interpreted = initial();
            assert!(crate::site::grap(grap_at(&child, payload.clone()), [])(
                &crate::editing::Scope::default().view(crate::sources::Sources {
                    doc: &document,
                    libraries: &stack.libraries
                }),
                &site,
                &mut interpreted,
            ));
            assert_eq!(native, interpreted);
        }
    }
}

#[test]
fn the_vocabulary_alone_grants_no_selection_access() {
    let library = library();
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
    let functions = crate::libraries::list::library().functions();
    let overlay = ::grap::ForeignOverlay::from_value(
        &[crate::libraries::site::vocabulary::PATH, vocabulary::SET],
        &interpret,
    );
    let result = ::grap::apply_value_scoped(
        &grap_at(std::slice::from_ref(&child), pending()),
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
        &overlay,
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
