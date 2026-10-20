//! The document's workspace vocabulary: its main value, its panes and its own
//! libraries. The editor defines and interprets them at the root; libraries
//! can use the same ordinary data when constructing templates. A workspace
//! record draws as its main value.

use crate::display::host::{Host, Projected, ProjectionInput};

pub mod vocabulary {
    pub use crate::workspace::vocabulary::{KEYS, LEFT, LIBRARIES, MAIN, PANES, RIGHT};
}

/// A workspace record draws only its main value and is no landmark of its
/// own, so its own fields such as its panes stay out of sight and out of
/// reach. One holding only those fields, as after its main value was
/// deleted, draws an empty one.
pub(super) fn display<W: Host>(input: &ProjectionInput<'_, W>) -> Option<Projected<W>> {
    let main = crate::workspace::main_path(input.value)?;
    Some(Projected {
        layout: crate::display::descend(main, None, None),
        landmark: false,
    })
}
