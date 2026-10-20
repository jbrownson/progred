//! The document's workspace vocabulary: its panes and its own libraries. The
//! editor defines and interprets both at the root; libraries can use the same
//! ordinary data when constructing templates.

pub mod vocabulary {
    pub use crate::workspace::vocabulary::{KEYS, LEFT, LIBRARIES, PANES, RIGHT};
}
