//! GID's native logical data model: durable cell identities, structural
//! values, documents, and traversal. Text notation is an external bridge;
//! GID's native binary storage stack will be designed directly from this model.

mod blob;
mod cell_id;
mod cells;
mod document;
mod entries;
mod hex;
mod list;
mod path;
pub mod position;
mod record;
mod value;

#[cfg(test)]
#[path = "tests/blob_profile.rs"]
mod blob_profile;

pub use blob::Blob;
pub use cell_id::{CellId, new_cell_id};
pub use cells::Cells;
pub use document::Document;
pub use entries::{Entries, sorted_entries};
pub use hex::{hex_string, read_hex};
pub use list::List;
pub use path::{Path, Resolution, Step};
pub use position::Position;
pub use record::Record;
pub use value::Value;
