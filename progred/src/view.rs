//! The toolkit's types over the editor: layouts, projection inputs and
//! partial projections. Display and the libraries are generic over the world
//! that hosts them, through [`crate::display::host`]; the projection runtime,
//! the shell and tests use these.

use crate::Editor;

pub type Layout = crate::display::host::Layout<Editor>;
pub type ProjectionInput<'a, V = gid::Value> = crate::display::host::ProjectionInput<'a, Editor, V>;
pub type Partial = crate::display::host::Partial<Editor>;
