//! Puri: a pure widget library. Widgets are pure functions from
//! (persistent widget state, props) to (draw calls, handlers); Puri
//! holds no state between frames, mints no identity, and retains no
//! hierarchy. See `docs/puri.md`.

pub mod draw;
pub mod edit;
pub mod handler;
pub mod layout;
pub mod text;

pub use draw::{Canvas, DrawCmd, DrawList, Glyph, GlyphRun, Shape, replay};
pub use edit::{EditCtx, EditStyle, LineEditState, text_edit};
pub use handler::{Handler, HasHandler, ImeEvent, capture};
pub use layout::{Extent, HAlign, Node, col, decorate, leaf, pad, place, place_top_left, row};
pub use text::{TextCtx, TextStyle, paragraph, text};
