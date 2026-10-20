//! Operations the evaluator runs natively. A library registers one under its
//! own cell as an ordinary foreign function, with its own labels and reasons;
//! a call that resolves to it runs the operation directly rather than through
//! the general foreign-call machinery. Resolution, shadowing, fuel and effects
//! are those of any foreign call.

use crate::{Context, Environment, Expression, ForeignFunction, Halt, RuntimeValue, absent};
use gid::CellId;
use std::rc::Rc;

#[derive(Clone, Copy)]
pub enum Primitive {
    /// An operation on the f64s under `left` and `right`, declining with
    /// `left_not` or `right_not` for an operand that isn't one.
    Binary {
        operation: Binary,
        left: CellId,
        right: CellId,
        left_not: CellId,
        right_not: CellId,
    },
    /// An operation on the f64 under `operand`, declining with
    /// `operand_not` when it isn't one.
    Unary {
        operation: Unary,
        operand: CellId,
        operand_not: CellId,
    },
}

#[derive(Clone, Copy)]
pub enum Binary {
    Arithmetic(fn(f64, f64) -> f64),
    Comparison(fn(f64, f64) -> bool),
}

#[derive(Clone, Copy)]
pub enum Unary {
    Arithmetic(fn(f64) -> f64),
    Predicate(fn(f64) -> bool),
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
        .parameters(self.labels().iter().copied())
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
        match *self {
            Self::Binary {
                operation,
                left,
                right,
                left_not,
                right_not,
            } => {
                let (Some(left_operand), Some(right_operand)) = (&operands[0], &operands[1]) else {
                    let missing = if operands[0].is_none() { left } else { right };
                    return Ok(context.missing_runtime_argument(missing));
                };
                let left_operand = context.eval_f64_or_value(left_operand.clone(), environment)?;
                let right_operand =
                    context.eval_f64_or_value(right_operand.clone(), environment)?;
                Ok(match (left_operand, right_operand) {
                    (Ok(left), Ok(right)) => match operation {
                        Binary::Arithmetic(apply) => RuntimeValue::f64(apply(left, right)),
                        Binary::Comparison(apply) => RuntimeValue::bool(apply(left, right)),
                    },
                    (Err(left), _) => declined(left, left_not),
                    (_, Err(right)) => declined(right, right_not),
                })
            }
            Self::Unary {
                operation,
                operand,
                operand_not,
            } => {
                let Some(expression) = &operands[0] else {
                    return Ok(context.missing_runtime_argument(operand));
                };
                Ok(
                    match context.eval_f64_or_value(expression.clone(), environment)? {
                        Ok(value) => match operation {
                            Unary::Arithmetic(apply) => RuntimeValue::f64(apply(value)),
                            Unary::Predicate(apply) => RuntimeValue::bool(apply(value)),
                        },
                        Err(value) => declined(value, operand_not),
                    },
                )
            }
        }
    }
}

/// Why an operation declines for an operand that isn't its number: it's
/// `not` one, carrying the operand, so an absent operand's own reason shows
/// beneath it as the cause.
pub fn declined(operand: RuntimeValue, not: CellId) -> RuntimeValue {
    absent::caused_by(not, absent::VALUE, operand.into_value()).into()
}
