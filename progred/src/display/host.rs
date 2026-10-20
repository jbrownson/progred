//! What a world lends the toolkit: what its pointer can rest on, and the
//! session of the editor's capabilities that a dispatch acts through. Display
//! and the libraries are generic over such a world in their types only; what
//! a handler does goes through the session, which isn't generic.

use crate::editing::Session;

pub trait Host: 'static {
    /// What the pointer rests on: what a tree draws, and whatever else the
    /// world draws around it, such as the shell's menus.
    type Hover: Clone + PartialEq + From<crate::hover::Hover> + 'static;

    /// What a tree drew that `hover` rests on, if it's that.
    fn tree(hover: &Self::Hover) -> Option<&crate::hover::Hover>;

    fn session(&mut self) -> Session<'_>;
}

/// A layout over `W`, hovered as `W` is.
pub type Layout<W> = super::Layout<W, <W as Host>::Hover>;
/// A projection's input over `W`, hovered as `W` is.
pub type ProjectionInput<'a, W, V = gid::Value> =
    super::ProjectionInput<'a, W, <W as Host>::Hover, V>;
/// A partial projection over `W`, hovered as `W` is.
pub type Partial<W> = super::Partial<W, <W as Host>::Hover>;
/// What a projection over `W` draws its occurrence as.
pub type Projected<W> = super::Projected<W, <W as Host>::Hover>;
