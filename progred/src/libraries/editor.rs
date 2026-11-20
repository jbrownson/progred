//! The editor's own vocabulary: its selection, each site's state and folds,
//! paths as data, the rows and columns of parts, and the loaded libraries,
//! any of which an empty document can start as, to look through. The editor defines these cells beside the
//! code that interprets them, and supplies their effects while dispatching;
//! this library names them.

use crate::display::Host;
use crate::libraries::{Library, name};

pub mod path;
pub mod selection;
pub mod site;
pub mod workspace;

pub use crate::workspace::EDITOR_LIBRARY as ID;

pub fn library<W: Host>() -> Library<W> {
    name::named_library(
        ID,
        "editor",
        [
            (selection::vocabulary::GET, "selection get"),
            (selection::vocabulary::SET, "selection set"),
            (selection::vocabulary::PATH, "selection path"),
            (selection::vocabulary::STAGE, "stage"),
            (selection::vocabulary::EDGE, "edge"),
            (selection::vocabulary::PENDING, "pending"),
            (selection::vocabulary::LABEL, "label"),
            (
                selection::vocabulary::NO_SELECTION,
                "no selection at this site",
            ),
            (site::vocabulary::GET, "get"),
            (site::vocabulary::SET, "set"),
            (site::vocabulary::PATH, "site path"),
            (site::vocabulary::VALUE, "value"),
            (site::vocabulary::FOLD, "fold"),
            (site::vocabulary::FOLDED, "folded"),
            (site::vocabulary::EXPANDED, "expanded"),
            (site::vocabulary::NO_STATE, "no site state"),
            (path::vocabulary::KEY, "key"),
            (path::vocabulary::ELEMENT, "element"),
            (path::vocabulary::FOLLOW, "follow"),
            (path::vocabulary::INDEXABLE, "indexable"),
            (path::vocabulary::INVALID_PATH, "invalid path"),
            (crate::location::vocabulary::kind::LOCATION, "location"),
            (crate::location::vocabulary::IN, "in"),
            (crate::location::vocabulary::STEPS, "steps"),
            (crate::location::vocabulary::NOT_FOUND, "place not found"),
            (crate::occurrence::vocabulary::JUMP, "jump"),
            (crate::occurrence::vocabulary::COMPUTED, "computed"),
            (workspace::vocabulary::kind::ROW, "row"),
            (workspace::vocabulary::kind::COLUMN, "column"),
            (workspace::vocabulary::PARTS, "parts"),
        ],
        ::grap::ForeignFunctions::default(),
    )
    .with_absents([
        selection::vocabulary::NO_SELECTION,
        site::vocabulary::NO_STATE,
        path::vocabulary::INVALID_PATH,
        crate::location::vocabulary::NOT_FOUND,
    ])
    // A selection's stages and a fold's states.
    .with_data([
        selection::vocabulary::EDGE,
        selection::vocabulary::PENDING,
        selection::vocabulary::LABEL,
        site::vocabulary::FOLDED,
        site::vocabulary::EXPANDED,
    ])
    .with_kinds([
        (
            workspace::vocabulary::kind::ROW,
            vec![workspace::vocabulary::PARTS],
        ),
        (
            workspace::vocabulary::kind::COLUMN,
            vec![workspace::vocabulary::PARTS],
        ),
        (
            crate::location::vocabulary::kind::LOCATION,
            vec![crate::location::vocabulary::STEPS],
        ),
    ])
    .with_projections_for(workspace::projections())
    .with_completions(completions)
}

/// An empty document can start as any loaded library, by its name, to look
/// through what it defines, saying it's that library's own.
fn completions(
    request: &crate::display::CompletionRequest<'_>,
) -> Option<Vec<crate::display::Completion>> {
    use crate::display::{CompletionKind, CompletionScope};
    use crate::libraries::def::vocabulary::LIBRARY;
    (request.kind == CompletionKind::Value && request.scope == CompletionScope::Start)
        .then_some(())?;
    Some(
        (request.loaded)()
            .into_iter()
            .filter(|cell| {
                (request.resolve)(*cell)
                    .and_then(|definition| definition.value.as_record())
                    .and_then(::grap::isa::of)
                    == Some(LIBRARY)
            })
            // A library's description is the library's own, as anything it
            // defines is, so it names the library.
            .map(|library| {
                crate::libraries::completion::select(library, library.into()).with_detail(library)
            })
            .collect(),
    )
}
