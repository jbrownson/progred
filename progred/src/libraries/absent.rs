//! Tagged Grap absence values. The tag identifies the result as absent;
//! its cell payload is the stable, language-independent reason identity.

use crate::display::host::{Host, Layout, ProjectionInput};
use crate::libraries::{Library, name};
use crate::location::Location;
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
    pub const INVALID_ARGUMENTS: CellId = ::grap::absent::INVALID_ARGUMENTS;
    pub const MISSING_ARGUMENT: CellId = ::grap::absent::MISSING_ARGUMENT;
    pub const CALL: CellId = ::grap::absent::CALL;
    pub const OR_DEFAULT: CellId = CellId::from_u128(0x6a7a30847dd7d939448d0e7eaeb82893);
    pub const DEFAULT: CellId = CellId::from_u128(0xd8bdf7403b181a43fa73578bd976ba3b);
}

/// An absent value reads as its reason, then any other fields, such as a
/// failed alternative's causes, all on red. The call it's about reads as
/// that call, by its function, where it is.
fn display<W: Host>(input: &ProjectionInput<'_, W, ::grap::RuntimeValue>) -> Option<Layout<W>> {
    let reason = input.value?.absent_reason()?;
    Some(on_red(crate::display::structure::with_extra_fields_by(
        input,
        |key| key == vocabulary::ABSENT,
        call_field,
        head(input, reason),
    )))
}

/// How an absent draws its field `key`: the call as [`call`] does, the
/// others as usual.
fn call_field<W: Host>(key: CellId) -> Option<crate::display::host::Partial<W>> {
    (key == vocabulary::CALL).then(|| crate::display::runtime_partial(call))
}

/// Where a call is, drawn as the call there, by its function, so it's the
/// same place as the call drawn in its code.
fn call<W: Host>(input: &ProjectionInput<'_, W, ::grap::RuntimeValue>) -> Option<Layout<W>> {
    let call = location(::grap::source::read(&input.value?.to_value())?)?;
    Some(crate::display::jump_at(
        [],
        call,
        Some(crate::display::compose_partials([
            crate::display::runtime_partial(function),
            input.default_projection.clone(),
        ])),
    ))
}

/// A call by its function, drawn as a reference where it's a named cell.
fn function<W: Host>(input: &ProjectionInput<'_, W, ::grap::RuntimeValue>) -> Option<Layout<W>> {
    input.value?.field(::grap::vocabulary::FUNCTION)?;
    Some(crate::display::descend_local(
        [gid::Step::Key(::grap::vocabulary::FUNCTION)],
        crate::display::runtime_partial(crate::libraries::name::shallow_cell),
        &input.default_projection,
    ))
}

/// A missing argument reads as its reason and the parameter, then the hole
/// for the argument where the call is, so filling it there fills the call.
/// A call with no source has no hole to show.
fn missing_argument<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
) -> Option<Layout<W>> {
    let value = input.value?;
    (value.absent_reason()? == vocabulary::MISSING_ARGUMENT).then_some(())?;
    let parameter = value.field(vocabulary::CELL)?.as_cell()?;
    let call = value
        .field(vocabulary::CALL)
        .and_then(|call| ::grap::source::read(&call.into_value()))
        .and_then(location);
    let hole = call.as_ref().map(|call| {
        let mut argument = call.clone();
        argument.path.to_mut().push(gid::Step::Key(parameter));
        crate::display::jump_at(
            [gid::Step::Key(vocabulary::CALL)],
            argument,
            Some(crate::display::compose_partials([
                crate::display::runtime_partial(crate::libraries::grap::expression),
                input.default_projection.clone(),
            ])),
        )
    });
    let named = crate::display::descend_local(
        [gid::Step::Key(vocabulary::CELL)],
        crate::display::runtime_partial(crate::libraries::name::shallow_cell),
        &input.default_projection,
    );
    Some(on_red(crate::display::structure::with_extra_fields_by(
        input,
        |key| {
            key == vocabulary::ABSENT
                || key == vocabulary::CELL
                || key == vocabulary::CALL && call.is_some()
        },
        call_field,
        crate::display::row(
            4.0,
            [head(input, vocabulary::MISSING_ARGUMENT), named]
                .into_iter()
                .chain(hole),
        ),
    )))
}

/// Where Grap says a call is, when it's somewhere a document names.
fn location(origin: ::grap::SourceOrigin) -> Option<Location<'static>> {
    match origin {
        ::grap::SourceOrigin::Stored(path) => Some(Location::root(path)),
        ::grap::SourceOrigin::Cell {
            cell,
            resolution,
            path,
        } => Some(Location::in_cell(cell, resolution, path)),
        ::grap::SourceOrigin::Input(_) => None,
    }
}

/// `reason`'s name, a stop for the absent drawn here.
fn head<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    reason: CellId,
) -> Layout<W> {
    let name = match input.env.name(reason) {
        Some(name) => crate::display::named(&name, crate::display::Face::Ink),
        None => crate::display::faced(crate::identity::short_id(reason), crate::display::Face::Ink),
    };
    crate::display::stop(crate::display::activatable(name, &input.site()))
}

fn on_red<W: Host>(content: Layout<W>) -> Layout<W> {
    crate::display::ground(
        crate::display::padding((4.0, 1.0, 4.0, 1.0).into(), content),
        |palette| palette.error,
    )
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
                return Ok(context.missing_runtime_argument(call, vocabulary::VALUE));
            };
            let value = context.eval(expression, environment)?;
            if value.absent_reason().is_some() {
                match context.field(call, vocabulary::DEFAULT) {
                    Some(default) => context.eval(default, environment),
                    None => Ok(context.missing_runtime_argument(call, vocabulary::DEFAULT)),
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
            (vocabulary::CALL, "call"),
            (vocabulary::NO_ALTERNATIVE, "no applicable alternative"),
            (vocabulary::DECLINED, "not applicable"),
            (vocabulary::INVALID_ARGUMENTS, "invalid arguments"),
            (vocabulary::OR_DEFAULT, "or default"),
            (vocabulary::DEFAULT, "default"),
        ],
        functions(),
    )
    .with_projection(crate::display::compose_partials([
        crate::display::runtime_partial(missing_argument),
        crate::display::runtime_partial(display),
    ]))
}

#[cfg(test)]
mod tests;
