//! Evaluation: fuel, effects, environments, and the values of expressions
//! and cells.

use crate::environment::{CellIndex, Environment};
use crate::lower::Form;
use crate::runtime::{RuntimeValue, RuntimeValueKind};
use crate::unboxed::{Native, Unboxed};
use crate::{Context, Evaluation, Expression, SourceOrigin, absent};
use gid::{CellId, Value};

/// Why an evaluation stopped before finishing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Halt {
    FuelExhausted,
    /// Evaluations nested deeper than [`MAX_DEPTH`].
    DepthExhausted,
}

impl Halt {
    /// The absent a halted evaluation answers.
    fn into_absent(self) -> RuntimeValue {
        RuntimeValue::from(match self {
            Self::FuelExhausted => absent::value(absent::FUEL_EXHAUSTED),
            Self::DepthExhausted => absent::value(absent::DEPTH_EXHAUSTED),
        })
    }
}

impl<'a> Context<'a> {
    pub(crate) fn conclude(
        mut self,
        run: impl FnOnce(&mut Self) -> Result<RuntimeValue, Halt>,
    ) -> Evaluation {
        let outcome = run(&mut self);
        Evaluation {
            halt: outcome.as_ref().err().copied(),
            result: outcome.unwrap_or_else(Halt::into_absent),
            remaining_fuel: self.remaining_fuel,
        }
    }

    /// Run an observable write to evaluation-local state. Foreign functions
    /// evaluate arguments and check applicability before entering this operation.
    pub fn effect<T>(&mut self, run: impl FnOnce() -> T) -> T {
        self.host.effect();
        run()
    }

    pub(crate) fn run(self, expression: &Value) -> Evaluation {
        self.conclude(|context| {
            let expression = context.lower_source(expression, SourceOrigin::Input(Vec::new()));
            let environment = context.empty_environment();
            context.eval(expression, &environment)
        })
    }

    pub fn eval_to_value(
        &mut self,
        expression: Expression,
        environment: &Environment,
    ) -> Result<Value, Halt> {
        self.eval(expression, environment)
            .map(RuntimeValue::into_value)
    }

    pub fn eval_value_runtime(
        &mut self,
        expression: &Value,
        environment: &Environment,
    ) -> Result<RuntimeValue, Halt> {
        let expression = self.lower_unattributed_source(expression);
        self.eval(expression, environment)
    }

    /// Explicitly interpret runtime-held syntax in this evaluation. Embedded
    /// native closures keep their code origins and lexical captures.
    pub fn eval_runtime_code(
        &mut self,
        expression: &RuntimeValue,
        environment: &Environment,
    ) -> Result<RuntimeValue, Halt> {
        let expression = self.lower_runtime_code(expression);
        self.eval(expression, environment)
    }

    /// An environment with no bindings, over this evaluation's index table.
    pub fn empty_environment(&self) -> Environment {
        Environment::with_indices(self.indices.clone())
    }

    /// Decode GID bindings into a shared lexical environment.
    pub fn environment(&self, value: &Value) -> Option<Environment> {
        let fields = value.as_record()?;
        Some(
            self.empty_environment()
                .extended(fields.iter().map(|(cell, value)| (*cell, value.clone()))),
        )
    }

    pub fn eval(
        &mut self,
        expression: Expression,
        environment: &Environment,
    ) -> Result<RuntimeValue, Halt> {
        self.burn()?;
        self.eval_burned(expression, environment)
    }

    /// Evaluate an expression a caller will read as a number, skipping
    /// the owned `RuntimeValue` round trip when the answer is already
    /// an unboxed f64. Fuel and results match
    /// [`Self::eval`] followed by [`RuntimeValue::as_f64`] exactly.
    pub fn eval_f64(
        &mut self,
        expression: Expression,
        environment: &Environment,
    ) -> Result<Option<f64>, Halt> {
        self.burn()?;
        match unboxed_f64(&expression, environment) {
            Some(number) => Ok(Some(number)),
            None => self
                .eval_burned(expression, environment)
                .map(|value| value.as_f64()),
        }
    }

    /// [`Self::eval_f64`], but answering with the value itself when it isn't
    /// an f64, for a caller that reports why.
    pub fn eval_f64_or_value(
        &mut self,
        expression: Expression,
        environment: &Environment,
    ) -> Result<Result<f64, RuntimeValue>, Halt> {
        self.burn()?;
        match unboxed_f64(&expression, environment) {
            Some(number) => Ok(Ok(number)),
            None => self
                .eval_burned(expression, environment)
                .map(|value| value.as_f64().ok_or(value)),
        }
    }

    fn eval_burned(
        &mut self,
        expression: Expression,
        environment: &Environment,
    ) -> Result<RuntimeValue, Halt> {
        if self.remaining_depth == 0 {
            return Err(Halt::DepthExhausted);
        }
        let compiled = match self.compiled.get(&expression) {
            Some(compiled) => compiled.clone(),
            None => {
                let compiled = self.compile(expression.clone());
                self.compiled.insert(expression, compiled.clone());
                compiled
            }
        };
        self.remaining_depth -= 1;
        let result = compiled(self, environment);
        self.remaining_depth += 1;
        result
    }

    /// Fuel is part of Grap's contract, not this implementation's: one
    /// burn per expression evaluation that the plain evaluator and each
    /// function's documented argument consumption would perform. An
    /// implementation shortcut that skips an evaluation must burn its
    /// fuel anyway, as the prepared-call path already does.
    pub fn burn(&mut self) -> Result<(), Halt> {
        self.remaining_fuel = self.remaining_fuel.saturating_sub(1);
        if self.remaining_fuel == 0 {
            Err(Halt::FuelExhausted)
        } else {
            Ok(())
        }
    }

    pub(crate) fn eval_cell_at(
        &mut self,
        cell: CellId,
        index: CellIndex,
        environment: &Environment,
    ) -> Result<RuntimeValue, Halt> {
        match environment.get_at(&self.indices, cell, index) {
            Some(value) => Ok(unbox(value.clone())),
            // Its definition, lowered once this evaluation, is evaluated
            // where it's referenced.
            None => {
                let expression = match self.definitions.get(index.0) {
                    Some(Some(expression)) => expression.clone(),
                    _ => match self.host.definition(cell) {
                        Some(value) => self.lowered_cell(cell, &value),
                        None => return Ok(missing_cell(cell)),
                    },
                };
                self.eval(expression, environment)
            }
        }
    }
}

/// How deeply evaluations may nest. Each level is a native call taking up to
/// about 3 KB of stack, and Grap runs on stacks as small as the browser's
/// 1 MB, so a deeper recursion halts rather than overflowing. The deepest
/// program measured in October 2026 nested 63 levels.
pub const MAX_DEPTH: usize = 256;

/// `expression`'s f64 when it's already one without evaluating: a number
/// literal, which lowering made native, or a variable bound to one.
fn unboxed_f64(expression: &Expression, environment: &Environment) -> Option<f64> {
    match &expression.0.form {
        // Literals are lowered to their native values.
        Form::Ready(value) => native_f64(value),
        Form::Cell(cell) => environment.get_runtime(*cell).and_then(native_f64),
        _ => None,
    }
}

fn native_f64(value: &RuntimeValue) -> Option<f64> {
    match &value.0 {
        RuntimeValueKind::Unboxed(unboxed) => match unboxed.native() {
            Native::F64(number) => Some(*number),
            _ => None,
        },
        _ => None,
    }
}

/// The absence a reference answers when `cell` has no definition.
pub(crate) fn missing_cell(cell: CellId) -> RuntimeValue {
    RuntimeValue::from(absent::with_detail(
        absent::MISSING_CELL,
        absent::CELL,
        cell.into(),
    ))
}

/// `value` in its unboxed form when it is a record following a
/// privileged convention.
pub(crate) fn unbox(value: RuntimeValue) -> RuntimeValue {
    match &value.0 {
        RuntimeValueKind::Data(Value::Record(fields)) => match Unboxed::recognize(fields) {
            Some(unboxed) => RuntimeValue::new(RuntimeValueKind::Unboxed(unboxed)),
            None => value,
        },
        _ => value,
    }
}
