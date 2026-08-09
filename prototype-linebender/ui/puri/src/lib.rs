//! Puri: a pure widget library. Widgets are pure functions from
//! (persistent widget state, props) to (draw calls, handlers); Puri
//! holds no state between frames, mints no identity, and retains no
//! hierarchy. Prepared descriptions expose their measurement and consume
//! caller-supplied [`Placement`]; Puri owns no layout tree or traversal.
//! See `docs/puri.md`.

pub mod delim;
pub mod draw;
pub mod edit;
pub mod geometry;
pub mod handler;
pub mod interact;
pub mod text;

pub use delim::{Delim, DelimStyle};
pub use draw::{Canvas, DrawCmd, DrawList, Glyph, GlyphRun, Shape, replay};
pub use edit::{
    EditCtx, EditStyle, LineEdit, LineEditDescription, LineEditPointerDown,
    LineEditPresentation, LineEditState, TextClipboard, text_edit,
};
pub use geometry::Placement;
pub use handler::{Handler, HasHandler, ImeEvent, capture};
pub use interact::{
    clickable, double_clickable, on_primary_click, on_primary_pointer_down,
    on_primary_pointer_down_where,
};
pub use text::{Text, TextCache, TextCtx, TextMetrics, TextStyle, paragraph, text};
