//! Puri: a pure widget library. Widgets are pure functions from
//! (persistent widget state, props) to (draw calls, handlers); Puri
//! holds no state between frames, mints no identity, and retains no
//! hierarchy. Prepared descriptions expose their measurement and consume
//! caller-supplied [`Placement`]; Puri owns no layout tree or traversal.

mod batch;
pub mod delim;
pub mod drag;
pub mod draw;
pub mod edit;
pub mod frame;
pub mod gesture;
pub mod handler;
pub mod hover;
pub mod interact;
pub mod keyboard;
pub mod mesh;
pub mod scroll;
pub mod text;
pub mod timer;

pub use draw::{Canvas, DrawCmd, DrawList, Shape};
pub use kurbo::{Affine, Circle, Line, Point, Rect, RoundedRect, Stroke};
pub use peniko::{Brush, Color, ColorStop, Gradient};
pub use text::{Script, Text, TextCtx, TextMetrics, TextStyle, scripted_text, text};
pub use uig::Placement;
