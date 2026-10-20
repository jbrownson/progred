//! Tagged Grap absence values. The tag identifies the result as absent;
//! its cell payload is the stable, language-independent reason identity.

use crate::display::host::{Host, Layout, ProjectionInput};
use crate::libraries::{Library, name};
use ::grap::{
    Context, Environment, Expression, ForeignFunction, ForeignFunctions, Halt, RuntimeValue,
    declined,
};
use gid::CellId;

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
    pub const CHECKED: CellId = CellId::from_u128(0xabdf461dbaff4b0dbd317830ffe2cca7);
    pub const REASON: CellId = CellId::from_u128(0x626afdedb77044ed97e3bd813fbbefcb);
    pub const ARGUMENTS: CellId = CellId::from_u128(0xbc4c9ae57b4d4160a4993bf2ca879289);
    /// Why `checked` declines when its reason isn't a cell or its arguments
    /// aren't a record.
    pub const INVALID_CHECK: CellId = CellId::from_u128(0x7e46e7f695d147f99da777a9de957695);
}

/// An absent value reads as its reason, then any other fields, such as a
/// failed alternative's causes, all on red.
fn display<W: Host>(input: &ProjectionInput<'_, W, ::grap::RuntimeValue>) -> Option<Layout<W>> {
    let reason = input.value?.absent_reason()?;
    let name = match input.env.name(reason) {
        Some(name) => crate::display::named(&name, crate::display::Face::Ink),
        None => crate::display::faced(crate::identity::short_id(reason), crate::display::Face::Ink),
    };
    let site = input.site();
    let head = crate::display::stop(crate::display::activatable(name, &site));
    Some(crate::display::ground(
        crate::display::padding(
            (4.0, 1.0, 4.0, 1.0).into(),
            crate::display::structure::with_extra_fields(
                input,
                |key| key == vocabulary::ABSENT,
                head,
            ),
        ),
        |palette| palette.error,
    ))
}

/// A function whose implementation a scope supplies, as called outside that
/// scope: it answers only `reason`.
pub fn outside_scope(
    reason: CellId,
    parameters: impl IntoIterator<Item = CellId>,
) -> ForeignFunction {
    ForeignFunction::from_value(move |_, _, _| Ok(::grap::absent::value(reason)))
        .parameters(parameters)
        .tracked()
}

/// `checked {reason, arguments: {<label>: <expression>…}}` is Grap's
/// counterpart of [`declined::all`]: it evaluates each argument and returns
/// their values as a record when none is absent, and otherwise is `reason`,
/// carrying each absent under its label.
fn checked(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
) -> Result<RuntimeValue, Halt> {
    let reason = match context.field(call, vocabulary::REASON) {
        Some(reason) => {
            let reason = context.value(&reason);
            reason.as_cell().ok_or_else(|| reason.clone().into())
        }
        None => Err(context.missing_runtime_argument(vocabulary::REASON)),
    };
    let arguments = match context.field(call, vocabulary::ARGUMENTS) {
        Some(arguments) => context
            .fields(&arguments)
            .map(<[_]>::to_vec)
            .ok_or_else(|| context.value(&arguments).clone().into()),
        None => Err(context.missing_runtime_argument(vocabulary::ARGUMENTS)),
    };
    let (reason, arguments) = match declined::all(
        vocabulary::INVALID_CHECK,
        (
            (vocabulary::REASON, reason),
            (vocabulary::ARGUMENTS, arguments),
        ),
    ) {
        Ok(check) => check,
        Err(invalid) => return Ok(invalid),
    };
    let mut values = Vec::with_capacity(arguments.len());
    let mut absent = Vec::new();
    for (label, expression) in arguments {
        let value = context.eval(expression, environment)?;
        if value.absent_reason().is_some() {
            absent.push((label, value.into_value()));
        } else {
            values.push((label, value));
        }
    }
    Ok(if absent.is_empty() {
        RuntimeValue::record(values)
    } else {
        ::grap::absent::caused_by_all(reason, absent).into()
    })
}

fn functions() -> ForeignFunctions {
    ForeignFunctions::default()
        .register(
            vocabulary::CHECKED,
            ForeignFunction::new(checked)
                .parameters([vocabulary::REASON, vocabulary::ARGUMENTS])
                .tracked(),
        )
        .register(
            vocabulary::OR_DEFAULT,
            ForeignFunction::new(|context, call, environment| {
                let Some(expression) = context.field(call, vocabulary::VALUE) else {
                    return Ok(context.missing_runtime_argument(vocabulary::VALUE));
                };
                let value = context.eval(expression, environment)?;
                if value.absent_reason().is_some() {
                    match context.field(call, vocabulary::DEFAULT) {
                        Some(default) => context.eval(default, environment),
                        None => Ok(context.missing_runtime_argument(vocabulary::DEFAULT)),
                    }
                } else {
                    Ok(value)
                }
            })
            .parameters([vocabulary::VALUE, vocabulary::DEFAULT])
            .tracked(),
        )
}

pub fn library<W: Host>() -> Library<W> {
    name::named_library(
        ID,
        "absent",
        [
            (vocabulary::ABSENT, "absent"),
            (vocabulary::CELL, "cell"),
            (vocabulary::VALUE, "value"),
            (vocabulary::CYCLE, "cycle"),
            (vocabulary::CAUSES, "causes"),
            (vocabulary::NO_ALTERNATIVE, "no applicable alternative"),
            (vocabulary::DECLINED, "not applicable"),
            (vocabulary::OR_DEFAULT, "or default"),
            (vocabulary::DEFAULT, "default"),
            (vocabulary::CHECKED, "checked"),
            (vocabulary::REASON, "reason"),
            (vocabulary::ARGUMENTS, "arguments"),
            (vocabulary::INVALID_CHECK, "invalid check"),
        ],
        functions(),
    )
    .with_projection(crate::display::runtime_partial(display))
}

#[cfg(test)]
mod tests;
