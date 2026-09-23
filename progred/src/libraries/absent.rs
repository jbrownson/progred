//! Tagged Grap absence values. The tag identifies the result as absent;
//! its cell payload is the stable, language-independent reason identity.

use crate::libraries::{Library, name};
use ::grap::{ForeignFunction, ForeignFunctions};
use gid::{CellId, Cells, Value};

pub const ID: CellId = CellId::from_u128(0x873c68ac371dbbb98a4f198546d60241);

pub mod vocabulary {
    use gid::CellId;

    pub const ABSENT: CellId = ::grap::absent::ABSENT;
    pub const CELL: CellId = ::grap::absent::CELL;
    pub const VALUE: CellId = ::grap::absent::VALUE;
    pub const CYCLE: CellId = ::grap::absent::CYCLE;
    pub const CAUSES: CellId = ::grap::absent::CAUSES;
    pub const NO_ALTERNATIVE: CellId = ::grap::absent::NO_ALTERNATIVE;
    pub const DECLINED: CellId = ::grap::absent::DECLINED;
    pub const OR_DEFAULT: CellId = CellId::from_u128(0x6a7a30847dd7d939448d0e7eaeb82893);
    pub const DEFAULT: CellId = CellId::from_u128(0xd8bdf7403b181a43fa73578bd976ba3b);
}

pub fn with_reason(reason: CellId) -> Value {
    ::grap::absent::value(reason)
}

#[cfg(test)]
pub fn reason(value: &Value) -> Option<CellId> {
    ::grap::absent::reason(value)
}

pub fn is_absent(value: &Value) -> bool {
    ::grap::absent::is_absent(value)
}

#[cfg(test)]
pub use ::grap::absent::decline;
pub use ::grap::absent::declines;

pub fn from_causes(causes: impl IntoIterator<Item = Value>) -> Value {
    ::grap::absent::from_causes(causes)
}

pub fn named_reason(value: impl Into<String>) -> Value {
    name::record(value, [])
}

fn functions() -> ForeignFunctions {
    ForeignFunctions::default().register(
        vocabulary::OR_DEFAULT,
        ForeignFunction::new(|context, call, environment| {
            let Some(expression) = context.field(call, vocabulary::VALUE) else {
                return Ok(context.missing_runtime_argument(vocabulary::VALUE));
            };
            let value = context.eval(expression, environment)?;
            if value.is_absent() {
                match context.field(call, vocabulary::DEFAULT) {
                    Some(default) => context.eval(default, environment),
                    None => Ok(context.missing_runtime_argument(vocabulary::DEFAULT)),
                }
            } else {
                Ok(value)
            }
        })
        .tracked(),
    )
}

pub fn library() -> Library<crate::Editor, crate::frame::Hovered> {
    let mut cells = Cells::new();
    cells.set_value(vocabulary::ABSENT, name::record("absent", []));
    cells.set_value(vocabulary::CELL, name::record("cell", []));
    cells.set_value(vocabulary::VALUE, name::record("value", []));
    cells.set_value(vocabulary::CYCLE, name::record("cycle", []));
    cells.set_value(vocabulary::CAUSES, name::record("causes", []));
    cells.set_value(
        vocabulary::NO_ALTERNATIVE,
        named_reason("no applicable alternative"),
    );
    cells.set_value(vocabulary::DECLINED, named_reason("not applicable"));
    cells.set_value(vocabulary::OR_DEFAULT, name::record("or default", []));
    cells.set_value(vocabulary::DEFAULT, name::record("default", []));
    Library::named(
        ID,
        "absent",
        crate::libraries::Definitions::from_parts(cells, functions()),
        crate::display::partial(|_| None),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use gid::new_cell_id;

    fn or_default(value: Value, default: Value) -> Value {
        ::grap::call(
            vocabulary::OR_DEFAULT.into(),
            [(vocabulary::VALUE, value), (vocabulary::DEFAULT, default)],
        )
    }

    #[test]
    fn default_preserves_present_values_without_type_validation() {
        let metadata = new_cell_id();
        let enriched = Value::record(
            crate::libraries::f64::value(0.4)
                .as_record()
                .unwrap()
                .clone()
                .update(metadata, Value::from(vec![7])),
        );
        for value in [
            enriched,
            crate::libraries::text::value("not a number"),
            Value::record([]),
            Value::record([(vocabulary::ABSENT, Value::from(vec![1]))]),
        ] {
            let result = crate::libraries::test_evaluate(
                &or_default(value.clone(), new_cell_id().into()),
                |_| None,
                &functions(),
                100,
            );
            assert!(result.completed);
            assert_eq!(result.result, value);
        }
    }

    #[test]
    fn default_evaluates_each_needed_argument_once_in_the_calling_environment() {
        use std::{cell::Cell, rc::Rc};
        let first = new_cell_id();
        let fallback = new_cell_id();
        let parameter = new_cell_id();
        for missing in [false, true] {
            let reads = Rc::new(Cell::new(0));
            let defaults = Rc::new(Cell::new(0));
            let functions = functions()
                .register(
                    first,
                    ForeignFunction::from_value({
                        let reads = reads.clone();
                        move |context, _, _| {
                            Ok(context.effect(|| {
                                reads.set(reads.get() + 1);
                                if missing {
                                    with_reason(new_cell_id())
                                } else {
                                    Value::record([])
                                }
                            }))
                        }
                    }),
                )
                .register(
                    fallback,
                    ForeignFunction::new({
                        let defaults = defaults.clone();
                        move |context, call, environment| {
                            let value = context.field(call, vocabulary::VALUE).unwrap();
                            let result = context.eval(value, environment)?;
                            Ok(context.effect(|| {
                                defaults.set(defaults.get() + 1);
                                result
                            }))
                        }
                    }),
                );
            let result = crate::libraries::test_apply(
                &::grap::lambda(
                    [parameter],
                    or_default(
                        ::grap::call(first.into(), []),
                        ::grap::call(fallback.into(), [(vocabulary::VALUE, parameter.into())]),
                    ),
                ),
                [(parameter, crate::libraries::f64::value(0.65))],
                |_| None,
                &functions,
                100,
            );
            assert!(result.completed);
            assert_eq!(reads.get(), 1);
            assert_eq!(defaults.get(), usize::from(missing));
            assert_eq!(
                result.result,
                if missing {
                    crate::libraries::f64::value(0.65)
                } else {
                    Value::record([])
                }
            );
        }
    }

    #[test]
    fn default_recovers_returned_absents_but_does_not_intercept_evaluator_halts() {
        let replacement = with_reason(new_cell_id());
        let returned = crate::libraries::test_evaluate(
            &or_default(
                with_reason(::grap::absent::FUEL_EXHAUSTED),
                replacement.clone(),
            ),
            |_| None,
            &functions(),
            100,
        );
        assert!(returned.completed);
        assert_eq!(returned.result, replacement);

        let recurse = new_cell_id();
        let fallback = new_cell_id();
        let functions = functions().register(
            fallback,
            ForeignFunction::from_value(|_, _, _| {
                panic!("an evaluator halt must not evaluate the default")
            }),
        );
        let result = crate::libraries::test_evaluate(
            &or_default(
                ::grap::call(recurse.into(), []),
                ::grap::call(fallback.into(), []),
            ),
            |cell| (cell == recurse).then(|| ::grap::lambda([], ::grap::call(recurse.into(), []))),
            &functions,
            30,
        );
        assert!(!result.completed);
        assert_eq!(reason(&result.result), Some(::grap::absent::FUEL_EXHAUSTED));
    }

    #[test]
    fn absence_is_an_open_tag_with_a_stable_reason() {
        let reason = new_cell_id();
        let extra = new_cell_id();
        let absent = Value::record(
            with_reason(reason)
                .as_record()
                .unwrap()
                .clone()
                .update(extra, Value::from(vec![1])),
        );

        assert!(is_absent(&absent));
        assert_eq!(super::reason(&absent), Some(reason));
        assert!(!is_absent(&named_reason("specific absence")));
        assert!(!is_absent(&Value::record([(
            vocabulary::ABSENT,
            Value::from(vec![1]),
        )])));
    }

    #[test]
    fn multiple_causes_form_an_ordered_absence_while_one_remains_itself() {
        let first = with_reason(new_cell_id());
        let second = with_reason(new_cell_id());

        assert_eq!(from_causes([first.clone()]), first);
        let combined = from_causes([first.clone(), second.clone()]);
        assert_eq!(reason(&combined), Some(vocabulary::NO_ALTERNATIVE));
        assert_eq!(
            combined
                .as_record()
                .and_then(|fields| fields.get(&vocabulary::CAUSES)),
            Some(&Value::list([first, second])),
        );
    }
}
