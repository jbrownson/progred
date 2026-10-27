//! Open boolean records. The payload identifies true or false without
//! evaluating that cell's description. Grap holds the convention natively,
//! as it does numbers; this library names it and adds `require`.

use crate::display::Host;
use crate::libraries::{Library, name};
use ::grap::{ForeignFunction, ForeignFunctions};
use gid::Value;

pub const ID: gid::CellId = gid::CellId::from_u128(0xf76a2ef341541a5c955fc23094e2df52);

pub mod vocabulary {
    use gid::CellId;

    pub use ::grap::logic::{BOOL, FALSE, NOT_BOOL, TRUE};
    pub const REQUIRE: CellId = CellId::from_u128(0xcadca87ed76939f9c1ba3bb94495ad06);
    pub const CONDITION: CellId = CellId::from_u128(0x91c786935eb5794b005582b8e7941bc2);
    pub const CONDITION_NOT_MET: CellId = CellId::from_u128(0x1d6c6b09f182a3a036e19eb06cdf1ffc);
}

fn functions() -> ForeignFunctions {
    ForeignFunctions::default().register(
        vocabulary::REQUIRE,
        ForeignFunction::with_arguments([vocabulary::CONDITION], |_, (condition,): (bool,)| {
            if condition {
                Value::record([])
            } else {
                ::grap::absent::with_detail(
                    vocabulary::CONDITION_NOT_MET,
                    vocabulary::CONDITION,
                    ::grap::logic::value(false),
                )
            }
        })
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
#[path = "tests/logic.rs"]
mod tests;
