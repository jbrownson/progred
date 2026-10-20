//! The document's workspace vocabulary: its main value, its panes and its own
//! libraries. The editor defines and interprets them at the root; libraries
//! can use the same ordinary data when constructing templates. A workspace
//! record draws as its main value.

use crate::display::host::{Host, Layout, ProjectionInput};

pub mod vocabulary {
    pub use crate::workspace::vocabulary::{KEYS, LEFT, LIBRARIES, MAIN, PANES, RIGHT};
}

/// A workspace record draws only its main value, with no stop of its own,
/// so its own fields such as its panes stay out of sight. One holding only
/// those fields, as after its main value was deleted, draws an empty one.
pub(super) fn display<W: Host>(input: &ProjectionInput<'_, W>) -> Option<Layout<W>> {
    let main = crate::workspace::main_path(input.value)?;
    Some(crate::display::transparent(crate::display::descend(
        main, None, None,
    )))
}
