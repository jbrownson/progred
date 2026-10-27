//! Operations the evaluator runs natively. A library registers one under its
//! own cell as an ordinary foreign function, with its own labels and reasons;
//! a call that resolves to it runs the operation directly rather than through
//! the general foreign-call machinery. Lookup, shadowing, fuel and effects
//! are those of any foreign call.

use crate::arguments::{Argument, Parse, Signature};
use crate::{Context, Environment, Expression, ForeignFunction, Halt, RuntimeValue, absent};
use gid::CellId;
use std::rc::Rc;

#[derive(Clone, Copy)]
pub enum Primitive {
    /// An operation on the numbers under `left` and `right`.
    Binary {
        operation: BinaryOperation,
        left: CellId,
        right: CellId,
    },
    /// An operation on the number under `operand`.
    Unary {
        operation: UnaryOperation,
        operand: CellId,
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
pub trait Number: Parse + Copy + 'static {
    fn runtime(self) -> RuntimeValue;

    fn binary(operation: Binary<Self>) -> BinaryOperation;

    fn unary(operation: Unary<Self>) -> UnaryOperation;
}

impl Number for f64 {
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

/// A call site's operands, in the primitive's label order.
pub(crate) type Operands = Rc<[Argument]>;

impl Primitive {
    /// The foreign function a library registers for this operation.
    pub fn function(self) -> ForeignFunction {
        ForeignFunction::staged(move |context, call| {
            let operands = self.operands(context, call);
            let call = call.clone();
            Rc::new(move |context, environment| self.run(context, &call, &operands, environment))
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
            .map(|label| context.argument(call, label))
            .collect()
    }

    pub(crate) fn run(
        &self,
        context: &mut Context,
        call: &Expression,
        operands: &[Argument],
        environment: &Environment,
    ) -> Result<RuntimeValue, Halt> {
        match self {
            Self::Binary {
                operation,
                left,
                right,
            } => {
                let operands = [(*left, &operands[0]), (*right, &operands[1])];
                match operation {
                    BinaryOperation::F64(operation) => {
                        binary(operation, context, call, operands, environment)
                    }
                    BinaryOperation::F32(operation) => {
                        binary(operation, context, call, operands, environment)
                    }
                    BinaryOperation::U64(operation) => {
                        binary(operation, context, call, operands, environment)
                    }
                }
            }
            Self::Unary { operation, operand } => {
                let operand = [(*operand, &operands[0])];
                match operation {
                    UnaryOperation::F64(operation) => {
                        unary(operation, context, call, operand, environment)
                    }
                    UnaryOperation::F32(operation) => {
                        unary(operation, context, call, operand, environment)
                    }
                    UnaryOperation::U64(operation) => {
                        unary(operation, context, call, operand, environment)
                    }
                }
            }
        }
    }
}

fn binary<N: Number>(
    operation: &Binary<N>,
    context: &mut Context,
    call: &Expression,
    operands: [(CellId, &Argument); 2],
    environment: &Environment,
) -> Result<RuntimeValue, Halt> {
    Ok(
        match <(N, N)>::parse(context, call, operands, environment)? {
            Ok((left, right)) => match operation {
                Binary::Arithmetic(apply) => apply(left, right).runtime(),
                Binary::Checked(apply, failure) => apply(left, right)
                    .map(N::runtime)
                    .unwrap_or_else(|| absent::value(*failure).into()),
                Binary::Comparison(apply) => RuntimeValue::bool(apply(left, right)),
            },
            Err(invalid) => invalid,
        },
    )
}

fn unary<N: Number>(
    operation: &Unary<N>,
    context: &mut Context,
    call: &Expression,
    operand: [(CellId, &Argument); 1],
    environment: &Environment,
) -> Result<RuntimeValue, Halt> {
    Ok(match <(N,)>::parse(context, call, operand, environment)? {
        Ok((value,)) => match operation {
            Unary::Arithmetic(apply) => apply(value).runtime(),
            Unary::Predicate(apply) => RuntimeValue::bool(apply(value)),
        },
        Err(invalid) => invalid,
    })
}
