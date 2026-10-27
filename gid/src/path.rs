use crate::{CellId, Position};

/// A traversal route from a document root through record fields, list
/// elements, and cell references.
pub type Path = Vec<Step>;

/// The source a followed cell resolves through: the document or a loaded
/// library, named by stable identity rather than load order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Resolution {
    Document,
    Library(CellId),
}

/// A projection path step: into a record field, into a list element,
/// or through a link to one of the cell's resolved values. A step that
/// doesn't resolve leaves a stale path, which the editor tolerates.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Step {
    Key(CellId),
    Element(Position),
    Follow(Resolution),
}
