//! Progred's built-in libraries. Most modules are one library's complete
//! contribution to the editor, through the interface in [`crate::library`];
//! `completion` and `representation` are helpers several share.

/// A built-in library's contribution, over a world that hosts the toolkit.
pub type Library<W> = crate::library::Library<W, <W as crate::display::Host>::Hover>;

pub mod absent;
pub mod color;
pub mod completion;
#[cfg(test)]
#[path = "tests/libraries/conformance.rs"]
mod conformance;
pub mod controls;
pub mod declared;
pub mod editor;
pub mod f32;
pub mod f64;
pub mod fidget;
pub mod grap;
pub mod isa;
pub mod layout;
pub mod list;
pub mod logic;
pub mod name;
pub mod number;
pub mod png;
pub mod presentation;
pub mod random;
pub mod representation;
pub mod sequence;
pub mod text;
pub mod toolpath;
pub mod tree;
pub mod u64;

#[cfg(test)]
#[path = "tests/libraries/test_widgets.rs"]
pub(crate) mod test_widgets;
#[cfg(test)]
#[path = "tests/libraries.rs"]
mod tests;
