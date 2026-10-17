//! The view toolkit's types for the editor: layouts, projection inputs and
//! partial projections over the editor and its hover type. The toolkit in
//! [`crate::display`] stays generic; the editor's projections and libraries
//! use these.

use crate::Editor;
use crate::frame::Hovered;

pub type Layout = crate::display::Layout<Editor, Hovered>;
pub type ProjectionInput<'a, V = gid::Value> =
    crate::display::ProjectionInput<'a, Editor, Hovered, V>;
pub type Partial = crate::display::Partial<Editor, Hovered>;
