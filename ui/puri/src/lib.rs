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
pub mod hover;
pub mod interact;
pub mod scroll;
pub mod text;

pub use delim::{Delim, DelimStyle};
pub use draw::{
    Canvas, Command, DrawCmd, DrawList, Drawing, Glyph, GlyphRun, Leaf, Shape, draw, replay,
};
pub use edit::{
    EditCtx, EditStyle, LineEdit, LineEditDescription, LineEditGeometry, LineEditPointerDown,
    LineEditPresentation, LineEditState, TextClipboard, text_edit,
};
pub use geometry::Placement;
pub use handler::{Handler, HasHandler, ImeEvent, capture};
pub use interact::{
    clickable, double_clickable, is_primary_contact, is_primary_contact_move, on_primary_click,
    on_primary_pointer_down, on_primary_pointer_down_where,
};
pub use kurbo::{
    Affine, BezPath, Circle, Line, PathEl, Point, Rect, RoundedRect, Size, Stroke, Vec2,
};
pub use peniko::{Brush, Color, ColorStop, Gradient, ImageAlphaType, ImageData, ImageFormat};
pub use text::{Text, TextCache, TextCtx, TextMetrics, TextStyle, paragraph, text};
