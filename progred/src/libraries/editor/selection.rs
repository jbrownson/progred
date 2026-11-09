//! Selection effects in a caller-supplied document and view. GET reads
//! the payload at the current projection site; SET takes an explicit
//! PATH and VALUE. The editor defines these cells and the selection's
//! stages beside the code that interprets them, and supplies the
//! implementation while dispatching.

pub mod vocabulary {
    pub use crate::selection::payload::vocabulary::{EDGE, LABEL, PENDING, STAGE};
    pub use crate::site::vocabulary::{
        NO_SELECTION, SELECTION_GET as GET, SELECTION_PATH as PATH, SELECTION_SET as SET,
    };
}

#[cfg(test)]
#[path = "tests/selection.rs"]
pub(crate) mod tests;
