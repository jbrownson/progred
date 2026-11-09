use crate::*;
use gid::new_cell_id;
use std::cell::RefCell;

fn after(effect: Value, result: Value) -> Value {
    let ignored = new_cell_id();
    call(lambda([ignored], result), [(ignored, effect)])
}

struct Operations {
    read: CellId,
    write: CellId,
    value: CellId,
}

impl Operations {
    fn new() -> Self {
        Self {
            read: new_cell_id(),
            write: new_cell_id(),
            value: new_cell_id(),
        }
    }

    fn write(&self, value: Value) -> Value {
        call(self.write.into(), [(self.value, value)])
    }
}

#[derive(Clone, Copy)]
enum Invocation {
    Expression,
    Applied,
    Prepared,
}

fn run(
    invocation: Invocation,
    function: Value,
    operations: &Operations,
    definitions: impl Fn(CellId) -> Option<TestDefinition>,
    initial: Value,
    fuel: usize,
) -> (Evaluation, Value) {
    let definitions = TestHost(definitions);
    let effects = RefCell::new(initial);
    let invoke = new_cell_id();
    let functions = [operations.read, operations.write, invoke];
    let foreign =
        |cell, context: &mut Context<'_>, expression: &Expression, environment: &Environment| {
            if cell == operations.write {
                let argument = expression.field(operations.value).unwrap();
                let value = context.eval_to_value(argument, environment)?;
                Ok(context.effect(|| {
                    *effects.borrow_mut() = value.clone();
                    value
                }))
            } else if cell == operations.read {
                Ok(effects.borrow().clone())
            } else {
                let argument = expression.field(operations.value).unwrap();
                let callable = context.prepare_callable(argument, environment)?;
                context
                    .call_prepared(&callable, [])
                    .map(RuntimeValue::into_value)
            }
        };
    let overlay = ForeignFunctions::shared(&functions, &foreign);
    let result = match invocation {
        Invocation::Expression => Evaluator::new(&definitions, fuel)
            .functions(overlay)
            .evaluate(&call(function, [])),
        Invocation::Applied => Evaluator::new(&definitions, fuel)
            .functions(overlay)
            .apply_expression(&crate::RuntimeValue::from(&function), []),
        Invocation::Prepared => Evaluator::new(&definitions, fuel)
            .functions(overlay)
            .evaluate(&call(invoke.into(), [(operations.value, function)])),
    };
    (result, effects.into_inner())
}

const INVOCATIONS: [Invocation; 3] = [
    Invocation::Expression,
    Invocation::Applied,
    Invocation::Prepared,
];

#[test]
fn a_returned_halt_shaped_value_is_still_a_completed_result() {
    let absent = absent::value(absent::FUEL_EXHAUSTED);
    let evaluation = crate::Evaluator::new(&TestHost(|_| None), 10).evaluate(&absent);
    assert_eq!(evaluation.result.to_value(), absent);
    assert_eq!(evaluation.halt, None);
}

/// A decline after a write is an ordinary result: the host decides what a
/// declined evaluation's writes come to.
#[test]
fn absent_and_declined_returns_keep_their_effects() {
    let operations = Operations::new();
    let function = new_cell_id();
    let written = Value::from(b"written".to_vec());
    for (invocation, result) in INVOCATIONS.into_iter().flat_map(|invocation| {
        [
            (invocation, absent::value(new_cell_id())),
            (invocation, absent::decline()),
        ]
    }) {
        let (evaluation, state) = run(
            invocation,
            function.into(),
            &operations,
            |cell| {
                assert_eq!(cell, function);
                Some(TestDefinition::from(lambda(
                    [],
                    after(operations.write(written.clone()), result.clone()),
                )))
            },
            Value::record([]),
            100,
        );
        assert_eq!(evaluation.result.to_value(), result);
        assert_eq!(state, written);
        assert_eq!(evaluation.halt, None);
    }
}

#[test]
fn a_pure_decline_keeps_its_details() {
    let operations = Operations::new();
    let function = new_cell_id();
    let decline = absent::with_detail(
        absent::DECLINED,
        absent::VALUE,
        Value::from(b"details".to_vec()),
    );
    let (evaluation, state) = run(
        Invocation::Expression,
        function.into(),
        &operations,
        |_| Some(TestDefinition::from(lambda([], decline.clone()))),
        Value::record([]),
        100,
    );
    assert_eq!(evaluation.result.to_value(), decline);
    assert_eq!(state, Value::record([]));
}

#[test]
fn halts_stop_without_rolling_back_temporary_state() {
    let operations = Operations::new();
    let function = new_cell_id();
    let recurse = new_cell_id();
    for invocation in INVOCATIONS {
        let (evaluation, state) = run(
            invocation,
            function.into(),
            &operations,
            |cell| {
                if cell == recurse {
                    Some(TestDefinition::from(lambda([], call(recurse.into(), []))))
                } else {
                    assert_eq!(cell, function);
                    Some(TestDefinition::from(lambda(
                        [],
                        after(
                            operations.write(Value::from(vec![42])),
                            call(recurse.into(), []),
                        ),
                    )))
                }
            },
            Value::record([]),
            50,
        );
        assert_eq!(
            evaluation.result.to_value(),
            absent::value(absent::FUEL_EXHAUSTED)
        );
        assert_eq!(evaluation.remaining_fuel, 0);
        assert!(evaluation.halt.is_some());
        assert_eq!(state, Value::from(vec![42]));
    }
}

#[test]
fn a_non_callable_definition_returns_absent() {
    let operations = Operations::new();
    let function = new_cell_id();
    let (evaluation, state) = run(
        Invocation::Expression,
        function.into(),
        &operations,
        |_| {
            Some(TestDefinition::from(after(
                operations.write(Value::from(vec![42])),
                Value::record([]),
            )))
        },
        Value::record([]),
        100,
    );
    assert_eq!(
        evaluation.result.absent_reason(),
        Some(absent::NOT_CALLABLE)
    );
    assert_eq!(evaluation.halt, None);
    assert_eq!(state, Value::from(vec![42]));
}
