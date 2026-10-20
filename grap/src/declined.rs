//! A function's arguments checked together, as a declaration of what each
//! must be: the function runs when every one is, and otherwise declines,
//! naming each that wasn't under its field.
//!
//! ```ignore
//! match declined::all(NOT_F64, ((START, start), (END, end))) {
//!     Ok((start, end)) => ...,
//!     Err(declined) => declined,
//! }
//! ```

use crate::{RuntimeValue, absent};
use gid::CellId;

/// Arguments checked together: each its field, and its value or what it
/// held instead.
pub trait Arguments {
    type Values;
    /// Every argument's value, or each field whose argument had none, with
    /// what it held.
    fn values(self) -> Result<Self::Values, Vec<(CellId, RuntimeValue)>>;
}

/// `arguments`' values, or an absent for `reason` carrying, under its field,
/// what each argument that had none held instead.
pub fn all<A: Arguments>(reason: CellId, arguments: A) -> Result<A::Values, RuntimeValue> {
    arguments.values().map_err(|declined| {
        absent::caused_by_all(
            reason,
            declined
                .into_iter()
                .map(|(field, held)| (field, held.into_value())),
        )
        .into()
    })
}

macro_rules! arguments {
    ($($value:ident: $type:ident),+) => {
        impl<$($type),+> Arguments for ($((CellId, Result<$type, RuntimeValue>),)+) {
            type Values = ($($type,)+);

            fn values(self) -> Result<Self::Values, Vec<(CellId, RuntimeValue)>> {
                match self {
                    ($((_, Ok($value)),)+) => Ok(($($value,)+)),
                    ($($value,)+) => {
                        let mut declined = Vec::new();
                        $(
                            if let (field, Err(held)) = $value {
                                declined.push((field, held));
                            }
                        )+
                        Err(declined)
                    }
                }
            }
        }
    };
}

arguments!(a: A);
arguments!(a: A, b: B);
arguments!(a: A, b: B, c: C);
arguments!(a: A, b: B, c: C, d: D);
