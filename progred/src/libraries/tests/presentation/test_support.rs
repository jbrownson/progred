//! Presentation's records, as tests across the crate build them.

use super::vocabulary;

/// `value` as `projection` draws it, in a pane.
pub(crate) fn projected(value: gid::Value, projection: gid::Value) -> gid::Value {
    ::grap::isa::record(
        vocabulary::kind::PROJECTED,
        [
            (vocabulary::VALUE, value),
            (vocabulary::PROJECTION, projection),
        ],
    )
}
