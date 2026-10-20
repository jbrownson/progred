//! The document's workspace vocabulary: its main value, its panes and its own
//! libraries. The editor defines and interprets them at the root; libraries
//! can use the same ordinary data when constructing templates.

pub mod vocabulary {
    pub use crate::workspace::vocabulary::{KEYS, LEFT, LIBRARIES, MAIN, PANES, RIGHT};
}
