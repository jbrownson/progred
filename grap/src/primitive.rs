//! Operations the evaluator runs natively. A library registers one under its
//! own cell as an ordinary foreign function, with its own labels and reasons;
//! a call that resolves to it runs the operation directly rather than through
//! the general foreign-call machinery. Resolution, shadowing, fuel and effects
//! are those of any foreign call.

use crate::{
    Context, Environment, Expression, ForeignFunction, Halt, RuntimeValue, absent, declined,
};
use gid::CellId;
use std::rc::Rc;

#[derive(Clone, Copy)]
pub enum Primitive {
    /// An operation on the numbers under `left` and `right`, declining with
    /// `not`, and each operand that isn't one, when either isn't.
    Binary {
        operation: BinaryOperation,
        left: CellId,
        right: CellId,
        not: CellId,
    },
    /// An operation on the number under `operand`, declining with `not`
    /// when it isn't one.
    Unary {
        operation: UnaryOperation,
        operand: CellId,
        not: CellId,
    },
}

/// A binary operation on one of the number types Grap holds natively.
#[derive(Clone, Copy)]
pub enum BinaryOperation {
    F64(Binary<f64>),
    F32(Binary<f32>),
    U64(Binary<u64>),
}

#[derive(Clone, Copy)]
pub enum UnaryOperation {
    F64(Unary<f64>),
    F32(Unary<f32>),
    U64(Unary<u64>),
}

#[derive(Clone, Copy)]
pub enum Binary<N> {
    Arithmetic(fn(N, N) -> N),
    /// Fails with the given reason instead of producing a result.
    Checked(fn(N, N) -> Option<N>, CellId),
    Comparison(fn(N, N) -> bool),
}

#[derive(Clone, Copy)]
pub enum Unary<N> {
    Arithmetic(fn(N) -> N),
    Predicate(fn(N) -> bool),
}

/// A number type Grap holds natively.
pub trait Number: Copy + 'static {
    /// `expression`'s value as this number, or the value itself when it
    /// isn't one.
    fn eval(
        context: &mut Context,
        expression: Expression,
        environment: &Environment,
    ) -> Result<Result<Self, RuntimeValue>, Halt>;

    fn runtime(self) -> RuntimeValue;

    fn binary(operation: Binary<Self>) -> BinaryOperation;

    fn unary(operation: Unary<Self>) -> UnaryOperation;
}

impl Number for f64 {
    fn eval(
        context: &mut Context,
        expression: Expression,
        environment: &Environment,
    ) -> Result<Result<Self, RuntimeValue>, Halt> {
        context.eval_f64_or_value(expression, environment)
    }

    fn runtime(self) -> RuntimeValue {
        RuntimeValue::f64(self)
    }

    fn binary(operation: Binary<Self>) -> BinaryOperation {
        BinaryOperation::F64(operation)
    }

    fn unary(operation: Unary<Self>) -> UnaryOperation {
        UnaryOperation::F64(operation)
    }
}

impl Number for f32 {
    fn eval(
        context: &mut Context,
        expression: Expression,
        environment: &Environment,
    ) -> Result<Result<Self, RuntimeValue>, Halt> {
        let value = context.eval(expression, environment)?;
        Ok(value.as_f32().ok_or(value))
    }

    fn runtime(self) -> RuntimeValue {
        RuntimeValue::f32(self)
    }

    fn binary(operation: Binary<Self>) -> BinaryOperation {
        BinaryOperation::F32(operation)
    }

    fn unary(operation: Unary<Self>) -> UnaryOperation {
        UnaryOperation::F32(operation)
    }
}

impl Number for u64 {
    fn eval(
        context: &mut Context,
        expression: Expression,
        environment: &Environment,
    ) -> Result<Result<Self, RuntimeValue>, Halt> {
        let value = context.eval(expression, environment)?;
        Ok(value.as_u64().ok_or(value))
    }

    fn runtime(self) -> RuntimeValue {
        RuntimeValue::u64(self)
    }

    fn binary(operation: Binary<Self>) -> BinaryOperation {
        BinaryOperation::U64(operation)
    }

    fn unary(operation: Unary<Self>) -> UnaryOperation {
        UnaryOperation::U64(operation)
    }
}

/// A call site's operand expressions, in the primitive's label order.
pub(crate) type Operands = Rc<[Option<Expression>]>;

impl Primitive {
    /// The foreign function a library registers for this operation.
    pub fn function(self) -> ForeignFunction {
        ForeignFunction::staged(move |context, call| {
            let operands = self.operands(context, call);
            Rc::new(move |context, environment| self.run(context, &operands, environment))
        })
        .parameters(self.labels())
        .tracked()
        .as_primitive(self)
    }

    fn labels(&self) -> Vec<CellId> {
        match *self {
            Self::Binary { left, right, .. } => vec![left, right],
            Self::Unary { operand, .. } => vec![operand],
        }
    }

    /// The call's operands, found once per call site.
    pub(crate) fn operands(&self, context: &Context, call: &Expression) -> Operands {
        self.labels()
            .into_iter()
            .map(|label| context.field(call, label))
            .collect()
    }

    pub(crate) fn run(
        &self,
        context: &mut Context,
        operands: &[Option<Expression>],
        environment: &Environment,
    ) -> Result<RuntimeValue, Halt> {
        match self {
            Self::Binary {
                operation,
                left,
                right,
                not,
            } => {
                let operands = [(*left, &operands[0]), (*right, &operands[1])];
                match operation {
                    BinaryOperation::F64(operation) => {
                        binary(operation, context, operands, *not, environment)
                    }
                    BinaryOperation::F32(operation) => {
                        binary(operation, context, operands, *not, environment)
                    }
                    BinaryOperation::U64(operation) => {
                        binary(operation, context, operands, *not, environment)
                    }
                }
            }
            Self::Unary {
                operation,
                operand,
                not,
            } => {
                let operand = (*operand, &operands[0]);
                match operation {
                    UnaryOperation::F64(operation) => {
                        unary(operation, context, operand, *not, environment)
                    }
                    UnaryOperation::F32(operation) => {
                        unary(operation, context, operand, *not, environment)
                    }
                    UnaryOperation::U64(operation) => {
                        unary(operation, context, operand, *not, environment)
                    }
                }
            }
        }
    }
}

fn binary<N: Number>(
    operation: &Binary<N>,
    context: &mut Context,
    [(left, left_operand), (right, right_operand)]: [(CellId, &Option<Expression>); 2],
    not: CellId,
    environment: &Environment,
) -> Result<RuntimeValue, Halt> {
    let left_number = number::<N>(context, left, left_operand, environment)?;
    let right_number = number::<N>(context, right, right_operand, environment)?;
    Ok(
        match declined::all(not, ((left, left_number), (right, right_number))) {
            Ok((left, right)) => match operation {
                Binary::Arithmetic(apply) => apply(left, right).runtime(),
                Binary::Checked(apply, failure) => apply(left, right)
                    .map(N::runtime)
                    .unwrap_or_else(|| absent::value(*failure).into()),
                Binary::Comparison(apply) => RuntimeValue::bool(apply(left, right)),
            },
            Err(declined) => declined,
        },
    )
}

fn unary<N: Number>(
    operation: &Unary<N>,
    context: &mut Context,
    (operand, expression): (CellId, &Option<Expression>),
    not: CellId,
    environment: &Environment,
) -> Result<RuntimeValue, Halt> {
    let value = number::<N>(context, operand, expression, environment)?;
    Ok(match declined::all(not, ((operand, value),)) {
        Ok((value,)) => match operation {
            Unary::Arithmetic(apply) => apply(value).runtime(),
            Unary::Predicate(apply) => RuntimeValue::bool(apply(value)),
        },
        Err(declined) => declined,
    })
}

/// The number the operand under `label` evaluates to, or what it held
/// instead: the missing-argument absent where the call has none.
fn number<N: Number>(
    context: &mut Context,
    label: CellId,
    expression: &Option<Expression>,
    environment: &Environment,
) -> Result<Result<N, RuntimeValue>, Halt> {
    match expression {
        Some(expression) => N::eval(context, expression.clone(), environment),
        None => Ok(Err(context.missing_runtime_argument(label))),
    }
}
