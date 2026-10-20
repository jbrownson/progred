//! Open boolean records. The payload identifies true or false without
//! evaluating that cell's description. Grap holds the convention natively,
//! as it does numbers; this library names it and adds `require` and
//! `require all`.

use crate::display::Host;
use crate::libraries::{Library, name};
use ::grap::{ForeignFunction, ForeignFunctions};
use gid::Value;

pub const ID: gid::CellId = gid::CellId::from_u128(0xf76a2ef341541a5c955fc23094e2df52);

pub mod vocabulary {
    use gid::CellId;

    pub use ::grap::logic::{BOOL, FALSE, TRUE};
    pub const REQUIRE: CellId = CellId::from_u128(0xcadca87ed76939f9c1ba3bb94495ad06);
    pub const REQUIRE_ALL: CellId = CellId::from_u128(0xa680412d0b7c403c8c9a81fb7437dbf8);
    pub const CONDITION: CellId = CellId::from_u128(0x91c786935eb5794b005582b8e7941bc2);
    pub const NOT_BOOL: CellId = CellId::from_u128(0x0f168ae21a023bdd711339e3aa949f9a);
    pub const CONDITION_NOT_MET: CellId = CellId::from_u128(0x1d6c6b09f182a3a036e19eb06cdf1ffc);
}

pub use ::grap::logic::read;

/// `require all {<label>: <condition>…}` holds when every condition is true;
/// it checks them all, and otherwise is "condition not met" carrying each
/// that isn't true under its label, whatever it was: false, another value,
/// or an absent such as a missing argument.
fn require_all(
    context: &mut ::grap::Context,
    call: &::grap::Expression,
    environment: &::grap::Environment,
) -> Result<Value, ::grap::Halt> {
    let conditions = context.fields(call).unwrap_or_default().to_vec();
    let mut unmet = Vec::new();
    for (label, condition) in conditions {
        if label != ::grap::vocabulary::FUNCTION {
            let value = context.eval_to_value(condition, environment)?;
            if read(&value) != Some(true) {
                unmet.push((label, value));
            }
        }
    }
    Ok(if unmet.is_empty() {
        Value::record([])
    } else {
        ::grap::absent::caused_by_all(vocabulary::CONDITION_NOT_MET, unmet)
    })
}

fn functions() -> ForeignFunctions {
    ForeignFunctions::default()
        .register(
            vocabulary::REQUIRE_ALL,
            // Its labels are whatever conditions it's given.
            ForeignFunction::from_value(require_all)
                .parameters([])
                .tracked(),
        )
        .register(
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
                        Some(false) => ::grap::absent::with_detail(
                            vocabulary::CONDITION_NOT_MET,
                            vocabulary::CONDITION,
                            value,
                        ),
                        None => ::grap::absent::with_detail(
                            vocabulary::NOT_BOOL,
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
            (vocabulary::REQUIRE_ALL, "require all"),
            (vocabulary::CONDITION, "condition"),
            (vocabulary::NOT_BOOL, "not a boolean"),
            (vocabulary::CONDITION_NOT_MET, "condition not met"),
        ],
        functions(),
    )
}

#[cfg(test)]
mod tests;
