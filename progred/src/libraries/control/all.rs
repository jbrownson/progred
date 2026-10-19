use crate::libraries::control::vocabulary as control;
use ::grap::{Context, Environment, Expression, Halt, RuntimeValue};

pub(super) fn evaluate(
    context: &mut Context,
    call: &Expression,
    environment: &Environment,
) -> Result<RuntimeValue, Halt> {
    let Some(expressions) = context.field(call, control::EXPRESSIONS) else {
        return Ok(context.missing_runtime_argument(control::EXPRESSIONS));
    };
    let Some(elements) = context.elements(&expressions) else {
        return Ok(::grap::absent::value(control::INVALID_EXPRESSIONS).into());
    };
    super::try_map_sized(elements, |expression| {
        context.eval(expression.clone(), environment)
    })
    .map(RuntimeValue::list)
}

#[cfg(test)]
mod tests;
