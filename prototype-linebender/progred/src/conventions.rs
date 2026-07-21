//! Editor policy over the data model's identity metadata. The old
//! NAME well-known cell and its name-names-itself bootstrap died
//! 2026-07-20 when names joined the cell table — a name now lives
//! beside the value, naming the identity rather than hiding in its
//! current record.

use crate::sources::Sources;
use progred_graph::{CellId, Cells};
use std::rc::Rc;

/// The built-in library: read under every document through
/// [`Sources`] — never written, never saved. Empty since names moved
/// into the table; real loadable libraries will compose here (they
/// are read-only, so composition is `Cells::merge`).
pub fn library() -> Cells {
    Cells::new()
}

/// The editor's name policy: every display-name lookup goes through
/// this one function, making "what counts as a name" editor state —
/// expandable (computed names, richer naming conventions layered
/// over the table). The Raw view is not a policy of its own: lookups
/// derive from the editor's one raw bit and skip the policy
/// entirely, so nothing is ever swapped.
#[derive(Clone)]
pub struct Names(Rc<dyn Fn(&Sources, CellId) -> Option<String>>);

impl Names {
    /// The default: the identity table's own name.
    pub fn table() -> Self {
        Self(Rc::new(|sources, cell| {
            sources.name(cell).map(str::to_owned)
        }))
    }

    pub fn of(&self, sources: &Sources, cell: CellId) -> Option<String> {
        (self.0)(sources, cell)
    }
}

impl Default for Names {
    fn default() -> Self {
        Self::table()
    }
}

/// The editor's one display-name read. Names are identity DATA, so
/// the Raw view shows the table's own name directly — what stands
/// down in Raw is the POLICY, which future convention layers
/// (computed names, per-library conventions) will vary; today the
/// policy reads the same table, so the two sides agree.
pub fn display_name(
    sources: &Sources,
    names: &Names,
    raw: bool,
    cell: CellId,
) -> Option<String> {
    if raw {
        sources.name(cell).map(str::to_owned)
    } else {
        names.of(sources, cell)
    }
}
