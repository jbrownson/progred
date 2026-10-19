//! Fidget's offers: an empty document can start as a scene.

use super::{ID, vocabulary};
use crate::display::{Completion, CompletionKind, CompletionRequest, CompletionScope};
use crate::libraries::{presentation, workspace};
use gid::Value;

pub(super) fn offers(request: &CompletionRequest<'_>) -> Option<Vec<Completion>> {
    (request.scope == CompletionScope::Start && request.kind == CompletionKind::Value)
        .then(|| vec![root_completion()])
}

fn root_completion() -> Completion {
    crate::libraries::completion::generated(
        vocabulary::FIDGET,
        || {
            let cell = gid::new_cell_id();
            Value::record([
                (vocabulary::FIDGET, cell.into()),
                (
                    workspace::vocabulary::PANES,
                    Value::record([(
                        workspace::vocabulary::LEFT,
                        Value::list([Value::record([
                            (presentation::vocabulary::VALUE, cell.into()),
                            (
                                presentation::vocabulary::VIEWPORT,
                                vocabulary::PREVIEW_3D.into(),
                            ),
                        ])]),
                    )]),
                ),
            ])
        },
        Some(crate::selection::pending_at(&[
            gid::Step::Key(vocabulary::FIDGET),
            gid::Step::Follow(gid::Resolution::Document),
        ])),
    )
    .with_aliases(["sdf"])
    .with_detail(ID)
}
