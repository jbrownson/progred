//! Fidget's offers to a hole no projection supplies them for: an empty
//! document can start as a scene.

use crate::display::{Completion, CompletionKind, CompletionRequest, CompletionScope};

pub(super) fn offers(request: &CompletionRequest<'_>) -> Option<Vec<Completion>> {
    (request.scope == CompletionScope::Everything
        && request.kind == CompletionKind::Value
        && request.path.is_empty())
    .then(|| vec![super::root_completion()])
}
