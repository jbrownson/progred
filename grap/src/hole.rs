//! The hole convention: a reference to [`HOLE`] stands where a shape requires
//! a value that isn't written yet. An editor library owns it; Grap knows it
//! so that evaluating a hole answers an absent, whose reason is the hole,
//! rather than reading the cell.

use gid::{CellId, Value};

pub const HOLE: CellId = CellId::from_u128(0x4b0456f43e554676a5a9ed44a358d4d3);

/// What a hole evaluates to.
pub fn absent() -> Value {
    crate::absent::value(HOLE)
}
