use crate::libraries::{absent, control::vocabulary as control};
use ::grap::{Context, Environment, Expression, Halt, RuntimeValue};

pub(super) fn evaluate(
    context: &mut Context,
    call: Expression,
    environment: &Environment,
) -> Result<RuntimeValue, Halt> {
    let Some(expressions) = context.field(call, control::EXPRESSIONS) else {
        return Ok(context.missing_runtime_argument(control::EXPRESSIONS));
    };
    let Some(count) = context.elements(expressions).map(<[_]>::len) else {
        return Ok(absent::with_reason(control::INVALID_EXPRESSIONS).into());
    };
    (0..count)
        .map(|index| {
            let expression = context.elements(expressions).unwrap()[index];
            context.eval_runtime(expression, environment)
        })
        .collect::<Result<Vec<_>, _>>()
        .map(RuntimeValue::list)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::libraries::test_evaluate;
    use ::grap::{ForeignFunction, ForeignFunctions};
    use gid::Value;
    use std::{cell::RefCell, rc::Rc};

    fn all(expressions: impl IntoIterator<Item = Value>) -> Value {
        ::grap::call(
            control::ALL.into(),
            [(control::EXPRESSIONS, Value::list(expressions))],
        )
    }

    fn functions() -> ForeignFunctions {
        ForeignFunctions::default()
            .register(control::ALL, ForeignFunction::runtime(evaluate).tracked())
    }

    #[test]
    fn all_retains_absents_and_effects_and_continues_in_order() {
        let emit = gid::new_cell_id();
        let log = Rc::new(RefCell::new(Vec::new()));
        let output = log.clone();
        let functions = functions().register(
            emit,
            ForeignFunction::new(move |context, call, env| {
                let value = context.eval(context.field(call, control::VALUE).unwrap(), env)?;
                Ok(context.effect(|| {
                    output.borrow_mut().push(value.clone());
                    value
                }))
            }),
        );
        let before = Value::from(b"before".to_vec());
        let after = Value::from(b"after".to_vec());
        for reason in [gid::new_cell_id(), ::grap::absent::FUEL_EXHAUSTED] {
            let failure = absent::with_reason(reason);
            let values = [before.clone(), failure, after.clone()];
            let expression = all(values
                .clone()
                .map(|value| ::grap::call(emit.into(), [(control::VALUE, value)])));
            log.borrow_mut().clear();
            let result = test_evaluate(&expression, |_| None, &functions, 100);
            assert!(result.completed);
            assert_eq!(result.result, Value::list(values.clone()));
            assert_eq!(*log.borrow(), values);
        }
    }

    #[test]
    fn all_handles_empty_and_malformed_lists_but_does_not_swallow_halts() {
        let run =
            |expression: &Value, fuel| test_evaluate(expression, |_| None, &functions(), fuel);
        assert_eq!(run(&all([]), 100).result, Value::list([]));
        let missing = run(&::grap::call(control::ALL.into(), []), 100);
        assert!(missing.completed);
        assert!(absent::reason(&missing.result).is_some());
        let invalid = ::grap::call(
            control::ALL.into(),
            [(control::EXPRESSIONS, Value::record([]))],
        );
        assert_eq!(
            run(&invalid, 100).result,
            absent::with_reason(control::INVALID_EXPRESSIONS)
        );
        let exhausted = run(&all((0..100).map(|_| Value::record([]))), 5);
        assert!(!exhausted.completed);
        assert_eq!(
            absent::reason(&exhausted.result),
            Some(::grap::absent::FUEL_EXHAUSTED)
        );
    }
}
