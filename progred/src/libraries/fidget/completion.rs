//! Fidget's offers: an empty document can start as a scene.

use crate::display::{Completion, CompletionKind, CompletionRequest, CompletionScope};

pub(super) fn offers(request: &CompletionRequest<'_>) -> Option<Vec<Completion>> {
    (request.scope == CompletionScope::Start && request.kind == CompletionKind::Value)
        .then(|| vec![super::root_completion()])
}
