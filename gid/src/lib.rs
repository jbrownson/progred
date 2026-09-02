//! GID's native logical data model: durable cell identities, structural
//! values, documents, and traversal. Text notation is an external bridge;
//! GID's native binary storage stack will be designed directly from this model.

mod cell_id;
mod cells;
mod document;
pub mod position;
mod value;

pub use cell_id::{CellId, ParseCellIdError, new_cell_id};
pub use cells::Cells;
pub use document::{Document, Path};
pub use position::Position;
pub use value::{List, Record, Resolution, Step, Value, hex_string};
