//! Grap's absent values, `{isa: absent, reason, …details}`: the kind says
//! the value is absent, and the reason, a cell, is the stable,
//! language-independent identity of why.

use crate::display::host::{Host, Layout, ProjectionInput};
use crate::libraries::{Library, name};
use crate::location::Location;
use ::grap::{ForeignFunction, ForeignFunctions};
use gid::CellId;

pub const ID: CellId = CellId::from_u128(0x873c68ac371dbbb98a4f198546d60241);

pub mod vocabulary {
    use gid::CellId;

    pub const REASON: CellId = ::grap::absent::REASON;
    pub const CELL: CellId = ::grap::absent::CELL;
    pub const VALUE: CellId = ::grap::absent::VALUE;
    pub const CYCLE: CellId = ::grap::absent::CYCLE;
    pub const CAUSES: CellId = ::grap::absent::CAUSES;
    pub const NO_ALTERNATIVE: CellId = ::grap::absent::NO_ALTERNATIVE;
    pub const DECLINED: CellId = ::grap::absent::DECLINED;
    pub const INVALID_ARGUMENTS: CellId = ::grap::absent::INVALID_ARGUMENTS;
    pub const MISSING_ARGUMENT: CellId = ::grap::absent::MISSING_ARGUMENT;
    pub const CALL: CellId = ::grap::absent::CALL;
    /// The calls an absent's call was made under, drawn below it; in its
    /// annotation record, whether they're shown.
    pub const CALLERS: CellId = CellId::from_u128(0x8724595a170962636e05d4c6ea37d0f5);
    pub const OR_DEFAULT: CellId = CellId::from_u128(0x6a7a30847dd7d939448d0e7eaeb82893);
    pub const DEFAULT: CellId = CellId::from_u128(0xd8bdf7403b181a43fa73578bd976ba3b);

    pub mod kind {
        use gid::CellId;

        pub const ABSENT: CellId = ::grap::vocabulary::kind::ABSENT;
    }
}

/// An absent value reads as its reason, then any other fields, such as a
/// failed alternative's causes, all on red. The call it's about reads as
/// that call, where it is: where it is is part of the absent, the call a
/// stop of its own.
fn display<W: Host>(input: &ProjectionInput<'_, W, ::grap::RuntimeValue>) -> Option<Layout<W>> {
    let reason = input.value?.absent_reason()?;
    let absent = crate::display::structure::Fields::default()
        .parts([vocabulary::CALL])
        .child(call_field)
        .after(input, is_head, head(input, reason));
    Some(on_red(match callers(input) {
        Some(callers) => crate::display::col(0, 2.0, [absent, callers]),
        None => absent,
    }))
}

/// Below an absent about a call, in a result that can be run again, the calls
/// that call was made under: shut, a toggle that opens them; open, each caller
/// drawn where it is, found by running the evaluation again. Shut, nothing
/// runs.
fn callers<W: Host>(input: &ProjectionInput<'_, W, ::grap::RuntimeValue>) -> Option<Layout<W>> {
    input.env.evaluated()?;
    let value = input.value?;
    let call = ::grap::source::read(&value.field(vocabulary::CALL)?.into_value())?;
    let open = callers_open(input.state);
    let site = input.site();
    let hover = W::Hover::from(site.toggle_hover());
    let toggle = crate::display::on_hover(
        crate::display::on_activate(
            crate::display::hover_highlight(
                crate::display::dim(if open {
                    "called from ▾"
                } else {
                    "called from ▸"
                }),
                hover.clone(),
            ),
            hover.clone(),
            std::rc::Rc::new(move |session| {
                let mut edit = site.open(session);
                let state = with_callers_open(edit.state(), !open);
                edit.set_state(state);
                true
            }),
        ),
        hover,
    );
    Some(match open {
        false => toggle,
        true => crate::display::col(
            0,
            2.0,
            [
                toggle,
                match input.env.stack(&call, &value.to_value()) {
                    // The call itself is first; with nothing after it, it's
                    // the evaluation's own.
                    Some(stack) if stack.len() < 2 => crate::display::dim("nothing: it's the top"),
                    Some(stack) => crate::display::at(
                        [gid::Step::Key(vocabulary::CALLERS)],
                        gid::Value::list(stack.iter().skip(1).map(::grap::source::value)),
                        Some(crate::display::runtime_partial(callers_column)),
                        None,
                    ),
                    None => crate::display::dim("not found"),
                },
            ],
        ),
    })
}

/// Whether an absent's annotation `state` shows its callers; they start
/// shut.
fn callers_open(state: Option<&gid::Value>) -> bool {
    state
        .and_then(gid::Value::as_record)
        .and_then(|fields| fields.get(&vocabulary::CALLERS))
        .and_then(gid::Value::as_cell)
        == Some(crate::annotations::EXPANDED)
}

/// `state` with its callers shown or not, leaving its other fields.
fn with_callers_open(state: Option<&gid::Value>, open: bool) -> gid::Value {
    let fields = state
        .and_then(gid::Value::as_record)
        .cloned()
        .unwrap_or_default();
    gid::Value::Record(fields.with(
        vocabulary::CALLERS,
        open.then(|| gid::Value::from(crate::annotations::EXPANDED)),
    ))
}

/// How an absent draws its field `key`: the call as [`call`] does, the
/// others as usual.
fn call_field<W: Host>(key: CellId) -> Option<crate::display::host::Partial<W>> {
    (key == vocabulary::CALL).then(|| crate::display::runtime_partial(call))
}

/// Where a call is, drawn as the call there, so it's the same place as the
/// call drawn in its code. Drawn as a part of what holds it, it's the call,
/// one stop.
fn call<W: Host>(input: &ProjectionInput<'_, W, ::grap::RuntimeValue>) -> Option<Layout<W>> {
    let call = location(::grap::source::read(&input.value?.to_value())?)?;
    Some(crate::display::jump([], call, None, None))
}

/// The calls an absent's call was made under, one under another, each where
/// it is as a part of the list, so each is its call.
fn callers_column<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
) -> Option<Layout<W>> {
    let positions = input.value?.list_positions()?;
    Some(crate::display::col(
        0,
        2.0,
        positions.into_iter().map(|position| {
            crate::display::part(
                [gid::Step::Element(position)],
                Some(crate::display::first(
                    input,
                    crate::display::runtime_partial(call),
                )),
                None,
            )
        }),
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
        crate::display::jump(
            [gid::Step::Key(vocabulary::CALL)],
            argument,
            Some(crate::display::first(
                input,
                crate::display::runtime_partial(crate::libraries::grap::expression),
            )),
            None,
        )
    });
    let named = crate::display::descend(
        [gid::Step::Key(vocabulary::CELL)],
        Some(crate::display::first(
            input,
            crate::display::runtime_partial(crate::libraries::name::shallow_cell),
        )),
        None,
    );
    Some(on_red(
        crate::display::structure::Fields::default()
            .parts([vocabulary::CALL])
            .child(call_field)
            .after(
                input,
                |key| {
                    is_head(key)
                        || key == vocabulary::CELL
                        || key == vocabulary::CALL && call.is_some()
                },
                crate::display::row(
                    4.0,
                    [head(input, vocabulary::MISSING_ARGUMENT), named]
                        .into_iter()
                        .chain(hole),
                ),
            ),
    ))
}

/// Where Grap says a call is, when it's somewhere a document names.
fn location(origin: ::grap::SourceOrigin) -> Option<Location<'static>> {
    match origin {
        ::grap::SourceOrigin::Root(path) => Some(Location::root(path)),
        ::grap::SourceOrigin::Cell { cell, path } => Some(Location::in_cell(cell, path)),
        ::grap::SourceOrigin::Input(_) => None,
    }
}

/// Whether `key` is one the head reads: what the absent is, and why.
fn is_head(key: CellId) -> bool {
    key == ::grap::isa::ISA || key == vocabulary::REASON
}

/// `reason`'s name; a click on it selects the absent drawn here.
fn head<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
    reason: CellId,
) -> Layout<W> {
    let name = match input.env.name(reason) {
        Some(name) => crate::display::named(&name, crate::display::Face::Ink),
        None => crate::display::faced(crate::identity::short_id(reason), crate::display::Face::Ink),
    };
    crate::display::selectable(name, &input.site())
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
        .cacheable()
}

fn functions() -> ForeignFunctions {
    ForeignFunctions::default().register(
        vocabulary::OR_DEFAULT,
        ForeignFunction::new(|context, call, environment| {
            let Some(expression) = call.field(vocabulary::VALUE) else {
                return Ok(call.missing_runtime_argument(vocabulary::VALUE));
            };
            let value = context.eval(expression, environment)?;
            if value.absent_reason().is_some() {
                match call.field(vocabulary::DEFAULT) {
                    Some(default) => context.eval(default, environment),
                    None => Ok(call.missing_runtime_argument(vocabulary::DEFAULT)),
                }
            } else {
                Ok(value)
            }
        })
        .parameters([vocabulary::VALUE, vocabulary::DEFAULT])
        .cacheable(),
    )
}

pub fn library<W: Host>() -> Library<W> {
    name::named_library(
        ID,
        "absent",
        [
            (vocabulary::kind::ABSENT, "absent"),
            (vocabulary::REASON, "reason"),
            (vocabulary::CELL, "cell"),
            (vocabulary::VALUE, "value"),
            (vocabulary::CYCLE, "cycle"),
            (vocabulary::CAUSES, "causes"),
            (vocabulary::CALL, "call"),
            (vocabulary::CALLERS, "callers"),
            (vocabulary::NO_ALTERNATIVE, "no applicable alternative"),
            (vocabulary::DECLINED, "not applicable"),
            (vocabulary::INVALID_ARGUMENTS, "invalid arguments"),
            (vocabulary::OR_DEFAULT, "or default"),
            (vocabulary::DEFAULT, "default"),
        ],
        functions(),
    )
    .with_projections_for([(
        vocabulary::kind::ABSENT,
        crate::display::compose_partials([
            crate::display::runtime_partial(missing_argument),
            crate::display::runtime_partial(display),
        ]),
    )])
}

#[cfg(test)]
#[path = "tests/absent.rs"]
mod tests;
