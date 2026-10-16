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

pub fn from_causes(causes: impl IntoIterator<Item = Value>) -> Value {
    ::grap::absent::from_causes(causes)
}

pub fn named_reason(value: impl Into<String>) -> Value {
    name::record(value, [])
}

/// An absent value reads as its reason, then any other fields, such as a
/// failed alternative's causes, all on red.
fn display(
    input: &crate::display::ProjectionInput<
        '_,
        crate::Editor,
        crate::frame::Hovered,
        ::grap::RuntimeValue,
    >,
) -> Option<crate::display::Layout<crate::Editor, crate::frame::Hovered>> {
    let reason = input.value?.field(vocabulary::ABSENT)?.as_cell()?;
    let name = input
        .env
        .name(reason)
        .map(str::to_owned)
        .unwrap_or_else(|| crate::identity::short_id(reason));
    let target = input.targets.current();
    let head = crate::display::projection::target(crate::display::activatable(
        crate::display::faced(name, crate::display::Face::Ink),
        target.hover,
        target.select,
    ));
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
        .parameters([vocabulary::VALUE, vocabulary::DEFAULT])
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
    )
    .with_projection(crate::display::runtime_partial(display))
}

#[cfg(test)]
mod tests;
