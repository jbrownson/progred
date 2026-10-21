//! Projections for computed artifacts. A document may place one in a
//! pane, whose view opts into applying its declared projection. The
//! document view leaves that declaration editable. A projection function
//! receives one argument under [`vocabulary::VALUE`] and returns a value;
//! an absent result exposes its source. An explicit viewport function also
//! receives the pane's assigned logical width and height.

use crate::display::at;
use crate::display::host::{Host, Layout, ProjectionInput};
use crate::libraries::{Library, f64, layout, name};
use gid::Step;

mod outline;

pub const ID: gid::CellId = gid::CellId::from_u128(0xd22b834154d60b1df228f9bb4d3c13de);

pub mod vocabulary {
    use gid::CellId;

    pub const RENDER: CellId = CellId::from_u128(0x37cda4bdea0091349e305951564fbdf1);
    pub use crate::workspace::vocabulary::PROJECTION;
    pub const VIEWPORT: CellId = CellId::from_u128(0x709c987e4c2a110931d8597c4da68f69);
    /// Optional size-independent preparation of a viewport's data input.
    pub const PREPARE: CellId = CellId::from_u128(0x8b782978097343efd90f6fb61f649419);
    /// The single argument of a projection function.
    pub const VALUE: CellId = CellId::from_u128(0x84d3ba81fd2a52ea37478f4a868106f4);
    /// Ordered field references for an opt-in record outline.
    pub const OUTLINE: CellId = CellId::from_u128(0xe6c0b2058b8468428d64576d953e2d62);
    /// Occurrence step for a computed result, not a field in the source record.
    pub const RESULT: CellId = CellId::from_u128(0x11098129f74918af8a0924f94c37df4f);
}

pub fn viewport(value: &gid::Value) -> Option<(&gid::Value, &gid::Value)> {
    let fields = value.as_record()?;
    Some((
        fields.get(&vocabulary::VALUE)?,
        fields.get(&vocabulary::VIEWPORT)?,
    ))
}

/// A viewport function receives its assigned logical size, not a size
/// inferred from its output. Its result uses the ordinary layout and handlers.
pub fn viewport_display<W: Host>(
    input: &ProjectionInput<'_, W>,
    width: f64,
    height: f64,
) -> Option<Layout<W>> {
    let declaration = input.value?;
    if width <= 0.0 || height <= 0.0 {
        viewport(declaration).map(|_| crate::display::row(0.0, []))
    } else {
        viewport_runtime_output(declaration, input.env, width, height).map(result_or_value)
    }
}

/// `value` drawn where a computed result goes: at the
/// [`vocabulary::RESULT`] step below the occurrence that computed it.
pub fn result_layout<W: Host>(value: impl Clone + Into<::grap::RuntimeValue>) -> Layout<W> {
    at([Step::Key(vocabulary::RESULT)], value, None, None)
}

/// A projection's result in place, or where it's absent, the value it
/// projects, so the absence exposes its source.
fn result_or_value<W: Host>(result: ::grap::RuntimeValue) -> Layout<W> {
    if result.absent_reason().is_some() {
        crate::display::descend([Step::Key(vocabulary::VALUE)], None, None)
    } else {
        result_layout(&result)
    }
}

fn viewport_runtime_output(
    declaration: &gid::Value,
    env: &dyn crate::display::Env,
    width: f64,
    height: f64,
) -> Option<::grap::RuntimeValue> {
    let (value, function) = viewport(declaration)?;
    let fields = declaration.as_record()?;
    let view = |value| {
        env.apply_expression_runtime(
            &function.into(),
            &[
                (vocabulary::VALUE, value),
                (layout::vocabulary::WIDTH, ::grap::RuntimeValue::f64(width)),
                (
                    layout::vocabulary::HEIGHT,
                    ::grap::RuntimeValue::f64(height),
                ),
            ],
        )
    };
    Some(match fields.get(&vocabulary::PREPARE) {
        Some(prepare) => {
            let fuel = match fields.get(&layout::vocabulary::FUEL) {
                Some(value) => ::grap::f64::to_usize(f64::read(value)?)?,
                None => ::grap::DEFAULT_FUEL,
            };
            let prepared =
                env.apply_runtime_memo(prepare, &[(vocabulary::VALUE, value.clone())], fuel);
            // An absent preparation is the output; the viewport doesn't run.
            if prepared.absent_reason().is_some() {
                prepared
            } else {
                view(prepared)
            }
        }
        None => view(value.into()),
    })
}

/// A render's own fuel and the expression it runs, when it gives both.
fn fueled(render: &::grap::RuntimeValue) -> Option<(usize, ::grap::RuntimeValue)> {
    Some((
        ::grap::f64::to_usize(render.field(layout::vocabulary::FUEL)?.as_f64()?)?,
        render.field(::grap::vocabulary::EXPRESSION)?,
    ))
}

/// A render reads as its result, which what's drawn inside can run again to
/// ask how it got there.
pub fn display<W: Host>(input: &ProjectionInput<'_, W, ::grap::RuntimeValue>) -> Option<Layout<W>> {
    let render = input.value?.field(vocabulary::RENDER)?;
    let (expression, fuel, steps) = match fueled(&render) {
        Some((fuel, expression)) => (
            expression,
            fuel,
            vec![
                Step::Key(vocabulary::RENDER),
                Step::Key(::grap::vocabulary::EXPRESSION),
            ],
        ),
        None => (
            render,
            ::grap::DEFAULT_FUEL,
            vec![Step::Key(vocabulary::RENDER)],
        ),
    };
    let result = input.env.evaluate_runtime_memo(&expression, fuel, &steps);
    Some(crate::display::at_evaluated(
        [Step::Key(vocabulary::RESULT)],
        result,
        input.env.evaluation(&expression, fuel, &steps),
    ))
}

/// Opt-in presentation of a declaration; not part of the library's
/// ordinary authoring projection.
pub fn projected_display<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
) -> Option<Layout<W>> {
    let value = input.value?.field(vocabulary::VALUE)?;
    let function = input.value?.field(vocabulary::PROJECTION)?;
    Some(result_or_value(input.env.apply_expression_runtime(
        &function,
        &[(vocabulary::VALUE, value)],
    )))
}

pub fn library<W: Host>() -> Library<W> {
    name::named_library(
        ID,
        "presentation",
        [
            (vocabulary::RENDER, "render"),
            (vocabulary::PROJECTION, "projection"),
            (vocabulary::VIEWPORT, "viewport"),
            (vocabulary::PREPARE, "prepare"),
            (vocabulary::VALUE, "value"),
            (vocabulary::OUTLINE, "outline"),
            (vocabulary::RESULT, "result"),
        ],
        ::grap::ForeignFunctions::default(),
    )
    .with_projection(crate::display::compose_partials([
        crate::display::runtime_partial(display),
        crate::display::partial(outline::display),
    ]))
}

#[cfg(test)]
pub(crate) mod tests;
