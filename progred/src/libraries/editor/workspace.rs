//! The document's workspace vocabulary: the rows and columns its root splits
//! into, and its own libraries. The editor defines and interprets them at the
//! root; libraries can use the same ordinary data when constructing templates.

pub use crate::workspace::row;

pub mod vocabulary {
    pub use crate::workspace::vocabulary::{KEYS, LIBRARIES, PARTS, kind};
}
