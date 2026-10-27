//! A durable GID document: one root value and its cell definitions.

use crate::{Cells, Value};

/// A document's root value plus the table holding every cell's current
/// value. An absent table entry is a bare cell. Cloning is O(1): the
/// table and its values share persistent structure.
#[derive(Clone, Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Document {
    pub root: Option<Value>,
    pub cells: Cells,
}
