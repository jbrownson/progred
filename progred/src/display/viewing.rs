//! What a widget reads about the view it's built in: the occurrence it
//! draws and the view's reading context. A library's widgets read only
//! this; the toolkit's also read the theme and the selection's details.

use gid::{Step, Value};

pub trait Viewing {
    /// Physical pixels per logical point, for what's drawn in pixels.
    fn scale(&self) -> f64;
    /// Whether an edit to this occurrence would be stored.
    fn writable(&self) -> bool;
    /// The document read over its libraries, for evaluating Grap.
    fn host(&self) -> &dyn ::grap::Host;
    /// The editor's memo roots and background work.
    fn computations(&self) -> &crate::computations::Computations;
    /// The per-path state `steps` below this occurrence.
    fn state_below(&self, steps: &[Step]) -> Option<&Value>;
    /// Whether the selection is `steps` below this occurrence, or deeper.
    fn selected_below(&self, steps: &[Step]) -> bool;
}
