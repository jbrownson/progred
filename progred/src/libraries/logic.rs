//! Open boolean records. The payload identifies true or false without
//! evaluating that cell's description.

use crate::display::Host;
use crate::libraries::{Library, name};
use ::grap::{ForeignFunction, ForeignFunctions};
use gid::Value;

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
        ForeignFunction::from_value(|context, call, environment| {
            let Some(condition) = context.field(call, vocabulary::CONDITION) else {
                return Ok(context.missing_argument(vocabulary::CONDITION));
            };
            let value = context.eval_to_value(condition, environment)?;
            Ok(if ::grap::absent::is_absent(&value) {
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
        .parameters([vocabulary::CONDITION])
        .tracked(),
    )
}

pub fn library<W: Host>() -> Library<W> {
    name::named_library(
        ID,
        "logic",
        [
            (vocabulary::TRUE, "true"),
            (vocabulary::FALSE, "false"),
            (vocabulary::BOOL, "bool"),
            (vocabulary::REQUIRE, "require"),
            (vocabulary::CONDITION, "condition"),
            (vocabulary::NOT_BOOL, "not a boolean"),
            (vocabulary::CONDITION_NOT_MET, "condition not met"),
        ],
        functions(),
    )
}

#[cfg(test)]
mod tests;
