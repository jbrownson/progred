//! Fidget's offers: an empty document can start as a scene.

use super::{ID, vocabulary};
use crate::display::{Completion, CompletionKind, CompletionRequest, CompletionScope};
use crate::libraries::presentation;

pub(super) fn offers(request: &CompletionRequest<'_>) -> Option<Vec<Completion>> {
    (request.scope == CompletionScope::Start && request.kind == CompletionKind::Value)
        .then(|| vec![root_completion()])
}

fn root_completion() -> Completion {
    crate::libraries::completion::starter(vocabulary::FIDGET, |cell| {
        presentation::sized(cell.into(), vocabulary::PREVIEW_3D.into())
    })
    .with_aliases(["sdf"])
    .with_detail(ID)
}
