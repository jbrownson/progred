use crate::{CellId, Position};

/// A traversal route from a document root through record fields, list
/// elements, and cell references.
pub type Path = Vec<Step>;

/// A projection path step: into a record field, into a list element,
/// or through a cell reference to the cell's definition, wherever lookup
/// finds it. A step that doesn't resolve leaves a stale path, which the
/// editor tolerates.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Step {
    Key(CellId),
    Element(Position),
    Follow,
}
