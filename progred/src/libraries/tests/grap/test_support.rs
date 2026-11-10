//! Grap's own records and offers, as tests across the crate build and read
//! them.

use super::{suggestions, vocabulary};
use gid::Value;

/// `expression` shown with its result.
pub(crate) fn evaluation(expression: Value) -> Value {
    ::grap::isa::record(
        vocabulary::kind::EVALUATION,
        [(::grap::vocabulary::EXPRESSION, expression)],
    )
}

/// What a Grap slot offers, for tests that read its list without a picker.
pub(crate) fn slot_offers() -> crate::display::CompletionProvider {
    std::rc::Rc::new(suggestions::offers)
}
