//! Booleans: literals of the bool kind, `{isa: bool, bytes}`, one byte, 1
//! for true and 0 for false. Privileged like f64's so comparisons answer
//! natively and stay unboxed through calls and environments. Privileged
//! knowledge is an accelerator only: the convention must remain expressible
//! as an ordinary external library.

use gid::{CellId, Value};

pub const BOOL: CellId = CellId::from_u128(0xb2d13b64336a9e808133d0c7d5e4981e);
/// Why a value isn't a boolean, carrying it.
pub const NOT_BOOL: CellId = CellId::from_u128(0x0f168ae21a023bdd711339e3aa949f9a);

pub fn value(value: bool) -> Value {
    crate::literal::value(BOOL, encode(value))
}

pub fn read(value: &Value) -> Option<bool> {
    crate::literal::bytes(value, BOOL).and_then(decode)
}

/// The byte of a boolean literal holding `value`.
pub(crate) fn encode(value: bool) -> Vec<u8> {
    vec![u8::from(value)]
}

/// The boolean a literal's bytes hold.
pub(crate) fn decode(bytes: &[u8]) -> Option<bool> {
    match bytes {
        [0] => Some(false),
        [1] => Some(true),
        _ => None,
    }
}
