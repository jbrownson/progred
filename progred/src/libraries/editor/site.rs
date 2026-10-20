//! Per-site access to the editor's annotations. GET and SET never
//! take a path: the host supplies them while dispatching an event
//! at a projection site. PATH exposes the site's occurrence path in its
//! view as data.
//! The editor defines these cells and the fold states beside the code
//! that interprets them.

pub mod vocabulary {
    pub use crate::annotations::{EXPANDED, FOLD, FOLDED};
    pub use crate::site::vocabulary::{GET, NO_STATE, PATH, SET, VALUE};
}

#[cfg(test)]
mod tests;
