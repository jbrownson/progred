//! Puri: a pure widget library. Widgets are pure functions from
//! (persistent widget state, props) to (draw calls, handlers); Puri
//! holds no state between frames, mints no identity, and retains no
//! hierarchy. See `docs/puri.md`.

pub mod delim;
pub mod draw;
pub mod edit;
pub mod handler;
pub mod interact;
pub mod layout;
pub mod scroll;
pub mod text;

pub use delim::{Delim, DelimStyle};
pub use draw::{Canvas, DrawCmd, DrawList, Glyph, GlyphRun, Shape, replay};
pub use edit::{
    EditCtx, EditStyle, LineEditDescription, LineEditPointerDown, LineEditPresentation,
    LineEditState, TextClipboard, text_edit,
};
pub use handler::{Handler, HasHandler, ImeEvent, capture};
pub use interact::{
    clickable, double_clickable, on_primary_click, on_primary_pointer_down,
    on_primary_pointer_down_where,
};
pub use layout::{
    Extent, HAlign, Node, PlaceInner, Placement, after, around, before, col, decorate, leaf,
    min_width, pad, place, place_top_left, row,
};
pub use scroll::{max_offset, place_scrolled};
pub use text::{TextCache, TextCtx, TextStyle, paragraph, text};
