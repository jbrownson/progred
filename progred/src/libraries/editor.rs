//! The editor's own vocabulary: its selection, each site's state and folds,
//! paths as data, and the workspace's panes and libraries. The editor defines
//! these cells beside the code that interprets them, and supplies their
//! effects while dispatching; this library names them.

use crate::display::Host;
use crate::display::host::{Layout, ProjectionInput};
use crate::libraries::{Library, name};
use gid::{CellId, Step};

pub mod path;
pub mod selection;
pub mod site;
pub mod workspace;

pub const ID: CellId = CellId::from_u128(0x6fe840ea1fa7bff4153b107f7d5683dc);

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
            (path::vocabulary::DOCUMENT, "document"),
            (path::vocabulary::LIBRARY, "library"),
            (path::vocabulary::INDEXABLE, "indexable"),
            (path::vocabulary::INVALID_PATH, "invalid path"),
            (crate::location::vocabulary::kind::LOCATION, "location"),
            (crate::location::vocabulary::IN, "in"),
            (crate::location::vocabulary::STEPS, "steps"),
            (crate::location::vocabulary::NOT_FOUND, "place not found"),
            (crate::occurrence::vocabulary::JUMP, "jump"),
            (crate::occurrence::vocabulary::COMPUTED, "computed"),
            (crate::occurrence::vocabulary::PANE, "pane"),
            (workspace::vocabulary::MAIN, "main"),
            (workspace::vocabulary::PANES, "panes"),
            (workspace::vocabulary::LEFT, "left"),
            (workspace::vocabulary::RIGHT, "right"),
            (workspace::vocabulary::LIBRARIES, "libraries"),
            (workspace::vocabulary::KEYS, "keys"),
            (crate::library::vocabulary::kind::LIBRARY, "loaded library"),
            (crate::library::vocabulary::CELLS, "cells"),
        ],
        ::grap::ForeignFunctions::default(),
    )
    .with_projections_for([(
        crate::library::vocabulary::kind::LIBRARY,
        crate::display::runtime_partial(loaded_library),
    )])
}

/// A loaded library: its name over its cells, each by its name, a reference
/// that opens to the cell's definition.
fn loaded_library<W: Host>(
    input: &ProjectionInput<'_, W, ::grap::RuntimeValue>,
) -> Option<Layout<W>> {
    use crate::library::vocabulary::{CELLS, kind::LIBRARY};
    let by_name = crate::display::first(input, crate::display::runtime_partial(name::shallow_cell));
    crate::display::structure::kind_form(input, LIBRARY, [name::vocabulary::NAME, CELLS], |_| {
        Some(crate::display::col(
            0,
            8.0,
            [
                crate::display::descend(
                    [Step::Key(name::vocabulary::NAME)],
                    Some(name::projection(None)),
                    None,
                ),
                crate::display::descend(
                    [Step::Key(CELLS)],
                    Some(crate::display::structure::list_column(4.0, Some(by_name))),
                    None,
                ),
            ],
        ))
    })
}
