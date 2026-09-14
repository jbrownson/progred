//! Open boolean records. The payload identifies true or false without
//! evaluating that cell's description.

use crate::libraries::{Library, absent, name};
use ::grap::{ForeignFunction, ForeignFunctions};
use gid::{Cells, Value};

pub const ID: gid::CellId = gid::CellId::from_u128(0xf76a2ef341541a5c955fc23094e2df52);

pub mod vocabulary {
    use gid::CellId;

    pub const TRUE: CellId = CellId::from_u128(0x04831f9b704935059231eb111770e62e);
    pub const FALSE: CellId = CellId::from_u128(0x8fcc0a2e5f26c72efd9e917a28919801);
    pub const BOOL: CellId = CellId::from_u128(0xb2d13b64336a9e808133d0c7d5e4981e);
    pub const REQUIRE: CellId = CellId::from_u128(0xcadca87ed76939f9c1ba3bb94495ad06);
    pub const CONDITION: CellId = CellId::from_u128(0x91c786935eb5794b005582b8e7941bc2);
    pub const NOT_BOOL: CellId = CellId::from_u128(0x0f168ae21a023bdd711339e3aa949f9a);
    pub const CONDITION_NOT_MET: CellId = CellId::from_u128(0x1d6c6b09f182a3a036e19eb06cdf1ffc);
}

pub fn value(value: bool) -> Value {
    Value::record([(
        vocabulary::BOOL,
        Value::from(if value {
            vocabulary::TRUE
        } else {
            vocabulary::FALSE
        }),
    )])
}

pub fn read(value: &Value) -> Option<bool> {
    match value.as_record()?.get(&vocabulary::BOOL)?.as_cell()? {
        vocabulary::TRUE => Some(true),
        vocabulary::FALSE => Some(false),
        _ => None,
    }
}

fn functions() -> ForeignFunctions {
    ForeignFunctions::default().register(
        vocabulary::REQUIRE,
        ForeignFunction::new(|context, call, environment| {
            let Some(condition) = context.field(call, vocabulary::CONDITION) else {
                return Ok(context.missing_argument(vocabulary::CONDITION));
            };
            let value = context.eval(condition, environment)?;
            Ok(if absent::is_absent(&value) {
                value
            } else {
                match read(&value) {
                    Some(true) => Value::record([]),
                    accepted => ::grap::absent::with_detail(
                        if accepted.is_some() {
                            vocabulary::CONDITION_NOT_MET
                        } else {
                            vocabulary::NOT_BOOL
                        },
                        vocabulary::CONDITION,
                        value,
                    ),
                }
            })
        })
        .tracked(),
    )
}

pub fn library() -> Library<crate::Editor, crate::frame::Hovered> {
    let mut cells = Cells::new();
    for (cell, spelling) in [
        (vocabulary::TRUE, "true"),
        (vocabulary::FALSE, "false"),
        (vocabulary::BOOL, "bool"),
        (vocabulary::REQUIRE, "require"),
        (vocabulary::CONDITION, "condition"),
        (vocabulary::NOT_BOOL, "not a boolean"),
        (vocabulary::CONDITION_NOT_MET, "condition not met"),
    ] {
        cells.set_value(cell, name::record(spelling, []));
    }
    Library::named(
        ID,
        "logic",
        crate::libraries::Definitions::from_parts(cells, functions()),
        crate::display::partial(|_| None),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn booleans_are_open_inert_values_not_evaluated_cell_descriptions() {
        let library = library();
        for expected in [true, false] {
            let extra = gid::new_cell_id();
            let boolean = Value::record(
                value(expected)
                    .as_record()
                    .unwrap()
                    .update(extra, Value::record([])),
            );
            assert_eq!(read(&boolean), Some(expected));
            let evaluated = crate::libraries::test_evaluate(
                &boolean,
                |id| library.value(id).cloned(),
                &functions(),
                10,
            );
            assert_eq!(evaluated.result, boolean);
            let cell = if expected {
                vocabulary::TRUE
            } else {
                vocabulary::FALSE
            };
            assert_eq!(read(&cell.into()), None);
            assert_eq!(read(library.value(cell).unwrap()), None);
        }
        assert_eq!(
            read(&Value::record([(
                vocabulary::BOOL,
                gid::new_cell_id().into()
            )])),
            None
        );
    }

    #[test]
    fn require_accepts_true_rejects_false_and_preserves_absents() {
        let failure = ::grap::absent::with_detail(
            gid::new_cell_id(),
            vocabulary::CONDITION,
            Value::record([]),
        );
        for (condition, expected) in [
            (value(true), Value::record([])),
            (
                value(false),
                ::grap::absent::with_detail(
                    vocabulary::CONDITION_NOT_MET,
                    vocabulary::CONDITION,
                    value(false),
                ),
            ),
            (
                Value::record([]),
                ::grap::absent::with_detail(
                    vocabulary::NOT_BOOL,
                    vocabulary::CONDITION,
                    Value::record([]),
                ),
            ),
            (failure.clone(), failure),
        ] {
            let expression = ::grap::call(
                vocabulary::REQUIRE.into(),
                [(vocabulary::CONDITION, condition)],
            );
            let result = crate::libraries::test_evaluate(&expression, |_| None, &functions(), 20);
            assert!(result.completed);
            assert_eq!(result.result, expected);
        }
        let missing = crate::libraries::test_evaluate(
            &::grap::call(vocabulary::REQUIRE.into(), []),
            |_| None,
            &functions(),
            20,
        );
        assert_eq!(
            absent::reason(&missing.result),
            Some(::grap::absent::MISSING_ARGUMENT)
        );
    }
}
