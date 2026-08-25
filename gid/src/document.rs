//! A durable GID document: one root value and its cell definitions.

use crate::{Cells, Step, Value};

/// A document's root value plus the table holding every cell's current
/// value. An absent table entry is a bare cell. Cloning is O(1): the
/// table and its values share persistent structure.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Document {
    pub root: Option<Value>,
    pub cells: Cells,
}

/// A traversal route from a document root through record fields, list
/// elements, and cell references.
pub type Path = Vec<Step>;
