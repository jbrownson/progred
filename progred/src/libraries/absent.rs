//! Tagged Grap absence values. The tag identifies the result as absent;
//! its cell payload is the stable, language-independent reason identity.

use crate::display::host::{Host, Layout, ProjectionInput};
use crate::libraries::{Library, name};
use ::grap::{ForeignFunction, ForeignFunctions};
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

fn functions() -> ForeignFunctions {
    ForeignFunctions::default().register(
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
        ],
        functions(),
    )
    .with_projection(crate::display::runtime_partial(display))
}

#[cfg(test)]
mod tests;
