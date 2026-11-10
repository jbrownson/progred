//! Control's records, as tests across the crate build them.

use super::vocabulary;
use gid::Value;

/// An unquote of `expression`, as a quoted template holds one.
pub(crate) fn unquote(expression: Value) -> Value {
    ::grap::isa::record(
        vocabulary::kind::UNQUOTE,
        [(vocabulary::UNQUOTE, expression)],
    )
}
