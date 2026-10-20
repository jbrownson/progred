//! How a function written in Rust reads its arguments: each found, evaluated
//! and parsed into the Rust type the function works with. The function runs
//! when every one parses; otherwise its result is "invalid arguments",
//! carrying under its label why each that didn't couldn't: a missing
//! argument, its type's reason ("not f64", carrying the value), or the absent
//! it already was, passed on unchanged as the cause.
//!
//! ```ignore
//! let (start, end): (f64, f64) =
//!     match context.arguments(call, [START, END], environment)? {
//!         Ok(arguments) => arguments,
//!         Err(invalid) => return Ok(invalid),
//!     };
//! ```

use crate::{Context, Environment, Expression, Halt, RuntimeValue, absent};
use gid::CellId;

/// A Rust type an argument parses into.
pub trait Parse: Sized {
    /// `value` as this type, or why it isn't one: an absent `value` as it
    /// is, since what failed before it is the reason.
    fn parse(value: RuntimeValue) -> Result<Self, RuntimeValue>;

    /// `expression`'s value as this type. A type overrides it where it reads
    /// faster without the value being built.
    fn eval(
        context: &mut Context,
        expression: Expression,
        environment: &Environment,
    ) -> Result<Result<Self, RuntimeValue>, Halt> {
        context.eval(expression, environment).map(Self::parse)
    }
}

/// Why `value` isn't what a type needs: `reason`, carrying it, unless it's
/// already absent.
#[cold]
pub fn not(reason: CellId, value: RuntimeValue) -> RuntimeValue {
    if value.absent_reason().is_some() {
        value
    } else {
        absent::with_detail(reason, absent::VALUE, value.into_value()).into()
    }
}

/// Arguments that parse together: a tuple of [`Parse`] types, one for each
/// label.
pub trait Signature<const N: usize>: Sized {
    /// Each argument, given under its label or not, parsed as its type; or
    /// "invalid arguments" carrying why each that didn't parse couldn't.
    fn parse(
        context: &mut Context,
        arguments: [(CellId, Option<Expression>); N],
        environment: &Environment,
    ) -> Result<Result<Self, RuntimeValue>, Halt>;
}

impl Context<'_> {
    /// [`Signature::parse`] of the arguments `call` gives under `labels`.
    pub fn arguments<S: Signature<N>, const N: usize>(
        &mut self,
        call: &Expression,
        labels: [CellId; N],
        environment: &Environment,
    ) -> Result<Result<S, RuntimeValue>, Halt> {
        let arguments = labels.map(|label| (label, self.field(call, label)));
        S::parse(self, arguments, environment)
    }
}

#[inline]
fn argument<T: Parse>(
    context: &mut Context,
    label: CellId,
    expression: Option<Expression>,
    environment: &Environment,
) -> Result<Result<T, RuntimeValue>, Halt> {
    match expression {
        Some(expression) => T::eval(context, expression, environment),
        None => Ok(Err(context.missing_runtime_argument(label))),
    }
}

/// "invalid arguments", carrying why each argument couldn't be used under its
/// label: off the path a call takes when its arguments are fine.
#[cold]
#[inline(never)]
fn invalid_arguments(invalid: Vec<(CellId, RuntimeValue)>) -> RuntimeValue {
    absent::caused_by_all(
        absent::INVALID_ARGUMENTS,
        invalid
            .into_iter()
            .map(|(label, why)| (label, why.into_value())),
    )
    .into()
}

macro_rules! signature {
    ($count:literal: $($type:ident $label:ident $value:ident),+) => {
        impl<$($type: Parse),+> Signature<$count> for ($($type,)+) {
            #[inline]
            fn parse(
                context: &mut Context,
                [$(($label, $value)),+]: [(CellId, Option<Expression>); $count],
                environment: &Environment,
            ) -> Result<Result<Self, RuntimeValue>, Halt> {
                // Only each argument's value stays live, so the values that
                // parse aren't moved through the results around them.
                let mut invalid = Vec::new();
                $(
                    let $value = match argument::<$type>(context, $label, $value, environment)? {
                        Ok(value) => Some(value),
                        Err(why) => {
                            invalid.push(($label, why));
                            None
                        }
                    };
                )+
                Ok(match ($($value,)+) {
                    ($(Some($value),)+) => Ok(($($value,)+)),
                    _ => Err(invalid_arguments(invalid)),
                })
            }
        }
    };
}

signature!(1: A a_label a);
signature!(2: A a_label a, B b_label b);
signature!(3: A a_label a, B b_label b, C c_label c);
signature!(4: A a_label a, B b_label b, C c_label c, D d_label d);

impl Parse for f64 {
    fn parse(value: RuntimeValue) -> Result<Self, RuntimeValue> {
        value
            .as_f64()
            .ok_or_else(|| not(crate::f64::NOT_F64, value))
    }

    #[inline]
    fn eval(
        context: &mut Context,
        expression: Expression,
        environment: &Environment,
    ) -> Result<Result<Self, RuntimeValue>, Halt> {
        Ok(context
            .eval_f64_or_value(expression, environment)?
            .map_err(|value| not(crate::f64::NOT_F64, value)))
    }
}

impl Parse for f32 {
    fn parse(value: RuntimeValue) -> Result<Self, RuntimeValue> {
        value
            .as_f32()
            .ok_or_else(|| not(crate::f32::NOT_F32, value))
    }
}

impl Parse for u64 {
    fn parse(value: RuntimeValue) -> Result<Self, RuntimeValue> {
        value
            .as_u64()
            .ok_or_else(|| not(crate::u64::NOT_U64, value))
    }
}

/// Any value at all, absent or not: an argument the function takes as it is.
impl Parse for RuntimeValue {
    fn parse(value: RuntimeValue) -> Result<Self, RuntimeValue> {
        Ok(value)
    }
}

/// Any value at all, as GID.
impl Parse for gid::Value {
    fn parse(value: RuntimeValue) -> Result<Self, RuntimeValue> {
        Ok(value.into_value())
    }
}

impl Parse for bool {
    fn parse(value: RuntimeValue) -> Result<Self, RuntimeValue> {
        crate::logic::read(&value.to_value()).ok_or_else(|| not(crate::logic::NOT_BOOL, value))
    }
}
